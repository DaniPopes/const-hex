#![allow(unexpected_cfgs)]

use divan::Bencher;
use std::fmt;
use std::hint::black_box;
use std::io::Write;

mod data;

struct HexBufferFormat<const N: usize>(&'static [u8; N]);
impl<const N: usize> fmt::Display for HexBufferFormat<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = const_hex::Buffer::<N>::new();
        f.write_str(buffer.format(self.0))
    }
}

struct StdFormat<const N: usize>(&'static [u8; N]);
impl<const N: usize> fmt::Display for StdFormat<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

macro_rules! benches {
    ($($name:ident($enc:expr, $dec:expr))*) => {
        mod check {
            use super::*;

            mod const_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::const_hex::check_raw(black_box($dec))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod faster_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::faster_hex::hex_check(black_box($dec.as_bytes()))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod naive {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            let dec = black_box($dec.as_bytes());
                            dec.iter().all(u8::is_ascii_hexdigit)
                        });
                    }
                )*
            }
        }

        #[cfg(feature = "alloc")]
        mod decode {
            use super::*;

            mod const_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::const_hex::decode(black_box($dec))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod faster_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            const L: usize = $dec.len() / 2;
                            let mut buf = vec![0; L];
                            ::faster_hex::hex_decode(black_box($dec.as_bytes()), black_box(&mut buf)).unwrap();
                            unsafe { String::from_utf8_unchecked(buf) }
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::hex::decode(black_box($dec))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod rustc_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::rustc_hex::FromHex::from_hex::<Vec<_>>(black_box($dec))
                        });
                    }
                )*
            }
        }

        mod decode_to_slice {
            use super::*;

            mod const_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let buf = &mut [0; $dec.len() / 2];
                        b.bench_local(|| {
                            let res = ::const_hex::decode_to_slice(black_box($dec), black_box(buf));
                            black_box(res.unwrap());
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod faster_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let buf = &mut [0; $dec.len() / 2];
                        b.bench_local(|| {
                            ::faster_hex::hex_decode(black_box($dec.as_bytes()), black_box(buf))
                        })
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let buf = &mut [0; $dec.len() / 2];
                        b.bench_local(|| {
                            ::hex::decode_to_slice(black_box($dec), black_box(buf))
                        });
                    }
                )*
            }
        }

        #[cfg(feature = "alloc")]
        mod encode {
            use super::*;

            mod const_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::const_hex::encode(black_box($enc))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod faster_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::faster_hex::hex_string(black_box($enc))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::hex::encode(black_box($enc))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod rustc_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        b.bench(|| {
                            ::rustc_hex::ToHex::to_hex::<String>(&black_box($enc)[..])
                        });
                    }
                )*
            }
        }

        mod encode_to_slice {
            use super::*;

            mod const_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let buf = &mut [0; $enc.len() * 2];
                        b.bench_local(|| {
                            ::const_hex::encode_to_slice(black_box($enc), black_box(buf))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod faster_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let buf = &mut [0; $enc.len() * 2];
                        b.bench_local(|| {
                            ::faster_hex::hex_encode(black_box($enc), black_box(buf)).map(drop)
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let buf = &mut [0; $enc.len() * 2];
                        b.bench_local(|| {
                            ::hex::encode_to_slice(black_box($enc), black_box(buf))
                        });
                    }
                )*
            }
        }

        mod format {
            use super::*;

            mod const_hex {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let mut buf = Vec::with_capacity($enc.len() * 2);
                        b.bench_local(|| {
                            buf.clear();
                            write!(&mut buf, "{}", HexBufferFormat(black_box($enc)))
                        });
                    }
                )*
            }

            #[cfg(not(codspeed))]
            mod std {
                use super::*;

                $(
                    #[divan::bench]
                    fn $name(b: Bencher) {
                        let mut buf = Vec::with_capacity($enc.len() * 2);
                        b.bench_local(|| {
                            buf.clear();
                            write!(&mut buf, "{}", StdFormat(black_box($enc)))
                        });
                    }
                )*
            }
        }
    }
}

benches! {
    bench1_32b(data::ENC_32, data::DEC_32)
    bench2_256b(data::ENC_256, data::DEC_256)
    bench3_2k(data::ENC_2048, data::DEC_2048)
    bench4_16k(data::ENC_16384, data::DEC_16384)
    bench5_128k(data::ENC_131072, data::DEC_131072)
    bench6_1m(data::ENC_1048576, data::DEC_1048576)
}

fn main() {
    divan::main();
}
