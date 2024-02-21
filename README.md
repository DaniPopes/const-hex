# const-hex

[![github](https://img.shields.io/badge/github-danipopes/const--hex-8da0cb?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/danipopes/const-hex)
[![crates.io](https://img.shields.io/crates/v/const-hex.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/const-hex)
[![docs.rs](https://img.shields.io/badge/docs.rs-const--hex-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs)](https://docs.rs/const-hex)
[![build status](https://img.shields.io/github/actions/workflow/status/danipopes/const-hex/ci.yml?branch=master&style=for-the-badge)](https://github.com/danipopes/const-hex/actions?query=branch%3Amaster)

This crate provides a fast conversion of byte arrays to hexadecimal strings,
both at compile time, and at run time.

It aims to be a drop-in replacement for the [`hex`] crate, as well as extending
the API with [const-eval], a [const-generics formatting buffer][buffer], similar
to [`itoa`]'s, and more.

_Version requirement: rustc 1.64+_

[const-eval]: https://docs.rs/const-hex/latest/const_hex/fn.const_encode.html
[buffer]: https://docs.rs/const-hex/latest/const_hex/struct.Buffer.html
[`itoa`]: https://docs.rs/itoa/latest/itoa/struct.Buffer.html

## Performance

This crate's performance is comparable with [`faster-hex`], but the latter only
provides specialized implementations for `x86`/`x86-64`.

This crate is 10 to 50 times faster than [`hex`] in encoding and decoding, and
100+ times faster than `libstd` in formatting.

The following benchmarks were ran on an AMD Ryzen 9 7950X, compiled with
`1.78.0-nightly (2bf78d12d 2024-02-18)` on `x86_64-unknown-linux-gnu`.

You can run these benchmarks with
`./benches/bench/gen-data.py && cargo bench --features std` on a nightly
compiler.

```log
test decode::const_hex::bench1_32               ... bench:          14 ns/iter (+/- 0)
test decode::const_hex::bench2_256              ... bench:          36 ns/iter (+/- 0)
test decode::const_hex::bench3_2048             ... bench:         223 ns/iter (+/- 3)
test decode::const_hex::bench4_16384            ... bench:       1,607 ns/iter (+/- 26)
test decode::const_hex::bench5_262144           ... bench:      25,604 ns/iter (+/- 487)
test decode::faster_hex::bench1_32              ... bench:          15 ns/iter (+/- 0)
test decode::faster_hex::bench2_256             ... bench:          49 ns/iter (+/- 0)
test decode::faster_hex::bench3_2048            ... bench:         241 ns/iter (+/- 2)
test decode::faster_hex::bench4_16384           ... bench:       1,753 ns/iter (+/- 28)
test decode::faster_hex::bench5_262144          ... bench:      27,650 ns/iter (+/- 481)
test decode::hex::bench1_32                     ... bench:          94 ns/iter (+/- 5)
test decode::hex::bench2_256                    ... bench:         690 ns/iter (+/- 13)
test decode::hex::bench3_2048                   ... bench:       5,282 ns/iter (+/- 322)
test decode::hex::bench4_16384                  ... bench:      41,557 ns/iter (+/- 2,409)
test decode::hex::bench5_262144                 ... bench:   1,491,536 ns/iter (+/- 26,706)

test decode_to_slice::const_hex::bench1_32      ... bench:           5 ns/iter (+/- 0)
test decode_to_slice::const_hex::bench2_256     ... bench:          26 ns/iter (+/- 0)
test decode_to_slice::const_hex::bench3_2048    ... bench:         199 ns/iter (+/- 3)
test decode_to_slice::const_hex::bench4_16384   ... bench:       1,589 ns/iter (+/- 14)
test decode_to_slice::const_hex::bench5_262144  ... bench:      25,425 ns/iter (+/- 560)
test decode_to_slice::faster_hex::bench1_32     ... bench:           5 ns/iter (+/- 0)
test decode_to_slice::faster_hex::bench2_256    ... bench:          28 ns/iter (+/- 0)
test decode_to_slice::faster_hex::bench3_2048   ... bench:         205 ns/iter (+/- 3)
test decode_to_slice::faster_hex::bench4_16384  ... bench:       1,633 ns/iter (+/- 23)
test decode_to_slice::faster_hex::bench5_262144 ... bench:      25,873 ns/iter (+/- 1,158)
test decode_to_slice::hex::bench1_32            ... bench:          35 ns/iter (+/- 1)
test decode_to_slice::hex::bench2_256           ... bench:         297 ns/iter (+/- 12)
test decode_to_slice::hex::bench3_2048          ... bench:       2,538 ns/iter (+/- 178)
test decode_to_slice::hex::bench4_16384         ... bench:      20,496 ns/iter (+/- 1,198)
test decode_to_slice::hex::bench5_262144        ... bench:   1,215,996 ns/iter (+/- 12,647)

test encode::const_hex::bench1_32               ... bench:           9 ns/iter (+/- 0)
test encode::const_hex::bench2_256              ... bench:          29 ns/iter (+/- 0)
test encode::const_hex::bench3_2048             ... bench:          96 ns/iter (+/- 2)
test encode::const_hex::bench4_16384            ... bench:         641 ns/iter (+/- 8)
test encode::const_hex::bench5_262144           ... bench:      10,217 ns/iter (+/- 403)
test encode::faster_hex::bench1_32              ... bench:          16 ns/iter (+/- 0)
test encode::faster_hex::bench2_256             ... bench:          37 ns/iter (+/- 0)
test encode::faster_hex::bench3_2048            ... bench:         101 ns/iter (+/- 2)
test encode::faster_hex::bench4_16384           ... bench:         633 ns/iter (+/- 9)
test encode::faster_hex::bench5_262144          ... bench:      10,340 ns/iter (+/- 541)
test encode::hex::bench1_32                     ... bench:          95 ns/iter (+/- 7)
test encode::hex::bench2_256                    ... bench:         685 ns/iter (+/- 98)
test encode::hex::bench3_2048                   ... bench:       5,416 ns/iter (+/- 254)
test encode::hex::bench4_16384                  ... bench:      43,155 ns/iter (+/- 1,980)
test encode::hex::bench5_262144                 ... bench:     702,371 ns/iter (+/- 73,299)

test encode_to_slice::const_hex::bench1_32      ... bench:           1 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench2_256     ... bench:           6 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench3_2048    ... bench:          51 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench4_16384   ... bench:         443 ns/iter (+/- 20)
test encode_to_slice::const_hex::bench5_262144  ... bench:       7,009 ns/iter (+/- 269)
test encode_to_slice::faster_hex::bench1_32     ... bench:           4 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench2_256    ... bench:           7 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench3_2048   ... bench:          46 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench4_16384  ... bench:         407 ns/iter (+/- 13)
test encode_to_slice::faster_hex::bench5_262144 ... bench:       6,196 ns/iter (+/- 167)
test encode_to_slice::hex::bench1_32            ... bench:          11 ns/iter (+/- 0)
test encode_to_slice::hex::bench2_256           ... bench:         114 ns/iter (+/- 0)
test encode_to_slice::hex::bench3_2048          ... bench:         955 ns/iter (+/- 10)
test encode_to_slice::hex::bench4_16384         ... bench:       7,721 ns/iter (+/- 70)
test encode_to_slice::hex::bench5_262144        ... bench:     122,247 ns/iter (+/- 3,388)

test format::const_hex::bench1_32               ... bench:           9 ns/iter (+/- 0)
test format::const_hex::bench2_256              ... bench:          23 ns/iter (+/- 2)
test format::const_hex::bench3_2048             ... bench:         118 ns/iter (+/- 3)
test format::const_hex::bench4_16384            ... bench:       1,133 ns/iter (+/- 15)
test format::const_hex::bench5_262144           ... bench:      19,991 ns/iter (+/- 1,012)
test format::std::bench1_32                     ... bench:         338 ns/iter (+/- 4)
test format::std::bench2_256                    ... bench:       2,700 ns/iter (+/- 55)
test format::std::bench3_2048                   ... bench:      21,852 ns/iter (+/- 278)
test format::std::bench4_16384                  ... bench:     178,010 ns/iter (+/- 1,697)
test format::std::bench5_262144                 ... bench:   2,865,767 ns/iter (+/- 62,347)
```

## Acknowledgements

- [`hex`] for the initial encoding/decoding implementations
- [`faster-hex`] for the `x86`/`x86-64` check and decode implementations
- [dtolnay]/[itoa] for the initial crate/library API layout

[`hex`]: https://crates.io/crates/hex
[`faster-hex`]: https://crates.io/crates/faster-hex
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
