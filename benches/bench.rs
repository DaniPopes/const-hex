#![feature(test)]

use std::fmt;
use std::io::Write;
use test::{black_box, Bencher};

extern crate test;

struct HexBufferFormat<const N: usize>([u8; N]);
impl<const N: usize> fmt::Display for HexBufferFormat<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = const_hex::Buffer::new();
        f.write_str(buffer.format(&self.0))
    }
}

struct StdFormat<const N: usize>([u8; N]);
impl<const N: usize> fmt::Display for StdFormat<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

macro_rules! benches {
    ($($name:ident($value:expr))*) => {
        mod bench_const_hex_format {
            use super::*;

            $(
                #[bench]
                fn $name(b: &mut Bencher) {
                    let mut buf = Vec::with_capacity($value.len() * 2);

                    b.iter(|| {
                        buf.clear();
                        write!(&mut buf, "{}", HexBufferFormat(black_box($value))).unwrap();
                        black_box(&buf);
                    });
                }
            )*
        }

        mod bench_std_fmt {
            use super::*;

            $(
                #[bench]
                fn $name(b: &mut Bencher) {
                    let mut buf = Vec::with_capacity($value.len() * 2);

                    b.iter(|| {
                        buf.clear();
                        write!(&mut buf, "{}", StdFormat(black_box($value))).unwrap();
                        black_box(&buf);
                    });
                }
            )*
        }
    }
}

benches! {
    bench1_20([0u8; 20])
    bench2_32([0u8; 32])
    bench3_128([0u8; 128])
}
