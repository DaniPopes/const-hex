# const-hex

[![github](https://img.shields.io/badge/github-danipopes/const-hex-8da0cb?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/danipopes/const-hex)
[![crates.io](https://img.shields.io/crates/v/const-hex.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/const-hex)
[![docs.rs](https://img.shields.io/badge/docs.rs-const-hex-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs)](https://docs.rs/const-hex)
[![build status](https://img.shields.io/github/actions/workflow/status/danipopes/const-hex/ci.yml?branch=master&style=for-the-badge)](https://github.com/danipopes/const-hex/actions?query=branch%3Amaster)

This crate provides a fast conversion of byte arrays to hexadecimal strings.
The implementation comes mostly from, and extends, the [`hex`] crate, but
avoids the performance penalty of going through [`core::fmt::Formatter`] or any
heap allocation.

_Version requirement: rustc 1.51+_

[`core::fmt::Formatter`]: https://doc.rust-lang.org/std/fmt/struct.Formatter.html

## Acknowledgements

-   [`hex`] for most of the formatting implementation
-   [dtolnay]/[itoa] for the initial crate/library API layout

[`hex`]: https://crates.io/crates/hex
[dtolnay]: https://github.com/dtolnay
[itoa]: https://github.com/dtolnay/itoa

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or
[MIT license](./LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
