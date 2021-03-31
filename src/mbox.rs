//! `malloc`-based Box.

use stable_deref_trait::StableDeref;

use std::borrow::{Borrow, BorrowMut};
use std::cmp::{max, Ordering};
use std::convert::{AsMut, AsRef};
use std::fmt::{Debug, Display, Formatter, Pointer, Result as FormatResult};
use std::hash::{Hash, Hasher};
use std::iter::{DoubleEndedIterator, FromIterator, IntoIterator};
use std::mem::forget;
use std::ops::{Deref, DerefMut};
use std::ptr::{copy_nonoverlapping, drop_in_place, read, write};
use std::slice::{from_raw_parts, from_raw_parts_mut, Iter, IterMut};
use std::str::{from_utf8, from_utf8_unchecked, Utf8Error};

use internal::{gen_free, gen_malloc, gen_realloc, Unique};

#[cfg(all(test, not(feature = "std")))]
use internal::GetExt;
#[cfg(test)]
use internal::{DropCounter, PanicOnClone};
#[cfg(test)]
use std::iter::{once, repeat};
#[cfg(test)]
use std::mem::size_of;

#[cfg(nightly_channel)]
use std::marker::Unsize;
#[cfg(nightly_channel)]
use std::ops::CoerceUnsized;

use free::Free;

//{{{ Basic structure -----------------------------------------------------------------------------

/// A malloc-backed box. This structure allows Rust to exchange objects with C without cloning.
pub struct MBox<T: ?Sized + Free>(Unique<T>);

impl<T: ?Sized + Free> MBox<T> {
    /// Constructs a new malloc-backed box from a pointer allocated by `malloc`. The content of the
    /// pointer must be already initialized.
    pub unsafe fn from_raw(ptr: *mut T) -> MBox<T> {
        MBox(Unique::new_unchecked(ptr))
    }

    /// Obtains the pointer owned by the box.
    pub fn as_ptr(&self) -> *const T {
        self.0.as_ptr()
    }

    /// Obtains the mutable pointer owned by the box.
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T: ?Sized + Free> MBox<T> {
    /// Consumes the box and returns the original pointer.
    ///
    /// The caller is responsible for `free`ing the pointer after this.
    pub fn into_raw(mut self) -> *mut T {
        let ptr = self.as_mut_ptr();
        forget(self);
        ptr
    }
}

impl<T: ?Sized + Free> Drop for MBox<T> {
    fn drop(&mut self) {
        T::free(self.as_mut_ptr());
    }
}

impl<T: ?Sized + Free> Deref for MBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.as_ptr() }
    }
}

unsafe impl<T: ?Sized + Free> StableDeref for MBox<T> {}

impl<T: ?Sized + Free> DerefMut for MBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.as_mut_ptr() }
    }
}

impl<T: ?Sized + Free> AsRef<T> for MBox<T> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T: ?Sized + Free> AsMut<T> for MBox<T> {
    fn as_mut(&mut self) -> &mut T {
        self
    }
}

impl<T: ?Sized + Free> Borrow<T> for MBox<T> {
    fn borrow(&self) -> &T {
        self
    }
}

impl<T: ?Sized + Free> BorrowMut<T> for MBox<T> {
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}

#[cfg(nightly_channel)]
impl<T: ?Sized + Free + Unsize<U>, U: ?Sized + Free> CoerceUnsized<MBox<U>> for MBox<T> {}

impl<T: ?Sized + Free> Pointer for MBox<T> {
    fn fmt(&self, formatter: &mut Formatter) -> FormatResult {
        Pointer::fmt(&self.as_ptr(), formatter)
    }
}

impl<T: ?Sized + Free + Debug> Debug for MBox<T> {
    fn fmt(&self, formatter: &mut Formatter) -> FormatResult {
        self.deref().fmt(formatter)
    }
}

impl<T: ?Sized + Free + Display> Display for MBox<T> {
    fn fmt(&self, formatter: &mut Formatter) -> FormatResult {
        self.deref().fmt(formatter)
    }
}

impl<T: ?Sized + Free + Hash> Hash for MBox<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl<U: ?Sized + Free, T: ?Sized + Free + PartialEq<U>> PartialEq<MBox<U>> for MBox<T> {
    fn eq(&self, other: &MBox<U>) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<T: ?Sized + Free + Eq> Eq for MBox<T> {}

impl<U: ?Sized + Free, T: ?Sized + Free + PartialOrd<U>> PartialOrd<MBox<U>> for MBox<T> {
    fn partial_cmp(&self, other: &MBox<U>) -> Option<Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<T: ?Sized + Free + Ord> Ord for MBox<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deref().cmp(other.deref())
    }
}

//}}}

//{{{ Single object -------------------------------------------------------------------------------

impl<T> MBox<T> {
    /// Constructs a new malloc-backed box, and move an initialized value into it.
    pub fn new(value: T) -> MBox<T> {
        unsafe {
            let storage = gen_malloc(1);
            write(storage, value);
            Self::from_raw(storage)
        }
    }
}

impl<T> From<T> for MBox<T> {
    fn from(value: T) -> MBox<T> {
        MBox::new(value)
    }
}

impl<T: Clone> Clone for MBox<T> {
    fn clone(&self) -> MBox<T> {
        let value: &T = self;
        MBox::new(value.clone())
    }

    fn clone_from(&mut self, source: &Self) {
        let ptr = self.as_mut_ptr();
        let src: &T = source;
        let clone = src.clone();
        unsafe {
            drop_in_place(ptr);
            write(ptr, clone);
        }
    }
}

impl<T: Default> Default for MBox<T> {
    fn default() -> MBox<T> {
        MBox::new(T::default())
    }
}

#[test]
fn test_single_object() {
    let counter = DropCounter::default();
    {
        let mbox = MBox::new(counter.clone());
        counter.assert_eq(0);
        drop(mbox);
    }
    counter.assert_eq(1);
}

#[test]
fn test_into_raw() {
    let mbox = MBox::new(66u8);
    let raw = mbox.into_raw();
    unsafe {
        assert_eq!(*raw, 66u8);
        gen_free(raw);
    }
}

#[test]
fn test_clone() {
    let counter = DropCounter::default();
    {
        let first_mbox = MBox::new(counter.clone());
        {
            let second_mbox = first_mbox.clone();
            counter.assert_eq(0);
            drop(second_mbox);
        }
        counter.assert_eq(1);
    }
    counter.assert_eq(2);
}

#[test]
fn test_clone_from() {
    let counter = DropCounter::default();
    {
        let first_mbox = MBox::new(counter.clone());
        {
            let mut second_mbox = MBox::new(counter.clone());
            counter.assert_eq(0);
            second_mbox.clone_from(&first_mbox);
            counter.assert_eq(1);
        }
        counter.assert_eq(2);
    }
    counter.assert_eq(3);
}

#[test]
fn test_no_drop_flag() {
    fn do_test_for_drop_flag(branch: bool, expected: usize) {
        let counter = DropCounter::default();
        let inner_counter = counter.counter.clone();
        {
            let mbox;
            if branch {
                mbox = MBox::new(counter.clone());
                let _ = &mbox;
            }
            assert_eq!(inner_counter.get(), 0);
        }
        assert_eq!(inner_counter.get(), expected);
    }

    do_test_for_drop_flag(true, 1);
    do_test_for_drop_flag(false, 0);

    if cfg!(nightly_channel) {
        assert_eq!(
            size_of::<MBox<DropCounter>>(),
            size_of::<*mut DropCounter>()
        );
    }
}

#[cfg(feature = "std")]
#[test]
fn test_format() {
    let a = MBox::new(3u64);
    assert_eq!(format!("{:p}", a), format!("{:p}", a.as_ptr()));
    assert_eq!(format!("{}", a), "3");
    assert_eq!(format!("{:?}", a), "3");
}

#[test]
fn test_standard_traits() {
    let mut a = MBox::new(0u64);
    assert_eq!(*a, 0);
    *a = 3;
    assert_eq!(*a, 3);
    assert_eq!(*a.as_ref(), 3);
    assert_eq!(*a.as_mut(), 3);
    assert_eq!(*(a.borrow() as &u64), 3);
    assert_eq!(*(a.borrow_mut() as &mut u64), 3);
    assert!(a == MBox::new(3u64));
    assert!(a != MBox::new(0u64));
    assert!(a < MBox::new(4u64));
    assert!(a > MBox::new(2u64));
    assert!(a <= MBox::new(4u64));
    assert!(a >= MBox::new(2u64));
    assert_eq!(a.cmp(&MBox::new(7u64)), Ordering::Less);
    assert_eq!(MBox::<u64>::default(), MBox::new(0u64));
}

#[test]
fn test_zero_sized_type() {
    let a = MBox::new(());
    assert!(!a.as_ptr().is_null());
}

#[test]
fn test_non_zero() {
    let b = 0u64;
    assert!(!Some(MBox::new(0u64)).is_none());
    assert!(!Some(MBox::new(())).is_none());
    assert!(!Some(MBox::new(&b)).is_none());

    if cfg!(nightly_channel) {
        assert_eq!(size_of::<Option<MBox<u64>>>(), size_of::<MBox<u64>>());
        assert_eq!(size_of::<Option<MBox<()>>>(), size_of::<MBox<()>>());
        assert_eq!(
            size_of::<Option<MBox<&'static u64>>>(),
            size_of::<MBox<&'static u64>>()
        );
    }
}

//}}}

//{{{ Slice helpers -------------------------------------------------------------------------------

struct MSliceBuilder<T> {
    ptr: *mut T,
    cap: usize,
    len: usize,
}

impl<T> MSliceBuilder<T> {
    fn with_capacity(cap: usize) -> MSliceBuilder<T> {
        MSliceBuilder {
            ptr: unsafe { gen_malloc(cap) },
            cap: cap,
            len: 0,
        }
    }

    fn push(&mut self, obj: T) {
        unsafe {
            if self.len >= self.cap {
                self.cap *= 2;
                self.ptr = gen_realloc(self.ptr, self.cap);
            }
            write(self.ptr.offset(self.len as isize), obj);
            self.len += 1;
        }
    }

    unsafe fn as_mboxed_slice(&mut self) -> MBox<[T]> {
        MBox::from_raw_parts(self.ptr, self.len as usize)
    }

    fn into_mboxed_slice(mut self) -> MBox<[T]> {
        let slice = unsafe { self.as_mboxed_slice() };
        forget(self);
        slice
    }
}

impl<T> Drop for MSliceBuilder<T> {
    fn drop(&mut self) {
        unsafe { self.as_mboxed_slice() };
    }
}

/// The iterator returned from `MBox<[T]>::into_iter()`.
pub struct MSliceIntoIter<T> {
    ptr: *mut T,
    begin: usize,
    end: usize,
}

impl<T> Iterator for MSliceIntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.begin == self.end {
            None
        } else {
            unsafe {
                let ptr = self.ptr.offset(self.begin as isize);
                self.begin += 1;
                Some(read(ptr))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end - self.begin;
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for MSliceIntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.begin == self.end {
            None
        } else {
            unsafe {
                self.end -= 1;
                let ptr = self.ptr.offset(self.end as isize);
                Some(read(ptr))
            }
        }
    }
}

unsafe impl<T: Send> Send for MSliceIntoIter<T> {}
unsafe impl<T: Sync> Sync for MSliceIntoIter<T> {}

impl<T> ExactSizeIterator for MSliceIntoIter<T> {}

impl<T> Drop for MSliceIntoIter<T> {
    fn drop(&mut self) {
        unsafe {
            let base = self.ptr.offset(self.begin as isize);
            let len = self.end - self.begin;
            let slice = from_raw_parts_mut(base, len) as *mut [T];
            drop_in_place(slice);
            gen_free(self.ptr);
        }
    }
}

//}}}

//{{{ Slice ---------------------------------------------------------------------------------------

impl<T> MBox<[T]> {
    /// Constructs a new malloc-backed slice from the pointer and the length (number of items).
    ///
    /// The `malloc`ed size of the pointer must be at least `len * size_of::<T>()`. The content
    /// must already been initialized.
    pub unsafe fn from_raw_parts(value: *mut T, len: usize) -> MBox<[T]> {
        let ptr = from_raw_parts_mut(value, len) as *mut [T];
        Self::from_raw(ptr)
    }
}

impl<T> Default for MBox<[T]> {
    fn default() -> Self {
        unsafe { Self::from_raw_parts(gen_malloc(0), 0) }
    }
}

impl<T: Clone> Clone for MBox<[T]> {
    fn clone(&self) -> Self {
        Self::from_slice(self)
    }
}

impl<T: Clone> MBox<[T]> {
    /// Creates a new `malloc`-boxed slice by cloning the content of an existing slice.
    pub fn from_slice(slice: &[T]) -> MBox<[T]> {
        let mut builder = MSliceBuilder::with_capacity(slice.len());
        for item in slice {
            builder.push(item.clone());
        }
        builder.into_mboxed_slice()
    }
}

impl<T> FromIterator<T> for MBox<[T]> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower_size, upper_size) = iter.size_hint();
        let initial_capacity = max(upper_size.unwrap_or(lower_size), 1);
        let mut builder = MSliceBuilder::with_capacity(initial_capacity);
        for item in iter {
            builder.push(item);
        }
        builder.into_mboxed_slice()
    }
}

impl<T> IntoIterator for MBox<[T]> {
    type Item = T;
    type IntoIter = MSliceIntoIter<T>;
    fn into_iter(mut self) -> MSliceIntoIter<T> {
        let ptr = (*self).as_mut_ptr();
        let len = self.len();
        forget(self);
        MSliceIntoIter {
            ptr: ptr,
            begin: 0,
            end: len,
        }
    }
}

impl<'a, T> IntoIterator for &'a MBox<[T]> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut MBox<[T]> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

#[test]
fn test_slice() {
    unsafe {
        let slice_content = gen_malloc::<u64>(5);
        *slice_content.offset(0) = 16458340076686561191;
        *slice_content.offset(1) = 15635007859502065083;
        *slice_content.offset(2) = 4845947824042606450;
        *slice_content.offset(3) = 8907026173756975745;
        *slice_content.offset(4) = 7378932587879886134;
        let mbox = MBox::from_raw_parts(slice_content, 5);
        assert_eq!(
            &mbox as &[u64],
            &[
                16458340076686561191,
                15635007859502065083,
                4845947824042606450,
                8907026173756975745,
                7378932587879886134
            ]
        );
    }
}

#[test]
fn test_slice_with_drops() {
    let counter = DropCounter::default();
    unsafe {
        let slice_content = gen_malloc::<DropCounter>(3);
        {
            write(slice_content.offset(0), counter.clone());
            write(slice_content.offset(1), counter.clone());
            write(slice_content.offset(2), counter.clone());
        }
        counter.assert_eq(0);
        let mbox = MBox::from_raw_parts(slice_content, 3);
        mbox[0].assert_eq(0);
        mbox[1].assert_eq(0);
        mbox[2].assert_eq(0);
        assert_eq!(mbox.len(), 3);
    }
    counter.assert_eq(3);
}

#[cfg(nightly_channel)]
#[test]
fn test_coerce_unsized() {
    let counter = DropCounter::default();
    {
        let pre_box = MBox::new([counter.clone(), counter.clone()]);
        counter.assert_eq(0);
        pre_box[0].assert_eq(0);
        pre_box[1].assert_eq(0);
        assert_eq!(pre_box.len(), 2);

        let post_box: MBox<[DropCounter]> = pre_box;
        counter.assert_eq(0);
        post_box[0].assert_eq(0);
        post_box[1].assert_eq(0);
        assert_eq!(post_box.len(), 2);
    }
    counter.assert_eq(2);
}

#[test]
fn test_empty_slice() {
    let mbox = MBox::<[DropCounter]>::default();
    let sl: &[DropCounter] = &mbox;
    assert_eq!(sl.len(), 0);
    assert!(!sl.as_ptr().is_null());
}

#[cfg(nightly_channel)]
#[test]
fn test_coerce_from_empty_slice() {
    let pre_box = MBox::<[DropCounter; 0]>::new([]);
    assert_eq!(pre_box.len(), 0);
    assert!(!pre_box.as_ptr().is_null());

    let post_box: MBox<[DropCounter]> = pre_box;
    let sl: &[DropCounter] = &post_box;
    assert_eq!(sl.len(), 0);
    assert!(!sl.as_ptr().is_null());
}

#[test]
fn test_clone_slice() {
    let counter = DropCounter::default();
    unsafe {
        let slice_content = gen_malloc::<DropCounter>(3);
        {
            write(slice_content.offset(0), counter.clone());
            write(slice_content.offset(1), counter.clone());
            write(slice_content.offset(2), counter.clone());
        }
        let mbox = MBox::from_raw_parts(slice_content, 3);
        assert_eq!(mbox.len(), 3);

        {
            let cloned_mbox = mbox.clone();
            counter.assert_eq(0);
            assert_eq!(cloned_mbox.len(), 3);
            cloned_mbox[0].assert_eq(0);
            cloned_mbox[1].assert_eq(0);
            cloned_mbox[2].assert_eq(0);
        }

        counter.assert_eq(3);
        mbox[0].assert_eq(3);
        mbox[1].assert_eq(3);
        mbox[2].assert_eq(3);
    }

    counter.assert_eq(6);
}

#[test]
fn test_from_iterator() {
    let counter = DropCounter::default();
    {
        let slice = repeat(counter.clone()).take(18).collect::<MBox<[_]>>();
        counter.assert_eq(1);
        assert_eq!(slice.len(), 18);
        for c in &slice {
            c.assert_eq(1);
        }
    }
    counter.assert_eq(19);
}

#[test]
fn test_into_iterator() {
    let counter = DropCounter::default();
    {
        let slice = repeat(counter.clone()).take(18).collect::<MBox<[_]>>();
        counter.assert_eq(1);
        assert_eq!(slice.len(), 18);
        for (i, c) in slice.into_iter().enumerate() {
            c.assert_eq(1 + i);
        }
    }
    counter.assert_eq(19);
}

#[cfg(feature = "std")]
#[test]
fn test_iter_properties() {
    let slice = vec![1, 4, 9, 16, 25].into_iter().collect::<MBox<[_]>>();
    let mut iter = slice.into_iter();
    assert_eq!(iter.size_hint(), (5, Some(5)));
    assert_eq!(iter.len(), 5);
    assert_eq!(iter.next(), Some(1));
    assert_eq!(iter.next_back(), Some(25));
    assert_eq!(iter.size_hint(), (3, Some(3)));
    assert_eq!(iter.len(), 3);
    assert_eq!(iter.collect::<Vec<_>>(), vec![4, 9, 16]);
}

#[test]
fn test_iter_drop() {
    let counter = DropCounter::default();
    {
        let slice = repeat(counter.clone()).take(18).collect::<MBox<[_]>>();
        counter.assert_eq(1);
        assert_eq!(slice.len(), 18);

        let mut iter = slice.into_iter();
        counter.assert_eq(1);
        {
            iter.next().unwrap().assert_eq(1)
        };
        {
            iter.next().unwrap().assert_eq(2)
        };
        {
            iter.next_back().unwrap().assert_eq(3)
        };
        counter.assert_eq(4);
    }
    counter.assert_eq(19);
}

#[test]
fn test_zst_slice() {
    let slice = repeat(()).take(7).collect::<MBox<[_]>>();
    let _ = slice.clone();
    slice.into_iter();
}

#[test]
#[should_panic(expected = "panic on clone")]
fn test_panic_during_clone() {
    let mbox = MBox::<PanicOnClone>::default();
    let _ = mbox.clone();
}

#[test]
#[should_panic(expected = "panic on clone")]
fn test_panic_during_clone_from() {
    let mut mbox = MBox::<PanicOnClone>::default();
    let other = MBox::default();
    mbox.clone_from(&other);
}

//}}}

//{{{ UTF-8 String --------------------------------------------------------------------------------

impl MBox<str> {
    /// Constructs a new malloc-backed string from the pointer and the length (number of UTF-8 code
    /// units).
    ///
    /// The `malloc`ed size of the pointer must be at least `len`. The content must already been
    /// initialized and be valid UTF-8.
    pub unsafe fn from_raw_utf8_parts_unchecked(value: *mut u8, len: usize) -> MBox<str> {
        let bytes = from_raw_parts(value, len);
        let string = from_utf8_unchecked(bytes) as *const str as *mut str;
        Self::from_raw(string)
    }

    /// Constructs a new malloc-backed string from the pointer and the length (number of UTF-8 code
    /// units).
    ///
    /// The `malloc`ed size of the pointer must be at least `len`. If the content does not contain
    /// valid UTF-8, this method returns an `Err`.
    pub unsafe fn from_raw_utf8_parts(value: *mut u8, len: usize) -> Result<MBox<str>, Utf8Error> {
        let bytes = from_raw_parts(value, len);
        let string = from_utf8(bytes)? as *const str as *mut str;
        Ok(Self::from_raw(string))
    }

    /// Converts the string into raw bytes.
    pub fn into_bytes(self) -> MBox<[u8]> {
        unsafe { MBox::from_raw(self.into_raw() as *mut [u8]) }
    }

    /// Creates a string from raw bytes. The bytes must be valid UTF-8.
    pub unsafe fn from_utf8_unchecked(bytes: MBox<[u8]>) -> MBox<str> {
        Self::from_raw(bytes.into_raw() as *mut str)
    }

    /// Creates a string from raw bytes. If the content does not contain valid UTF-8, this method
    /// returns an `Err`.
    pub fn from_utf8(mut bytes: MBox<[u8]>) -> Result<MBox<str>, Utf8Error> {
        unsafe {
            let len = bytes.len();
            let ptr = (*bytes).as_mut_ptr();
            forget(bytes);
            Self::from_raw_utf8_parts(ptr, len)
        }
    }

    /// Creates a new `malloc`-boxed string by cloning the content of an existing string slice.
    pub fn from_str(string: &str) -> MBox<str> {
        let len = string.len();
        unsafe {
            let new_slice = gen_malloc(len);
            copy_nonoverlapping(string.as_ptr(), new_slice, len);
            Self::from_raw_utf8_parts_unchecked(new_slice, len)
        }
    }
}

impl Default for MBox<str> {
    fn default() -> Self {
        unsafe { Self::from_raw_utf8_parts_unchecked(gen_malloc(0), 0) }
    }
}

impl Clone for MBox<str> {
    fn clone(&self) -> Self {
        Self::from_str(self)
    }
}

#[test]
fn test_string_from_bytes() {
    let bytes = MBox::from_slice(b"abcdef\xe4\xb8\x80\xe4\xba\x8c\xe4\xb8\x89");
    let string = MBox::from_utf8(bytes).unwrap();
    assert_eq!(&*string, "abcdef一二三");
    assert_eq!(string, MBox::from_str("abcdef一二三"));
    let bytes = string.into_bytes();
    assert_eq!(&*bytes, b"abcdef\xe4\xb8\x80\xe4\xba\x8c\xe4\xb8\x89");
}

#[test]
fn test_non_utf8() {
    let bytes = MBox::from_slice(b"\x88\x88\x88\x88");
    let string = MBox::from_utf8(bytes);
    assert!(string.is_err());
}

#[test]
fn test_default_str() {
    assert_eq!(MBox::<str>::default(), MBox::from_str(""));
}

#[test]
#[should_panic(expected = "panic on clone")]
fn test_panic_on_clone_slice() {
    let mbox: MBox<[PanicOnClone]> = once(PanicOnClone::default()).collect();
    let _ = mbox.clone();
}

//}}}
