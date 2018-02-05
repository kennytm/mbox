//! Placement-new support.
//!
//! This allows you to write construct an `MBox` using the placement-in syntax:
//!
//! ```rust
//! #![feature(placement_in_syntax)]
//!
//! use mbox::MALLOC;
//!
//! let b = MALLOC <- 1 + 2 + 3;
//! assert_eq!(*b, 6);
//! ```

use std::ops::{Placer, Place, InPlace, BoxPlace, Boxed};
use std::mem::forget;

use mbox::MBox;
use internal::{gen_malloc, gen_free};
#[cfg(test)] use internal::DropCounter;

/// The placer for an `MBox`.
pub const MALLOC: MallocPlacer = MallocPlacer(());

#[doc(hidden)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MallocPlacer(());

#[doc(hidden)]
pub struct MPlace<T>(*mut T);

impl<T> Placer<T> for MallocPlacer {
    type Place = MPlace<T>;
    fn make_place(self) -> MPlace<T> {
        unsafe {
            MPlace(gen_malloc(1))
        }
    }
}

impl<T> Drop for MPlace<T> {
    fn drop(&mut self) {
        unsafe {
            gen_free(self.0);
        }
    }
}

unsafe impl<T> Place<T> for MPlace<T> {
    fn pointer(&mut self) -> *mut T {
        self.0
    }
}

impl<T> InPlace<T> for MPlace<T> {
    type Owner = MBox<T>;
    unsafe fn finalize(self) -> MBox<T> {
        let result = MBox::from_raw(self.0);
        forget(self);
        result
    }
}

impl<T> BoxPlace<T> for MPlace<T> {
    fn make_place() -> MPlace<T> {
        MALLOC.make_place()
    }
}

impl<T> Boxed for MBox<T> {
    type Data = T;
    type Place = MPlace<T>;
    unsafe fn finalize(filled: MPlace<T>) -> Self {
        filled.finalize()
    }
}

/*
// Doesn't work: see https://github.com/rust-lang/rust/issues/27779

#[test]
fn test_box() {
    let counter = DropCounter::default();
    {
        let mbox: MBox<DropCounter> = box counter.clone();
        mbox.assert_eq(0);
        counter.assert_eq(0);
    }
    counter.assert_eq(1);
}
*/

#[test]
fn test_in_place() {
    let counter = DropCounter::default();
    {
        let mbox: MBox<DropCounter> = MALLOC <- counter.clone();
        mbox.assert_eq(0);
        counter.assert_eq(0);
    }
    counter.assert_eq(1);
}

#[test]
#[should_panic(expected="should panic without crash")]
#[allow(unreachable_code)]
fn test_panic_during_construction() {
    let _: MBox<DropCounter> = MALLOC <- panic!("should panic without crash");
}

