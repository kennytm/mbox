use libc::{c_void, realloc, free};
#[cfg(windows)] use libc::malloc;
#[cfg(all(not(windows), target_os="android"))] use libc::memalign;
#[cfg(all(not(windows), not(target_os="android")))] use libc::posix_memalign;

use std::mem::{size_of, align_of};

#[cfg(all(test, not(feature="no-std")))] use std::rc::Rc;
#[cfg(all(test, not(feature="no-std")))] use std::cell::Cell;

#[cfg(nightly_channel)] pub use std::ptr::Unique;
#[cfg(nightly_channel)] pub use std::mem::POST_DROP_USIZE;

#[cfg(stable_channel)] use std::marker::PhantomData;
#[cfg(stable_channel)] use std::ops::Deref;

#[cfg(all(not(windows), not(target_os="android")))] use std::cmp::max;
#[cfg(all(not(windows), not(target_os="android")))] use std::ptr::null_mut;

/// A pointer indicating the parent structure embedding it has been dropped.
#[cfg(all(stable_channel, target_pointer_width="64"))] pub const POST_DROP_USIZE: usize = 0x1d1d1d1d1d1d1d1d;
#[cfg(all(stable_channel, target_pointer_width="32"))] pub const POST_DROP_USIZE: usize = 0x1d1d1d1d;
#[cfg(all(stable_channel, target_pointer_width="16"))] pub const POST_DROP_USIZE: usize = 0x1d1d;

//{{{ Unique --------------------------------------------------------------------------------------

/// Same as `std::ptr::Unique`, but provides a close-enough representation on stable channel.
#[cfg(stable_channel)]
pub struct Unique<T: ?Sized> {
    pointer: *mut T,
    marker: PhantomData<T>,
}

#[cfg(stable_channel)]
unsafe impl<T: Send + ?Sized> Send for Unique<T> {}

#[cfg(stable_channel)]
unsafe impl<T: Sync + ?Sized> Sync for Unique<T> {}

#[cfg(stable_channel)]
impl<T: ?Sized> Unique<T> {
    pub unsafe fn new(ptr: *mut T) -> Unique<T> {
        Unique {
            pointer: ptr,
            marker: PhantomData,
        }
    }
}

#[cfg(stable_channel)]
impl<T: ?Sized> Deref for Unique<T> {
    type Target = *mut T;
    fn deref(&self) -> &*mut T {
        &self.pointer
    }
}

//}}}

//{{{ gen_malloc ----------------------------------------------------------------------------------

/// An arbitrary non-zero pointer is not allocated through `malloc`. This is the pointer used for
/// zero-sized types.
pub const NON_MALLOC_PTR: *mut c_void = 1 as *mut c_void;

#[cfg(windows)]
unsafe fn malloc_aligned(size: usize, _align: usize) -> *mut c_void {
    malloc(size)
}

#[cfg(all(not(windows), target_os="android"))]
unsafe fn malloc_aligned(size: usize, align: usize) -> *mut c_void {
    memalign(align, size)
}

#[cfg(all(not(windows), not(target_os="android")))]
unsafe fn malloc_aligned(size: usize, align: usize) -> *mut c_void {
    let mut result = null_mut();
    let align = max(align, size_of::<*mut ()>());
    posix_memalign(&mut result, align, size);
    result
}

/// Generic malloc function.
pub unsafe fn gen_malloc<T>(count: usize) -> *mut T {
    if size_of::<T>() == 0 || count == 0 {
        NON_MALLOC_PTR as *mut T
    } else {
        let requested_size = count.checked_mul(size_of::<T>()).expect("memory overflow");
        malloc_aligned(requested_size, align_of::<T>()) as *mut T
    }
}

/// Generic free function.
pub unsafe fn gen_free<T>(ptr: *mut T) {
    let p = ptr as *mut c_void;
    if p != NON_MALLOC_PTR {
        free(p);
    }
}

/// Generic realloc function.
pub unsafe fn gen_realloc<T>(ptr: *mut T, new_count: usize) -> *mut T {
    if size_of::<T>() == 0 {
        ptr
    } else if new_count == 0 {
        gen_free(ptr);
        NON_MALLOC_PTR as *mut T
    } else if ptr == NON_MALLOC_PTR as *mut T {
        gen_malloc(new_count)
    } else {
        let requested_size = new_count.checked_mul(size_of::<T>()).expect("memory overflow");
        realloc(ptr as *mut c_void, requested_size) as *mut T
    }
}

//}}}

//{{{ Drop counter --------------------------------------------------------------------------------

/// A test structure to count how many times the value has been dropped.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature="no-std"), derive(Default))]
pub struct DropCounter {
    #[cfg(not(feature="no-std"))]
    pub counter: Rc<Cell<usize>>,

    #[cfg(feature="no-std")]
    pub counter: *mut usize,
}

#[cfg(all(test, feature="no-std"))]
impl Default for DropCounter {
    fn default() -> DropCounter {
        unsafe {
            let ptr = gen_malloc(1);
            *ptr = 0;
            DropCounter { counter: ptr }
        }
    }
}

#[cfg(test)]
impl DropCounter {
    #[cfg(not(feature="no-std"))]
    pub fn assert_eq(&self, value: usize) {
        assert_eq!(self.counter.get(), value);
    }

    #[cfg(feature="no-std")]
    pub fn assert_eq(&self, value: usize) {
        unsafe {
            assert_eq!(*self.counter, value);
        }
    }
}

#[cfg(test)]
impl Drop for DropCounter {
    #[cfg(not(feature="no-std"))]
    fn drop(&mut self) {
        let cell: &Cell<usize> = &self.counter;
        cell.set(cell.get() + 1);
    }

    #[cfg(feature="no-std")]
    fn drop(&mut self) {
        unsafe {
            *self.counter += 1;
            // we don't care about the leak in test.
        }
    }
}

#[doc(hidden)]
#[cfg(all(test, feature="no-std"))]
pub trait GetExt {
    fn get(&self) -> usize;
}

#[cfg(all(test, feature="no-std"))]
impl GetExt for *mut usize {
    fn get(&self) -> usize {
        unsafe { **self }
    }
}

//}}}

//{{{ Panic-on-clone ------------------------------------------------------------------------------

/// A test structure which panics when it is cloned.
#[cfg(test)]
#[derive(Default)]
pub struct PanicOnClone(u8);

#[cfg(test)]
impl Clone for PanicOnClone {
    fn clone(&self) -> Self {
        panic!("panic on clone");
    }
}

//}}}
