//! Trait to instruct how to properly drop and free pointers.

use std::ptr::drop_in_place;

use internal::{POST_DROP_USIZE, gen_free};

/// Implemented for pointers which can be freed.
pub trait Free {
    /// Drops the content pointed by this pointer and frees it.
    ///
    /// If the pointer is already dropped, this method should not do anything. This makes it "safe"
    /// to double-free even without a drop flag.
    fn free(ptr: *mut Self);
}

fn free_ptr_ref<T>(ptr: *mut T) {
    if ptr != POST_DROP_USIZE as *mut T {
        unsafe {
            drop_in_place(ptr);
            gen_free(ptr);
        }
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
            if thin_ptr != POST_DROP_USIZE as *mut T {
                drop_in_place(fat_ptr);
                gen_free(thin_ptr);
            }
        }
    }
}

impl Free for str {
    fn free(fat_ptr: *mut str) {
        Free::free(fat_ptr as *mut [u8]);
    }
}


