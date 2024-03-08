`mbox`: `malloc`-based box
==========================

[![Crates.io](https://img.shields.io/crates/v/mbox.svg)](https://crates.io/crates/mbox)
[![docs.rs](https://docs.rs/mbox/badge.svg)](https://docs.rs/mbox)
[![Build status](https://github.com/kennytm/mbox/workflows/Rust/badge.svg)](https://github.com/kennytm/mbox/actions?query=workflow%3ARust)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE.txt)

This crate provides structures that wrap pointers returned from `malloc` as a Box, and
automatically `free` them on drop. These types allow you to interact with pointers and
null-terminated strings and arrays in a Rusty style.

> Note: this crate does not support Windows.
>
> Pointers in Rust are required to be aligned to be sound. However, there is no API on
> Windows that are both compatible with `free()` and supports aligned-malloc.
>
> Because the primary purpose of this crate is interoperability with C code working
> with `malloc()`, it is impossible for us to switch to the safe variant like
> [`_aligned_malloc()`](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/aligned-malloc)
> (which requires [`_aligned_free()`](https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/aligned-free)).
>
> On Windows, trying to use `MBox<T>` or `MArray<T>` with `T`'s alignment not equal to 1
> will panic the program.

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

* [`malloc_buf`](https://crates.io/crates/malloc_buf) — `Malloc<T>` is equivalent to `MBox<T>`.
  Note however that `MBox<[T]>::from_raw_parts` will not allow null, 0-length buffers; use a
  dangling pointer instead.

* [`cbox`](https://crates.io/crates/cbox) — When not using a custom `DisposeRef`, the
  `CSemiBox<'static, T>` type is equivalent to `MBox<T>`, and `CBox<T>` is equivalent to
  `&'static T`.

* [`c_vec`](https://crates.io/crates/c_vec) — When using `free` as the destructor, `CVec<T>` is
  equivalent to `MBox<[T]>` and `CSlice<T>` as `[T]`.

* [`malloced`](https://crates.io/crates/malloced) — `Malloced<T>` is equivalent to `MBox<T>`.
  Note however that `mbox` depends on `libc` (more stable, but also longer build-time) and
  doesn't support `dyn Any` downcasting.

* [`malloc-array`](https://crates.io/crates/malloc-array) — `HeapArray<T>` is similar to
  `MBox<T>`, but this crate focuses more on raw memory management.
