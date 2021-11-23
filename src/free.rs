use std::ptr::{drop_in_place, NonNull};

use internal::gen_free;

/// Trait to instruct how to properly drop and free pointers.
///
/// Implemented for pointers which can be freed.
pub trait Free {
    /// Drops the content pointed by this pointer and frees it.
    ///
    /// # Safety
    ///
    /// The `ptr` must be allocated through `malloc()`.
    ///
    /// Do not call this method if the pointer has been freed. Users of this trait should maintain a
    /// flag to track if the pointer has been freed or not (the Rust compiler will automatically do
    /// this with a `Drop` type).
    unsafe fn free(ptr: NonNull<Self>);
}

/// Drops the content of `*ptr`, then frees the `ptr` itself.
unsafe fn free_ptr_ref<T>(ptr: NonNull<T>) {
    drop_in_place(ptr.as_ptr());
    gen_free(ptr);
}

impl<T> Free for T {
    #[cfg(nightly_channel)]
    default unsafe fn free(ptr_ref: NonNull<Self>) {
        free_ptr_ref(ptr_ref);
    }

    #[cfg(stable_channel)]
    unsafe fn free(ptr_ref: NonNull<Self>) {
        free_ptr_ref(ptr_ref);
    }
}

impl<T> Free for [T] {
    unsafe fn free(mut fat_ptr: NonNull<Self>) {
        let thin_ptr = NonNull::new_unchecked(fat_ptr.as_mut().as_mut_ptr());
        drop_in_place(fat_ptr.as_ptr());
        gen_free(thin_ptr);
    }
}

impl Free for str {
    unsafe fn free(fat_ptr: NonNull<Self>) {
        Free::free(NonNull::new_unchecked(fat_ptr.as_ptr() as *mut [u8]));
    }
}
