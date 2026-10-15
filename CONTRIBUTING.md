# Contributing

Keep the core promise: one lightweight viewer window, not a streaming studio.

1. Explain the problem and hardware context in an issue before adding UI/features.
2. Use stable Rust and the checked-in lockfile; keep dependencies minimal.
3. Add a focused failing regression test before a bug fix. Keep pure logic separate from Win32/COM.
4. Format with `cargo fmt`; run Clippy with `-D warnings`, unit tests and a release build.
5. Document each unsafe boundary and preserve COM-thread/lifetime guarantees.
6. For graph/device changes, perform the optional hardware test and manual live AV tests. CI cannot prove a capture card works.
7. Never commit captured frames, logs with private data, device serials, proprietary drivers, firmware or personal machine paths.

Compatibility reports should distinguish graph connection from real source picture, audible audio and synchronization. Do not publish a new model as supported solely because it enumerates.

Use focused changes. Recording, cloud features, an embedded browser and automatic services are outside scope. A release binary is unsigned unless a maintainer explicitly arranges signing. Verify artifacts and their checksums before distributing.
