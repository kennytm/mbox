//! Trait to instruct how to properly drop and free pointers.

use std::ptr::drop_in_place;

use internal::gen_free;

/// Implemented for pointers which can be freed.
pub trait Free {
    /// Drops the content pointed by this pointer and frees it.
    ///
    /// Do not call this method if the pointer has been freed. Users of this trait should maintain a
    /// flag to track if the pointer has been freed or not (the Rust compiler will automatically do
    /// this with a `Drop` type).
    fn free(ptr: *mut Self);
}

fn free_ptr_ref<T>(ptr: *mut T) {
    unsafe {
        drop_in_place(ptr);
        gen_free(ptr);
    }
}

impl<T> Free for T {
    #[cfg(nightly_channel)]
    default fn free(ptr_ref: *mut T) {
        free_ptr_ref(ptr_ref);
    }

    #[cfg(stable_channel)]
    fn free(ptr_ref: *mut T) {
        free_ptr_ref(ptr_ref);
    }
}

impl<T> Free for [T] {
    fn free(fat_ptr: *mut [T]) {
        unsafe {
            let thin_ptr = (*fat_ptr).as_mut_ptr();
            drop_in_place(fat_ptr);
            gen_free(thin_ptr);
        }
    }
}

impl Free for str {
    fn free(fat_ptr: *mut str) {
        Free::free(fat_ptr as *mut [u8]);
    }
}


