//! Placement-new support.
//!
//! Since placement-new support has been removed, this module does nothing now.

use internal::gen_free;

/// The placer for an `MBox`.
pub const MALLOC: MallocPlacer = MallocPlacer(());

#[doc(hidden)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MallocPlacer(());

#[doc(hidden)]
pub struct MPlace<T>(*mut T);

impl<T> Drop for MPlace<T> {
    fn drop(&mut self) {
        unsafe {
            gen_free(self.0);
        }
    }
}
