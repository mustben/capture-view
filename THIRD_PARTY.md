# Third-party software

Runtime Windows APIs are accessed through Microsoft's Rust `windows` crate (0.62.2 in the initial lockfile). Its license is MIT OR Apache-2.0. Transitive Rust dependencies and exact versions/checksums are recorded in `Cargo.lock`.

- Project: https://github.com/microsoft/windows-rs
- Package metadata: https://crates.io/crates/windows

The distributable includes a `third-party-licenses` directory containing the LICENSE/COPYING/NOTICE files shipped in the exact downloaded Cargo packages. Keep those notices with distributed binaries. This file is attribution, not a substitute for the package license texts.

Windows components and manufacturer drivers remain external prerequisites and are not redistributed. AVerMedia names identify supported hardware only. OBS, FFmpeg, VLC, .NET and DirectShowLib are not runtime dependencies and are not shipped.
