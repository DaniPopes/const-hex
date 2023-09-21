# const-hex

[![github](https://img.shields.io/badge/github-danipopes/const--hex-8da0cb?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/danipopes/const-hex)
[![crates.io](https://img.shields.io/crates/v/const-hex.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/const-hex)
[![docs.rs](https://img.shields.io/badge/docs.rs-const--hex-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs)](https://docs.rs/const-hex)
[![build status](https://img.shields.io/github/actions/workflow/status/danipopes/const-hex/ci.yml?branch=master&style=for-the-badge)](https://github.com/danipopes/const-hex/actions?query=branch%3Amaster)

This crate provides a fast conversion of byte arrays to hexadecimal strings,
both at compile time, and at run time.

Extends the [`hex`] crate's implementation with [const-eval], a
[const-generics formatting buffer][buffer], similar to [`itoa`]'s, and more.

_Version requirement: rustc 1.64+_

[const-eval]: https://docs.rs/const-hex/latest/const_hex/fn.const_encode.html
[buffer]: https://docs.rs/const-hex/latest/const_hex/struct.Buffer.html
[`itoa`]: https://docs.rs/itoa/latest/itoa/struct.Buffer.html

## Performance

This crate is ~10 times faster than [`hex`] in encoding, and 30+ times faster
than `libstd` in formatting.

You can run the following benchmarks with `cargo bench` on a nightly compiler.

```log
test decode::const_hex::bench1_32   ... bench:          12 ns/iter (+/- 0)
test decode::const_hex::bench2_128  ... bench:          43 ns/iter (+/- 0)
test decode::const_hex::bench3_4096 ... bench:       1,349 ns/iter (+/- 24)
test decode::hex::bench1_32         ... bench:          50 ns/iter (+/- 1)
test decode::hex::bench2_128        ... bench:         233 ns/iter (+/- 16)
test decode::hex::bench3_4096       ... bench:       7,323 ns/iter (+/- 85)
test encode::const_hex::bench1_32   ... bench:           1 ns/iter (+/- 0)
test encode::const_hex::bench2_128  ... bench:           4 ns/iter (+/- 0)
test encode::const_hex::bench3_4096 ... bench:         138 ns/iter (+/- 0)
test encode::hex::bench1_32         ... bench:          13 ns/iter (+/- 0)
test encode::hex::bench2_128        ... bench:          51 ns/iter (+/- 4)
test encode::hex::bench3_4096       ... bench:       1,593 ns/iter (+/- 13)
test format::const_hex::bench1_32   ... bench:          11 ns/iter (+/- 0)
test format::const_hex::bench2_128  ... bench:          14 ns/iter (+/- 0)
test format::const_hex::bench3_4096 ... bench:         312 ns/iter (+/- 2)
test format::std::bench1_32         ... bench:         462 ns/iter (+/- 1)
test format::std::bench2_128        ... bench:       1,929 ns/iter (+/- 4)
test format::std::bench3_4096       ... bench:      62,233 ns/iter (+/- 523)
```

## Acknowledgements

- [`hex`] for the initial encoding/decoding implementations
- [dtolnay]/[itoa] for the initial crate/library API layout

[`hex`]: https://crates.io/crates/hex
[dtolnay]: https://github.com/dtolnay
[itoa]: https://github.com/dtolnay/itoa

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in these crates by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
</sub>
