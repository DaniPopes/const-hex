#![feature(test)]

extern crate test;

#[rustfmt::skip]
mod data;

use std::fmt;
use std::io::Write;
use test::{black_box, Bencher};

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
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            ::const_hex::check_raw(black_box($dec))
                        });
                    }
                )*
            }

            mod faster_hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            ::faster_hex::hex_check(black_box($dec.as_bytes()))
                        });
                    }
                )*
            }

            mod naive {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
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
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            ::const_hex::decode(black_box($dec))
                        });
                    }
                )*
            }

            mod faster_hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            const L: usize = $dec.len() / 2;
                            let mut buf = vec![0; L];
                            ::faster_hex::hex_decode(black_box($dec.as_bytes()), black_box(&mut buf)).unwrap();
                            unsafe { String::from_utf8_unchecked(buf) }
                        });
                    }
                )*
            }

            mod hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            ::hex::decode(black_box($dec))
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
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let buf = &mut [0; $dec.len() / 2];
                        b.iter(|| {
                            let res = ::const_hex::decode_to_slice(black_box($dec), black_box(buf));
                            black_box(res.unwrap());
                        });
                    }
                )*
            }

            mod faster_hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let buf = &mut [0; $dec.len() / 2];
                        b.iter(|| {
                            ::faster_hex::hex_decode(black_box($dec.as_bytes()), black_box(buf))
                        })
                    }
                )*
            }

            mod hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let buf = &mut [0; $dec.len() / 2];
                        b.iter(|| {
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
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            ::const_hex::encode(black_box($enc))
                        });
                    }
                )*
            }

            mod faster_hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            ::faster_hex::hex_string(black_box($enc))
                        });
                    }
                )*
            }

            mod hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        b.iter(|| {
                            ::hex::encode(black_box($enc))
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
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let buf = &mut [0; $enc.len() * 2];
                        b.iter(|| {
                            ::const_hex::encode_to_slice(black_box($enc), black_box(buf))
                        });
                    }
                )*
            }

            mod faster_hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let buf = &mut [0; $enc.len() * 2];
                        b.iter(|| {
                            ::faster_hex::hex_encode(black_box($enc), black_box(buf)).map(drop)
                        });
                    }
                )*
            }

            mod hex {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let buf = &mut [0; $enc.len() * 2];
                        b.iter(|| {
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
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let mut buf = Vec::with_capacity($enc.len() * 2);
                        b.iter(|| {
                            buf.clear();
                            write!(&mut buf, "{}", HexBufferFormat(black_box($enc)))
                        });
                    }
                )*
            }

            mod std {
                use super::*;

                $(
                    #[bench]
                    fn $name(b: &mut Bencher) {
                        let mut buf = Vec::with_capacity($enc.len() * 2);
                        b.iter(|| {
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
