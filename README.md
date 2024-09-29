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
test check::const_hex::bench1_32b             ... bench:           2.85 ns/iter (+/- 0.44)
test check::const_hex::bench2_256b            ... bench:          15.36 ns/iter (+/- 0.44)
test check::const_hex::bench3_2k              ... bench:         117.55 ns/iter (+/- 1.72)
test check::const_hex::bench4_16k             ... bench:         915.78 ns/iter (+/- 56.34)
test check::const_hex::bench5_128k            ... bench:       7,269.26 ns/iter (+/- 80.62)
test check::const_hex::bench6_1m              ... bench:      58,975.63 ns/iter (+/- 707.02)
test check::faster_hex::bench1_32b            ... bench:           2.70 ns/iter (+/- 0.01)
test check::faster_hex::bench2_256b           ... bench:          14.45 ns/iter (+/- 1.44)
test check::faster_hex::bench3_2k             ... bench:         123.58 ns/iter (+/- 1.09)
test check::faster_hex::bench4_16k            ... bench:         960.32 ns/iter (+/- 6.34)
test check::faster_hex::bench5_128k           ... bench:       7,709.14 ns/iter (+/- 77.69)
test check::faster_hex::bench6_1m             ... bench:      62,165.54 ns/iter (+/- 1,167.78)
test check::naive::bench1_32b                 ... bench:          16.41 ns/iter (+/- 1.90)
test check::naive::bench2_256b                ... bench:         221.25 ns/iter (+/- 3.47)
test check::naive::bench3_2k                  ... bench:       2,493.23 ns/iter (+/- 154.04)
test check::naive::bench4_16k                 ... bench:      16,221.81 ns/iter (+/- 384.98)
test check::naive::bench5_128k                ... bench:     485,588.59 ns/iter (+/- 10,260.00)
test check::naive::bench6_1m                  ... bench:   3,895,089.20 ns/iter (+/- 45,589.05)

test decode::const_hex::bench1_32b            ... bench:          17.33 ns/iter (+/- 0.38)
test decode::const_hex::bench2_256b           ... bench:          38.17 ns/iter (+/- 1.07)
test decode::const_hex::bench3_2k             ... bench:         235.07 ns/iter (+/- 3.27)
test decode::const_hex::bench4_16k            ... bench:       1,681.14 ns/iter (+/- 17.25)
test decode::const_hex::bench5_128k           ... bench:      13,097.65 ns/iter (+/- 101.14)
test decode::const_hex::bench6_1m             ... bench:     105,945.60 ns/iter (+/- 2,703.49)
test decode::faster_hex::bench1_32b           ... bench:          17.91 ns/iter (+/- 0.40)
test decode::faster_hex::bench2_256b          ... bench:          54.53 ns/iter (+/- 1.41)
test decode::faster_hex::bench3_2k            ... bench:         245.35 ns/iter (+/- 3.89)
test decode::faster_hex::bench4_16k           ... bench:       1,836.62 ns/iter (+/- 25.01)
test decode::faster_hex::bench5_128k          ... bench:      14,471.53 ns/iter (+/- 184.29)
test decode::faster_hex::bench6_1m            ... bench:     116,688.27 ns/iter (+/- 1,539.72)
test decode::hex::bench1_32b                  ... bench:         109.14 ns/iter (+/- 1.88)
test decode::hex::bench2_256b                 ... bench:         712.92 ns/iter (+/- 14.25)
test decode::hex::bench3_2k                   ... bench:       5,196.66 ns/iter (+/- 102.67)
test decode::hex::bench4_16k                  ... bench:      41,308.30 ns/iter (+/- 917.60)
test decode::hex::bench5_128k                 ... bench:     786,648.00 ns/iter (+/- 6,589.60)
test decode::hex::bench6_1m                   ... bench:   6,316,271.50 ns/iter (+/- 22,712.18)

test decode_to_slice::const_hex::bench1_32b   ... bench:           5.14 ns/iter (+/- 0.39)
test decode_to_slice::const_hex::bench2_256b  ... bench:          26.18 ns/iter (+/- 0.26)
test decode_to_slice::const_hex::bench3_2k    ... bench:         206.71 ns/iter (+/- 1.88)
test decode_to_slice::const_hex::bench4_16k   ... bench:       1,666.49 ns/iter (+/- 15.67)
test decode_to_slice::const_hex::bench5_128k  ... bench:      12,979.03 ns/iter (+/- 80.40)
test decode_to_slice::const_hex::bench6_1m    ... bench:     107,213.20 ns/iter (+/- 4,024.91)
test decode_to_slice::faster_hex::bench1_32b  ... bench:           6.51 ns/iter (+/- 0.04)
test decode_to_slice::faster_hex::bench2_256b ... bench:          28.66 ns/iter (+/- 0.29)
test decode_to_slice::faster_hex::bench3_2k   ... bench:         217.84 ns/iter (+/- 1.03)
test decode_to_slice::faster_hex::bench4_16k  ... bench:       1,730.89 ns/iter (+/- 19.36)
test decode_to_slice::faster_hex::bench5_128k ... bench:      13,439.63 ns/iter (+/- 92.19)
test decode_to_slice::faster_hex::bench6_1m   ... bench:     109,432.90 ns/iter (+/- 1,526.01)
test decode_to_slice::hex::bench1_32b         ... bench:          38.44 ns/iter (+/- 1.30)
test decode_to_slice::hex::bench2_256b        ... bench:         290.78 ns/iter (+/- 16.09)
test decode_to_slice::hex::bench3_2k          ... bench:       2,663.51 ns/iter (+/- 48.22)
test decode_to_slice::hex::bench4_16k         ... bench:      19,016.95 ns/iter (+/- 514.35)
test decode_to_slice::hex::bench5_128k        ... bench:     612,840.31 ns/iter (+/- 6,561.70)
test decode_to_slice::hex::bench6_1m          ... bench:   5,098,572.75 ns/iter (+/- 120,113.94)

test encode::const_hex::bench1_32b            ... bench:           6.94 ns/iter (+/- 0.06)
test encode::const_hex::bench2_256b           ... bench:          11.84 ns/iter (+/- 0.07)
test encode::const_hex::bench3_2k             ... bench:          78.36 ns/iter (+/- 0.89)
test encode::const_hex::bench4_16k            ... bench:         475.29 ns/iter (+/- 11.56)
test encode::const_hex::bench5_128k           ... bench:       3,577.27 ns/iter (+/- 70.48)
test encode::const_hex::bench6_1m             ... bench:      29,996.00 ns/iter (+/- 668.44)
test encode::faster_hex::bench1_32b           ... bench:          17.31 ns/iter (+/- 0.37)
test encode::faster_hex::bench2_256b          ... bench:          39.39 ns/iter (+/- 0.76)
test encode::faster_hex::bench3_2k            ... bench:         106.60 ns/iter (+/- 1.41)
test encode::faster_hex::bench4_16k           ... bench:         653.21 ns/iter (+/- 5.40)
test encode::faster_hex::bench5_128k          ... bench:       5,260.68 ns/iter (+/- 88.46)
test encode::faster_hex::bench6_1m            ... bench:      44,520.36 ns/iter (+/- 1,200.74)
test encode::hex::bench1_32b                  ... bench:         102.77 ns/iter (+/- 0.82)
test encode::hex::bench2_256b                 ... bench:         720.90 ns/iter (+/- 22.52)
test encode::hex::bench3_2k                   ... bench:       5,672.44 ns/iter (+/- 287.53)
test encode::hex::bench4_16k                  ... bench:      38,988.71 ns/iter (+/- 6,457.99)
test encode::hex::bench5_128k                 ... bench:     364,376.25 ns/iter (+/- 51,416.85)
test encode::hex::bench6_1m                   ... bench:   2,959,499.88 ns/iter (+/- 410,006.38)

test encode_to_slice::const_hex::bench1_32b   ... bench:           1.56 ns/iter (+/- 0.00)
test encode_to_slice::const_hex::bench2_256b  ... bench:           6.75 ns/iter (+/- 0.03)
test encode_to_slice::const_hex::bench3_2k    ... bench:          58.32 ns/iter (+/- 0.23)
test encode_to_slice::const_hex::bench4_16k   ... bench:         518.24 ns/iter (+/- 4.91)
test encode_to_slice::const_hex::bench5_128k  ... bench:       4,003.77 ns/iter (+/- 28.57)
test encode_to_slice::const_hex::bench6_1m    ... bench:      34,519.64 ns/iter (+/- 656.35)
test encode_to_slice::faster_hex::bench1_32b  ... bench:           4.54 ns/iter (+/- 0.01)
test encode_to_slice::faster_hex::bench2_256b ... bench:           8.11 ns/iter (+/- 0.05)
test encode_to_slice::faster_hex::bench3_2k   ... bench:          52.10 ns/iter (+/- 0.64)
test encode_to_slice::faster_hex::bench4_16k  ... bench:         475.81 ns/iter (+/- 6.50)
test encode_to_slice::faster_hex::bench5_128k ... bench:       3,425.49 ns/iter (+/- 15.01)
test encode_to_slice::faster_hex::bench6_1m   ... bench:      28,725.82 ns/iter (+/- 839.95)
test encode_to_slice::hex::bench1_32b         ... bench:          12.01 ns/iter (+/- 0.12)
test encode_to_slice::hex::bench2_256b        ... bench:         121.68 ns/iter (+/- 1.98)
test encode_to_slice::hex::bench3_2k          ... bench:         989.33 ns/iter (+/- 3.83)
test encode_to_slice::hex::bench4_16k         ... bench:       8,087.93 ns/iter (+/- 25.25)
test encode_to_slice::hex::bench5_128k        ... bench:      64,323.94 ns/iter (+/- 249.97)
test encode_to_slice::hex::bench6_1m          ... bench:     515,710.80 ns/iter (+/- 2,232.59)

test format::const_hex::bench1_32b            ... bench:          10.26 ns/iter (+/- 0.19)
test format::const_hex::bench2_256b           ... bench:          18.28 ns/iter (+/- 0.86)
test format::const_hex::bench3_2k             ... bench:         116.95 ns/iter (+/- 2.17)
test format::const_hex::bench4_16k            ... bench:       1,122.29 ns/iter (+/- 6.25)
test format::const_hex::bench5_128k           ... bench:       8,903.81 ns/iter (+/- 111.29)
test format::const_hex::bench6_1m             ... bench:      77,476.15 ns/iter (+/- 1,498.93)
test format::std::bench1_32b                  ... bench:         370.27 ns/iter (+/- 2.52)
test format::std::bench2_256b                 ... bench:       2,910.66 ns/iter (+/- 40.35)
test format::std::bench3_2k                   ... bench:      22,554.52 ns/iter (+/- 263.93)
test format::std::bench4_16k                  ... bench:     182,692.06 ns/iter (+/- 3,494.64)
test format::std::bench5_128k                 ... bench:   1,475,988.90 ns/iter (+/- 21,895.90)
test format::std::bench6_1m                   ... bench:  11,834,234.60 ns/iter (+/- 139,230.20)
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
