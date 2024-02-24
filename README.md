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

You can run these benchmarks with
`./benches/bench/gen-data.py && cargo bench --features std` on a nightly
compiler.

```log
test decode::const_hex::bench1_32b            ... bench:          14 ns/iter (+/- 0)
test decode::const_hex::bench2_256b           ... bench:          37 ns/iter (+/- 4)
test decode::const_hex::bench3_2k             ... bench:         226 ns/iter (+/- 7)
test decode::const_hex::bench4_16k            ... bench:       1,636 ns/iter (+/- 13)
test decode::const_hex::bench5_128k           ... bench:      12,644 ns/iter (+/- 84)
test decode::const_hex::bench6_1m             ... bench:     102,836 ns/iter (+/- 3,236)
test decode::faster_hex::bench1_32b           ... bench:          15 ns/iter (+/- 0)
test decode::faster_hex::bench2_256b          ... bench:          50 ns/iter (+/- 1)
test decode::faster_hex::bench3_2k            ... bench:         244 ns/iter (+/- 4)
test decode::faster_hex::bench4_16k           ... bench:       1,782 ns/iter (+/- 31)
test decode::faster_hex::bench5_128k          ... bench:      13,745 ns/iter (+/- 66)
test decode::faster_hex::bench6_1m            ... bench:     115,126 ns/iter (+/- 1,544)
test decode::hex::bench1_32b                  ... bench:         101 ns/iter (+/- 6)
test decode::hex::bench2_256b                 ... bench:         701 ns/iter (+/- 12)
test decode::hex::bench3_2k                   ... bench:       5,287 ns/iter (+/- 97)
test decode::hex::bench4_16k                  ... bench:      41,743 ns/iter (+/- 1,420)
test decode::hex::bench5_128k                 ... bench:     782,327 ns/iter (+/- 18,876)
test decode::hex::bench6_1m                   ... bench:   6,283,181 ns/iter (+/- 88,813)

test decode_to_slice::const_hex::bench1_32b   ... bench:           5 ns/iter (+/- 0)
test decode_to_slice::const_hex::bench2_256b  ... bench:          25 ns/iter (+/- 0)
test decode_to_slice::const_hex::bench3_2k    ... bench:         201 ns/iter (+/- 3)
test decode_to_slice::const_hex::bench4_16k   ... bench:       1,600 ns/iter (+/- 17)
test decode_to_slice::const_hex::bench5_128k  ... bench:      12,732 ns/iter (+/- 119)
test decode_to_slice::const_hex::bench6_1m    ... bench:     103,414 ns/iter (+/- 2,402)
test decode_to_slice::faster_hex::bench1_32b  ... bench:           6 ns/iter (+/- 0)
test decode_to_slice::faster_hex::bench2_256b ... bench:          28 ns/iter (+/- 0)
test decode_to_slice::faster_hex::bench3_2k   ... bench:         206 ns/iter (+/- 3)
test decode_to_slice::faster_hex::bench4_16k  ... bench:       1,640 ns/iter (+/- 13)
test decode_to_slice::faster_hex::bench5_128k ... bench:      13,065 ns/iter (+/- 92)
test decode_to_slice::faster_hex::bench6_1m   ... bench:     105,963 ns/iter (+/- 2,831)
test decode_to_slice::hex::bench1_32b         ... bench:          37 ns/iter (+/- 0)
test decode_to_slice::hex::bench2_256b        ... bench:         298 ns/iter (+/- 6)
test decode_to_slice::hex::bench3_2k          ... bench:       2,552 ns/iter (+/- 27)
test decode_to_slice::hex::bench4_16k         ... bench:      20,335 ns/iter (+/- 581)
test decode_to_slice::hex::bench5_128k        ... bench:     611,494 ns/iter (+/- 11,531)
test decode_to_slice::hex::bench6_1m          ... bench:   4,941,477 ns/iter (+/- 180,172)

test encode::const_hex::bench1_32b            ... bench:          10 ns/iter (+/- 0)
test encode::const_hex::bench2_256b           ... bench:          27 ns/iter (+/- 0)
test encode::const_hex::bench3_2k             ... bench:          97 ns/iter (+/- 0)
test encode::const_hex::bench4_16k            ... bench:         644 ns/iter (+/- 8)
test encode::const_hex::bench5_128k           ... bench:       4,967 ns/iter (+/- 52)
test encode::const_hex::bench6_1m             ... bench:      45,424 ns/iter (+/- 1,922)
test encode::faster_hex::bench1_32b           ... bench:          17 ns/iter (+/- 0)
test encode::faster_hex::bench2_256b          ... bench:          36 ns/iter (+/- 0)
test encode::faster_hex::bench3_2k            ... bench:          95 ns/iter (+/- 1)
test encode::faster_hex::bench4_16k           ... bench:         597 ns/iter (+/- 10)
test encode::faster_hex::bench5_128k          ... bench:       4,538 ns/iter (+/- 180)
test encode::faster_hex::bench6_1m            ... bench:      41,513 ns/iter (+/- 779)
test encode::hex::bench1_32b                  ... bench:          97 ns/iter (+/- 0)
test encode::hex::bench2_256b                 ... bench:         694 ns/iter (+/- 4)
test encode::hex::bench3_2k                   ... bench:       5,476 ns/iter (+/- 28)
test encode::hex::bench4_16k                  ... bench:      43,617 ns/iter (+/- 215)
test encode::hex::bench5_128k                 ... bench:     348,646 ns/iter (+/- 1,155)
test encode::hex::bench6_1m                   ... bench:   2,895,775 ns/iter (+/- 95,699)

test encode_to_slice::const_hex::bench1_32b   ... bench:           1 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench2_256b  ... bench:           6 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench3_2k    ... bench:          59 ns/iter (+/- 0)
test encode_to_slice::const_hex::bench4_16k   ... bench:         438 ns/iter (+/- 2)
test encode_to_slice::const_hex::bench5_128k  ... bench:       3,414 ns/iter (+/- 10)
test encode_to_slice::const_hex::bench6_1m    ... bench:      28,947 ns/iter (+/- 546)
test encode_to_slice::faster_hex::bench1_32b  ... bench:           4 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench2_256b ... bench:           7 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench3_2k   ... bench:          63 ns/iter (+/- 0)
test encode_to_slice::faster_hex::bench4_16k  ... bench:         390 ns/iter (+/- 5)
test encode_to_slice::faster_hex::bench5_128k ... bench:       3,012 ns/iter (+/- 22)
test encode_to_slice::faster_hex::bench6_1m   ... bench:      26,138 ns/iter (+/- 596)
test encode_to_slice::hex::bench1_32b         ... bench:          11 ns/iter (+/- 0)
test encode_to_slice::hex::bench2_256b        ... bench:         116 ns/iter (+/- 0)
test encode_to_slice::hex::bench3_2k          ... bench:         971 ns/iter (+/- 6)
test encode_to_slice::hex::bench4_16k         ... bench:       7,821 ns/iter (+/- 48)
test encode_to_slice::hex::bench5_128k        ... bench:      61,907 ns/iter (+/- 377)
test encode_to_slice::hex::bench6_1m          ... bench:     499,203 ns/iter (+/- 3,771)

test format::const_hex::bench1_32b            ... bench:          10 ns/iter (+/- 1)
test format::const_hex::bench2_256b           ... bench:          18 ns/iter (+/- 0)
test format::const_hex::bench3_2k             ... bench:         134 ns/iter (+/- 2)
test format::const_hex::bench4_16k            ... bench:       1,151 ns/iter (+/- 5)
test format::const_hex::bench5_128k           ... bench:       9,298 ns/iter (+/- 83)
test format::const_hex::bench6_1m             ... bench:      83,611 ns/iter (+/- 1,530)
test format::std::bench1_32b                  ... bench:         359 ns/iter (+/- 6)
test format::std::bench2_256b                 ... bench:       2,773 ns/iter (+/- 44)
test format::std::bench3_2k                   ... bench:      22,620 ns/iter (+/- 213)
test format::std::bench4_16k                  ... bench:     183,197 ns/iter (+/- 1,512)
test format::std::bench5_128k                 ... bench:   1,481,851 ns/iter (+/- 9,791)
test format::std::bench6_1m                   ... bench:  11,947,054 ns/iter (+/- 132,579)
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
