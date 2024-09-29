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

This crate offers performance comparable to [`faster-hex`] on `x86`/`x86-64`
architectures but outperforms it on other platforms, as [`faster-hex`] is
only optimized for `x86`/`x86-64`.

This crate is 10 to 50 times faster than [`hex`] in encoding and decoding, and
100+ times faster than `libstd` in formatting.

The following benchmarks were ran on an AMD Ryzen 9 7950X, compiled with
`1.83.0-nightly (9e394f551 2024-09-25)` on `x86_64-unknown-linux-gnu`.

You can run these benchmarks with `cargo bench --features std` on a nightly
compiler.

```log
test check::const_hex::bench1_32b             ... bench:           7.36 ns/iter (+/- 0.34)
test check::const_hex::bench2_256b            ... bench:          19.39 ns/iter (+/- 0.27)
test check::const_hex::bench3_2k              ... bench:         121.85 ns/iter (+/- 15.13)
test check::const_hex::bench4_16k             ... bench:         903.95 ns/iter (+/- 13.53)
test check::const_hex::bench5_128k            ... bench:       7,121.20 ns/iter (+/- 57.48)
test check::const_hex::bench6_1m              ... bench:      57,834.53 ns/iter (+/- 1,000.67)
test check::faster_hex::bench1_32b            ... bench:           2.75 ns/iter (+/- 0.03)
test check::faster_hex::bench2_256b           ... bench:          14.95 ns/iter (+/- 0.45)
test check::faster_hex::bench3_2k             ... bench:         123.08 ns/iter (+/- 4.92)
test check::faster_hex::bench4_16k            ... bench:         983.89 ns/iter (+/- 18.29)
test check::faster_hex::bench5_128k           ... bench:       7,806.75 ns/iter (+/- 234.99)
test check::faster_hex::bench6_1m             ... bench:      64,115.09 ns/iter (+/- 754.27)
test check::naive::bench1_32b                 ... bench:          18.52 ns/iter (+/- 3.59)
test check::naive::bench2_256b                ... bench:         187.49 ns/iter (+/- 6.94)
test check::naive::bench3_2k                  ... bench:       1,953.95 ns/iter (+/- 52.85)
test check::naive::bench4_16k                 ... bench:      17,243.26 ns/iter (+/- 3,391.35)
test check::naive::bench5_128k                ... bench:     493,272.86 ns/iter (+/- 11,374.41)
test check::naive::bench6_1m                  ... bench:   4,193,959.30 ns/iter (+/- 180,118.90)

test decode::const_hex::bench1_32b            ... bench:          19.77 ns/iter (+/- 0.80)
test decode::const_hex::bench2_256b           ... bench:          41.15 ns/iter (+/- 1.48)
test decode::const_hex::bench3_2k             ... bench:         235.43 ns/iter (+/- 2.39)
test decode::const_hex::bench4_16k            ... bench:       1,703.37 ns/iter (+/- 5.44)
test decode::const_hex::bench5_128k           ... bench:      13,097.29 ns/iter (+/- 54.88)
test decode::const_hex::bench6_1m             ... bench:     105,834.33 ns/iter (+/- 1,860.67)
test decode::faster_hex::bench1_32b           ... bench:          17.09 ns/iter (+/- 0.26)
test decode::faster_hex::bench2_256b          ... bench:          55.30 ns/iter (+/- 0.56)
test decode::faster_hex::bench3_2k            ... bench:         249.42 ns/iter (+/- 7.53)
test decode::faster_hex::bench4_16k           ... bench:       1,867.34 ns/iter (+/- 12.68)
test decode::faster_hex::bench5_128k          ... bench:      14,542.82 ns/iter (+/- 114.09)
test decode::faster_hex::bench6_1m            ... bench:     118,627.86 ns/iter (+/- 2,471.00)
test decode::hex::bench1_32b                  ... bench:         111.69 ns/iter (+/- 7.82)
test decode::hex::bench2_256b                 ... bench:         728.81 ns/iter (+/- 18.34)
test decode::hex::bench3_2k                   ... bench:       5,263.46 ns/iter (+/- 87.04)
test decode::hex::bench4_16k                  ... bench:      42,284.40 ns/iter (+/- 2,312.96)
test decode::hex::bench5_128k                 ... bench:     800,810.80 ns/iter (+/- 7,695.87)
test decode::hex::bench6_1m                   ... bench:   6,442,642.10 ns/iter (+/- 38,417.90)

test decode_to_slice::const_hex::bench1_32b   ... bench:           9.90 ns/iter (+/- 2.75)
test decode_to_slice::const_hex::bench2_256b  ... bench:          29.02 ns/iter (+/- 1.99)
test decode_to_slice::const_hex::bench3_2k    ... bench:         210.05 ns/iter (+/- 8.65)
test decode_to_slice::const_hex::bench4_16k   ... bench:       1,667.70 ns/iter (+/- 12.13)
test decode_to_slice::const_hex::bench5_128k  ... bench:      13,083.20 ns/iter (+/- 96.53)
test decode_to_slice::const_hex::bench6_1m    ... bench:     108,756.59 ns/iter (+/- 2,321.92)
test decode_to_slice::faster_hex::bench1_32b  ... bench:           6.67 ns/iter (+/- 0.26)
test decode_to_slice::faster_hex::bench2_256b ... bench:          29.25 ns/iter (+/- 0.46)
test decode_to_slice::faster_hex::bench3_2k   ... bench:         218.65 ns/iter (+/- 2.40)
test decode_to_slice::faster_hex::bench4_16k  ... bench:       1,743.88 ns/iter (+/- 18.52)
test decode_to_slice::faster_hex::bench5_128k ... bench:      13,694.73 ns/iter (+/- 36.07)
test decode_to_slice::faster_hex::bench6_1m   ... bench:     110,733.30 ns/iter (+/- 1,679.82)
test decode_to_slice::hex::bench1_32b         ... bench:          37.57 ns/iter (+/- 0.85)
test decode_to_slice::hex::bench2_256b        ... bench:         287.52 ns/iter (+/- 23.10)
test decode_to_slice::hex::bench3_2k          ... bench:       2,705.00 ns/iter (+/- 26.99)
test decode_to_slice::hex::bench4_16k         ... bench:      21,850.53 ns/iter (+/- 191.97)
test decode_to_slice::hex::bench5_128k        ... bench:     614,217.67 ns/iter (+/- 2,237.99)
test decode_to_slice::hex::bench6_1m          ... bench:   5,357,921.20 ns/iter (+/- 240,508.79)

test encode::const_hex::bench1_32b            ... bench:           7.00 ns/iter (+/- 0.37)
test encode::const_hex::bench2_256b           ... bench:          11.83 ns/iter (+/- 0.05)
test encode::const_hex::bench3_2k             ... bench:          73.28 ns/iter (+/- 0.30)
test encode::const_hex::bench4_16k            ... bench:         467.14 ns/iter (+/- 26.32)
test encode::const_hex::bench5_128k           ... bench:       3,760.74 ns/iter (+/- 69.40)
test encode::const_hex::bench6_1m             ... bench:      29,080.93 ns/iter (+/- 532.47)
test encode::faster_hex::bench1_32b           ... bench:          17.25 ns/iter (+/- 0.17)
test encode::faster_hex::bench2_256b          ... bench:          39.03 ns/iter (+/- 0.77)
test encode::faster_hex::bench3_2k            ... bench:         102.46 ns/iter (+/- 1.27)
test encode::faster_hex::bench4_16k           ... bench:         655.39 ns/iter (+/- 2.28)
test encode::faster_hex::bench5_128k          ... bench:       5,233.70 ns/iter (+/- 11.75)
test encode::faster_hex::bench6_1m            ... bench:      43,802.73 ns/iter (+/- 1,115.53)
test encode::hex::bench1_32b                  ... bench:         102.98 ns/iter (+/- 0.75)
test encode::hex::bench2_256b                 ... bench:         721.27 ns/iter (+/- 4.31)
test encode::hex::bench3_2k                   ... bench:       5,659.67 ns/iter (+/- 18.84)
test encode::hex::bench4_16k                  ... bench:      45,138.29 ns/iter (+/- 352.13)
test encode::hex::bench5_128k                 ... bench:     361,400.70 ns/iter (+/- 1,472.30)
test encode::hex::bench6_1m                   ... bench:   3,210,824.02 ns/iter (+/- 207,640.35)

test encode_to_slice::const_hex::bench1_32b   ... bench:           1.56 ns/iter (+/- 0.00)
test encode_to_slice::const_hex::bench2_256b  ... bench:           6.72 ns/iter (+/- 0.03)
test encode_to_slice::const_hex::bench3_2k    ... bench:          58.79 ns/iter (+/- 1.45)
test encode_to_slice::const_hex::bench4_16k   ... bench:         510.57 ns/iter (+/- 11.70)
test encode_to_slice::const_hex::bench5_128k  ... bench:       4,030.22 ns/iter (+/- 76.92)
test encode_to_slice::const_hex::bench6_1m    ... bench:      35,273.20 ns/iter (+/- 583.54)
test encode_to_slice::faster_hex::bench1_32b  ... bench:           4.52 ns/iter (+/- 0.03)
test encode_to_slice::faster_hex::bench2_256b ... bench:           8.09 ns/iter (+/- 0.02)
test encode_to_slice::faster_hex::bench3_2k   ... bench:          53.83 ns/iter (+/- 1.28)
test encode_to_slice::faster_hex::bench4_16k  ... bench:         450.39 ns/iter (+/- 6.73)
test encode_to_slice::faster_hex::bench5_128k ... bench:       3,444.01 ns/iter (+/- 17.74)
test encode_to_slice::faster_hex::bench6_1m   ... bench:      29,645.36 ns/iter (+/- 535.00)
test encode_to_slice::hex::bench1_32b         ... bench:          12.08 ns/iter (+/- 0.11)
test encode_to_slice::hex::bench2_256b        ... bench:         119.24 ns/iter (+/- 0.48)
test encode_to_slice::hex::bench3_2k          ... bench:         988.01 ns/iter (+/- 11.35)
test encode_to_slice::hex::bench4_16k         ... bench:       8,044.36 ns/iter (+/- 54.57)
test encode_to_slice::hex::bench5_128k        ... bench:      64,068.07 ns/iter (+/- 954.12)
test encode_to_slice::hex::bench6_1m          ... bench:     517,206.80 ns/iter (+/- 4,775.29)

test format::const_hex::bench1_32b            ... bench:          10.15 ns/iter (+/- 0.14)
test format::const_hex::bench2_256b           ... bench:          17.32 ns/iter (+/- 1.00)
test format::const_hex::bench3_2k             ... bench:         116.15 ns/iter (+/- 5.37)
test format::const_hex::bench4_16k            ... bench:       1,102.71 ns/iter (+/- 6.87)
test format::const_hex::bench5_128k           ... bench:       8,784.66 ns/iter (+/- 108.90)
test format::const_hex::bench6_1m             ... bench:      77,741.10 ns/iter (+/- 2,452.30)
test format::std::bench1_32b                  ... bench:         385.04 ns/iter (+/- 2.50)
test format::std::bench2_256b                 ... bench:       2,979.01 ns/iter (+/- 226.14)
test format::std::bench3_2k                   ... bench:      24,019.65 ns/iter (+/- 118.96)
test format::std::bench4_16k                  ... bench:     200,691.74 ns/iter (+/- 1,243.46)
test format::std::bench5_128k                 ... bench:   1,565,830.30 ns/iter (+/- 96,284.89)
test format::std::bench6_1m                   ... bench:  12,532,954.20 ns/iter (+/- 400,001.89)
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
