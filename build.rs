extern crate rustc_version;

use rustc_version::{version_meta, Channel};

pub fn main() {
    let meta = version_meta().unwrap();

    let channel = match meta.channel {
        Channel::Dev | Channel::Nightly => "nightly",
        Channel::Beta | Channel::Stable => "stable",
    };
    println!("cargo:rustc-cfg={}_channel", channel);
}

