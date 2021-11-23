#[cfg(not(feature = "std"))]
extern crate alloc;

use libc::c_void;

use std::alloc::Layout;
use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::ptr::NonNull;

#[cfg(not(feature = "std"))]
use self::alloc::alloc::handle_alloc_error;
#[cfg(feature = "std")]
use std::alloc::handle_alloc_error;

#[cfg(nightly_channel)]
use std::marker::Unsize;
#[cfg(nightly_channel)]
use std::ops::CoerceUnsized;

//{{{ Unique --------------------------------------------------------------------------------------

/// Same as `std::ptr::Unique`, but provides a close-enough representation on stable channel.
pub(crate) struct Unique<T: ?Sized> {
    pointer: NonNull<T>,
    marker: PhantomData<T>,
}

unsafe impl<T: Send + ?Sized> Send for Unique<T> {}

unsafe impl<T: Sync + ?Sized> Sync for Unique<T> {}

impl<T: ?Sized> Unique<T> {
    /// Creates a new raw unique pointer.
    ///
    /// # Safety
    ///
    /// The `pointer`'s ownership must be transferred into the result. That is,
    /// it is no longer valid to touch `pointer` and its copies directly after
    /// calling this function.
    pub(crate) const unsafe fn new(pointer: NonNull<T>) -> Self {
        Self {
            pointer,
            marker: PhantomData,
        }
    }
}

impl<T: ?Sized> Unique<T> {
    pub(crate) fn as_non_null_ptr(&self) -> NonNull<T> {
        self.pointer
    }
}

#[cfg(nightly_channel)]
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Unique<U>> for Unique<T> {}

//}}}

//{{{ gen_malloc ----------------------------------------------------------------------------------

#[cfg(windows)]
unsafe fn malloc_aligned(size: usize, _align: usize) -> *mut c_void {
    libc::malloc(size)
}

#[cfg(all(not(windows), target_os = "android"))]
unsafe fn malloc_aligned(size: usize, align: usize) -> *mut c_void {
    libc::memalign(align, size)
}

#[cfg(all(not(windows), not(target_os = "android")))]
unsafe fn malloc_aligned(size: usize, align: usize) -> *mut c_void {
    let mut result = std::ptr::null_mut();
    let align = align.max(size_of::<*mut ()>());
    libc::posix_memalign(&mut result, align, size);
    result
}

/// Generic malloc function.
pub(crate) fn gen_malloc<T>(count: usize) -> NonNull<T> {
    if size_of::<T>() == 0 || count == 0 {
        NonNull::dangling()
    } else {
        let requested_size = count.checked_mul(size_of::<T>()).expect("memory overflow");
        // SAFETY:
        //  - allocating should be safe, duh.
        //  - in the rare case allocation failed, we throw an allocation error, so when we reach
        //    NonNull::new_unchecked we can be sure the result is not null.
        unsafe {
            let res = malloc_aligned(requested_size, align_of::<T>()) as *mut T;
            if res.is_null() {
                handle_alloc_error(Layout::new::<T>());
            }
            NonNull::new_unchecked(res)
        }
    }
}

/// Generic free function.
///
/// # Safety
///
/// The `ptr` must be obtained from `malloc()` or similar C functions.
pub(crate) unsafe fn gen_free<T>(ptr: NonNull<T>) {
    if ptr != NonNull::dangling() {
        libc::free(ptr.as_ptr() as *mut c_void);
    }
}

/// Generic realloc function.
///
/// # Safety
///
/// The `ptr` must be obtained from `malloc()` or similar C functions.
pub(crate) unsafe fn gen_realloc<T>(ptr: NonNull<T>, new_count: usize) -> NonNull<T> {
    if size_of::<T>() == 0 {
        ptr
    } else if new_count == 0 {
        gen_free(ptr);
        NonNull::dangling()
    } else if ptr == NonNull::dangling() {
        gen_malloc(new_count)
    } else {
        if let Some(requested_size) = new_count.checked_mul(size_of::<T>()) {
            let res = libc::realloc(ptr.as_ptr() as *mut c_void, requested_size);
            if !res.is_null() {
                return NonNull::new_unchecked(res as *mut T);
            }
        }
        handle_alloc_error(Layout::new::<T>());
    }
}

//}}}

//{{{ Drop counter --------------------------------------------------------------------------------

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Default))]
pub(crate) struct SharedCounter {
    #[cfg(feature = "std")]
    counter: std::rc::Rc<std::cell::Cell<usize>>,

    /// A shared, mutable counter on heap.
    #[cfg(not(feature = "std"))]
    counter: NonNull<usize>,
}

#[cfg(all(test, not(feature = "std")))]
impl Default for SharedCounter {
    fn default() -> Self {
        // SAFETY: malloc() returns an uninitialized integer which is then filled in.
        unsafe {
            let counter = gen_malloc(1);
            std::ptr::write(counter.as_ptr(), 0);
            Self { counter }
        }
    }
}

#[cfg(test)]
impl SharedCounter {
    /// Gets the counter value.
    pub(crate) fn get(&self) -> usize {
        #[cfg(feature = "std")]
        {
            self.counter.get()
        }
        // SAFETY: `self.counter` is malloc()'ed, initialized and never freed.
        #[cfg(not(feature = "std"))]
        unsafe {
            *self.counter.as_ref()
        }
    }

    /// Asserts the counter value equals to the input. Panics when different.
    pub(crate) fn assert_eq(&self, value: usize) {
        assert_eq!(self.get(), value);
    }

    /// Increases the counter by 1.
    fn inc(&self) {
        #[cfg(feature = "std")]
        {
            self.counter.set(self.counter.get() + 1);
        }
        // SAFETY: `self.counter` is malloc()'ed, initialized and never freed.
        // Since `SharedCounter` is not Sync nor Send, we are sure the
        // modification happens in the single thread, so we don't worry about
        // the interior mutation.
        #[cfg(not(feature = "std"))]
        unsafe {
            *self.counter.as_ptr() += 1;
        }
    }
}

/// A test structure to count how many times the value has been dropped.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct DropCounter(SharedCounter);

#[cfg(test)]
impl std::ops::Deref for DropCounter {
    type Target = SharedCounter;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.inc();
    }
}

//}}}

//{{{ Panic-on-clone ------------------------------------------------------------------------------

/// A test structure which panics when it is cloned.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct PanicOnClone(u8);

#[cfg(test)]
impl Clone for PanicOnClone {
    fn clone(&self) -> Self {
        panic!("panic on clone");
    }
}

//}}}
