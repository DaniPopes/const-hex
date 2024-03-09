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
`1.78.0-nightly (a4472498d 2024-02-15)` on `x86_64-unknown-linux-gnu`.

You can run these benchmarks with `cargo bench --features std` on a nightly
compiler.

```log
test decode::const_hex::bench1_32b            ... bench:          16 ns/iter (+/- 5)
test decode::const_hex::bench2_256b           ... bench:          37 ns/iter (+/- 0)
test decode::const_hex::bench3_2k             ... bench:         232 ns/iter (+/- 2)
test decode::const_hex::bench4_16k            ... bench:       1,672 ns/iter (+/- 12)
test decode::const_hex::bench5_128k           ... bench:      12,979 ns/iter (+/- 91)
test decode::const_hex::bench6_1m             ... bench:     104,751 ns/iter (+/- 2,068)
test decode::faster_hex::bench1_32b           ... bench:          15 ns/iter (+/- 0)
test decode::faster_hex::bench2_256b          ... bench:          54 ns/iter (+/- 1)
test decode::faster_hex::bench3_2k            ... bench:         253 ns/iter (+/- 3)
test decode::faster_hex::bench4_16k           ... bench:       1,831 ns/iter (+/- 20)
test decode::faster_hex::bench5_128k          ... bench:      14,120 ns/iter (+/- 57)
test decode::faster_hex::bench6_1m            ... bench:     115,291 ns/iter (+/- 1,325)
test decode::hex::bench1_32b                  ... bench:         104 ns/iter (+/- 1)
test decode::hex::bench2_256b                 ... bench:         697 ns/iter (+/- 7)
test decode::hex::bench3_2k                   ... bench:       5,189 ns/iter (+/- 86)
test decode::hex::bench4_16k                  ... bench:      42,355 ns/iter (+/- 21,853)
test decode::hex::bench5_128k                 ... bench:     765,278 ns/iter (+/- 4,091)
test decode::hex::bench6_1m                   ... bench:   6,161,416 ns/iter (+/- 64,954)

test decode_to_slice::const_hex::bench1_32b   ... bench:           5 ns/iter (+/- 0)
test decode_to_slice::const_hex::bench2_256b  ... bench:          26 ns/iter (+/- 0)
test decode_to_slice::const_hex::bench3_2k    ... bench:         210 ns/iter (+/- 10)
test decode_to_slice::const_hex::bench4_16k   ... bench:       1,667 ns/iter (+/- 13)
test decode_to_slice::const_hex::bench5_128k  ... bench:      13,043 ns/iter (+/- 19)
test decode_to_slice::const_hex::bench6_1m    ... bench:     105,883 ns/iter (+/- 1,427)
test decode_to_slice::faster_hex::bench1_32b  ... bench:           6 ns/iter (+/- 0)
test decode_to_slice::faster_hex::bench2_256b ... bench:          28 ns/iter (+/- 0)
test decode_to_slice::faster_hex::bench3_2k   ... bench:         214 ns/iter (+/- 2)
test decode_to_slice::faster_hex::bench4_16k  ... bench:       1,710 ns/iter (+/- 6)
test decode_to_slice::faster_hex::bench5_128k ... bench:      13,304 ns/iter (+/- 37)
test decode_to_slice::faster_hex::bench6_1m   ... bench:     110,276 ns/iter (+/- 3,475)
test decode_to_slice::hex::bench1_32b         ... bench:          38 ns/iter (+/- 2)
test decode_to_slice::hex::bench2_256b        ... bench:         300 ns/iter (+/- 185)
test decode_to_slice::hex::bench3_2k          ... bench:       2,717 ns/iter (+/- 64)
test decode_to_slice::hex::bench4_16k         ... bench:      19,257 ns/iter (+/- 530)
test decode_to_slice::hex::bench5_128k        ... bench:     624,172 ns/iter (+/- 15,725)
test decode_to_slice::hex::bench6_1m          ... bench:   5,333,915 ns/iter (+/- 298,093)

test encode::const_hex::bench1_32b            ... bench:           6 ns/iter (+/- 0)
test encode::const_hex::bench2_256b           ... bench:          10 ns/iter (+/- 0)
test encode::const_hex::bench3_2k             ... bench:          72 ns/iter (+/- 1)
test encode::const_hex::bench4_16k            ... bench:         462 ns/iter (+/- 4)
test encode::const_hex::bench5_128k           ... bench:       3,600 ns/iter (+/- 28)
test encode::const_hex::bench6_1m             ... bench:      29,447 ns/iter (+/- 858)
test encode::faster_hex::bench1_32b           ... bench:          17 ns/iter (+/- 0)
test encode::faster_hex::bench2_256b          ... bench:          37 ns/iter (+/- 3)
test encode::faster_hex::bench3_2k            ... bench:         102 ns/iter (+/- 1)
test encode::faster_hex::bench4_16k           ... bench:         614 ns/iter (+/- 6)
test encode::faster_hex::bench5_128k          ... bench:       4,764 ns/iter (+/- 12)
test encode::faster_hex::bench6_1m            ... bench:      40,894 ns/iter (+/- 1,223)
test encode::hex::bench1_32b                  ... bench:         112 ns/iter (+/- 0)
test encode::hex::bench2_256b                 ... bench:         812 ns/iter (+/- 5)
test encode::hex::bench3_2k                   ... bench:       6,404 ns/iter (+/- 26)
test encode::hex::bench4_16k                  ... bench:      51,039 ns/iter (+/- 595)
test encode::hex::bench5_128k                 ... bench:     408,378 ns/iter (+/- 23,022)
test encode::hex::bench6_1m                   ... bench:   3,571,916 ns/iter (+/- 142,828)

test encode_to_slice::const_hex::bench1_32b   ... bench:           1 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench2_256b  ... bench:           6 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench3_2k    ... bench:          53 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench4_16k   ... bench:         452 ns/iter (+/- 3)
test encode_to_slice::const_hex::bench5_128k  ... bench:       3,550 ns/iter (+/- 10)
test encode_to_slice::const_hex::bench6_1m    ... bench:      29,605 ns/iter (+/- 916)
test encode_to_slice::faster_hex::bench1_32b  ... bench:           4 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench2_256b ... bench:           7 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench3_2k   ... bench:          47 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench4_16k  ... bench:         402 ns/iter (+/- 5)
test encode_to_slice::faster_hex::bench5_128k ... bench:       3,121 ns/iter (+/- 25)
test encode_to_slice::faster_hex::bench6_1m   ... bench:      26,171 ns/iter (+/- 573)
test encode_to_slice::hex::bench1_32b         ... bench:          11 ns/iter (+/- 0)
test encode_to_slice::hex::bench2_256b        ... bench:         118 ns/iter (+/- 0)
test encode_to_slice::hex::bench3_2k          ... bench:         994 ns/iter (+/- 4)
test encode_to_slice::hex::bench4_16k         ... bench:       8,065 ns/iter (+/- 31)
test encode_to_slice::hex::bench5_128k        ... bench:      63,982 ns/iter (+/- 2,026)
test encode_to_slice::hex::bench6_1m          ... bench:     515,171 ns/iter (+/- 2,789)

test format::const_hex::bench1_32b            ... bench:           9 ns/iter (+/- 0)
test format::const_hex::bench2_256b           ... bench:          18 ns/iter (+/- 0)
test format::const_hex::bench3_2k             ... bench:         119 ns/iter (+/- 1)
test format::const_hex::bench4_16k            ... bench:       1,157 ns/iter (+/- 3)
test format::const_hex::bench5_128k           ... bench:       9,560 ns/iter (+/- 443)
test format::const_hex::bench6_1m             ... bench:      85,479 ns/iter (+/- 1,498)
test format::std::bench1_32b                  ... bench:         374 ns/iter (+/- 6)
test format::std::bench2_256b                 ... bench:       2,952 ns/iter (+/- 10)
test format::std::bench3_2k                   ... bench:      23,767 ns/iter (+/- 61)
test format::std::bench4_16k                  ... bench:     183,579 ns/iter (+/- 2,078)
test format::std::bench5_128k                 ... bench:   1,498,391 ns/iter (+/- 8,445)
test format::std::bench6_1m                   ... bench:  11,965,082 ns/iter (+/- 43,784)
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
