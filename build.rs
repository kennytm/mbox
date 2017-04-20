extern crate rustc_version;
extern crate semver;

use rustc_version::{version_meta, Channel};
use semver::Version;

pub fn main() {
    let meta = version_meta().unwrap();

    let channel = match meta.channel {
        Channel::Dev | Channel::Nightly => "nightly",
        Channel::Beta | Channel::Stable => "stable",
    };
    println!("cargo:rustc-cfg={}_channel", channel);

    if meta.semver >= Version::parse("1.10.0").unwrap() {
        println!("cargo:rustc-cfg=can_use_from_bytes_with_nul_unchecked");
    }
}

