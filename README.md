`mbox`: `malloc`-based box
==========================

[![Crates.io](https://img.shields.io/crates/v/mbox.svg)](https://crates.io/crates/mbox)
[![docs.rs](https://docs.rs/mbox/badge.svg)](https://docs.rs/mbox)
[![Build status](https://github.com/kennytm/mbox/workflows/Rust/badge.svg)](https://github.com/kennytm/mbox/actions?query=workflow%3ARust)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE.txt)

This crate provides structures that wrap pointers returned from `malloc` as a Box, and
automatically `free` them on drop. These types allow you to interact with pointers and
null-terminated strings and arrays in a Rusty style.

## Examples

```rust
extern crate libc;
extern crate mbox;

use libc::{c_char, malloc, strcpy};
use mbox::MString;

// Assume we have a C function that returns a malloc'ed string.
unsafe extern "C" fn create_str() -> *mut c_char {
    let ptr = malloc(12) as *mut c_char;
    strcpy(ptr, b"Hello world\0".as_ptr() as *const c_char);
    ptr
}

fn main() {
    // we wrap the null-terminated string into an MString.
    let string = unsafe { MString::from_raw_unchecked(create_str()) };

    // check the content is expected.
    assert_eq!(&*string, "Hello world");

    // the string will be dropped by `free` after the code is done.
}
```

## Installation

Add this to your Cargo.toml:

```toml
[dependencies]
mbox = "0.6"
```

## Usage

This crate provides three main types, all of which uses the system's `malloc`/`free` as the
allocator.

* `MBox<T>` — Similar to `Box<T>`.
* `MString` — Similar to `std::ffi::CString`.
* `MArray<T>` — A null-terminated array, which can be used to represent e.g. array of C strings
  terminated by a null pointer.

## `#![no_std]`

You may compile `mbox` and disable the `std` feature to not link to `std` (it will still link to
`core`.

```toml
[dependencies]
mbox = { version = "0.6", default-features = false }
```

When `#![no_std]` is activated, you cannot convert an `MString` into a `std::ffi::CStr`, as the
type simply does not exist 🙂.

## Migrating from other crates

Note that `MBox` does not support custom allocator. If the type requires custom allocation,
`MBox` cannot serve you.

* [`malloc_buf`](https://crates.io/crates/malloc_buf) — `MallocBuffer<T>` is equivalent to
  `MBox<[T]>`. Note however we will not check for null pointers.

* [`cbox`](https://crates.io/crates/cbox) — When not using a custom `DisposeRef`, the
  `CSemiBox<'static, T>` type is equivalent to `MBox<T>`, and `CBox<T>` is equivalent to
  `&'static T`.

* [`c_vec`](https://crates.io/crates/c_vec) — When using `free` as the destructor, `CVec<T>` is
  equivalent to `MBox<[T]>` and `CSlice<T>` as `[T]`.
