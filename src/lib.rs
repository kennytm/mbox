//! `malloc`-based box.
//!
//! This crate provides structures that wrap pointers returned from `malloc` as a Box, and
//! automatically `free` them on drop. These types allow you to interact with pointers and
//! null-terminated strings and arrays in a Rusty style.
//!
//! # Examples
//!
//! ```rust
//! extern crate libc;
//! extern crate mbox;
//!
//! use libc::{c_char, malloc, strcpy};
//! use mbox::MString;
//!
//! // Assume we have a C function that returns a malloc'ed string.
//! unsafe extern "C" fn create_str() -> *mut c_char {
//!     let ptr = malloc(12) as *mut c_char;
//!     strcpy(ptr, b"Hello world\0".as_ptr() as *const c_char);
//!     ptr
//! }
//!
//! fn main() {
//!     // we wrap the null-terminated string into an MString.
//!     let string = unsafe { MString::from_raw_unchecked(create_str()) };
//!
//!     // check the content is expected.
//!     assert_eq!(&*string, "Hello world");
//!
//!     // the string will be dropped by `free` after the code is done.
//! }
//! ```
//!
//! # Usage
//!
//! This crate provides three main types, all of which uses the system's `malloc`/`free` as the
//! allocator.
//!
//! * [`MBox<T>`](mbox/struct.MBox.html) — Similar to `Box<T>`.
//! * [`MString`](sentinel/struct.MString.html) — Similar to `std::ffi::CString`.
//! * [`MArray<T>`](sentinel/struct.MArray.html) — A null-terminated array, which can be used to
//!   represent e.g. array of C strings terminated by a null pointer.
//!
//! # no-std
//!
//! You may compile `mbox` using the `no-std` feature to not link to `std` (it will still link to
//! `core`.
//!
//! ```toml
//! [dependencies]
//! mbox = { version = "0.1.1", features = ["no-std"] }
//! ```
//!
//! When `no-std` is activated, you cannot convert an `MString` into a `std::ffi::CStr`, as the
//! type simply does not exist 🙂.
//!
//! # Migrating from other crates
//!
//! Note that `MBox` does not support custom allocator. If the type requires custom allocation,
//! `MBox` cannot serve you.
//!
//! * [`malloc_buf`](https://crates.io/crates/malloc_buf) — `MallocBuffer<T>` is equivalent to
//!   `MBox<[T]>`. Note however we will not check for null pointers.
//!
//! * [`cbox`](https://crates.io/crates/cbox) — When not using a custom `DisposeRef`, the
//!   `CSemiBox<'static, T>` type is equivalent to `MBox<T>`, and `CBox<T>` is equivalent to
//!   `&'static T`.
//!
//! * [`c_vec`](https://crates.io/crates/c_vec) — When using `free` as the destructor, `CVec<T>` is
//!   equivalent to `MBox<[T]>` and `CSlice<T>` as `[T]`.

#![cfg_attr(nightly_channel, feature(specialization,
                                     unique,
                                     unsafe_no_drop_flag,
                                     filling_drop,
                                     unsize,
                                     coerce_unsized,
                                     placement_new_protocol,
                                     placement_in_syntax,
                                     box_syntax))]

#![cfg_attr(feature="no-std", no_std)]

#[cfg(feature="no-std")] extern crate core as std;
extern crate libc;

mod internal;
pub mod free;
pub mod sentinel;
pub mod mbox;
#[cfg(nightly_channel)] pub mod placer;

pub use mbox::MBox;
pub use sentinel::{MArray, MString};
#[cfg(nightly_channel)] pub use placer::MALLOC;

