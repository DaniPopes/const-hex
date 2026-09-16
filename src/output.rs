use core::fmt::{self, Write};
use core::mem::MaybeUninit;

/// Internal trait for abstracting over output buffer types.
pub(crate) trait Output {
    fn write(&mut self, bytes: &[u8]);

    #[inline]
    fn write_byte(&mut self, byte: u8) {
        self.write(&[byte])
    }

    #[inline]
    fn remaining(&self) -> Option<usize> {
        None
    }
}

impl<O: Output + ?Sized> Output for &mut O {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        (**self).write(bytes);
    }

    #[inline]
    fn write_byte(&mut self, byte: u8) {
        (**self).write_byte(byte);
    }

    #[inline]
    fn remaining(&self) -> Option<usize> {
        (**self).remaining()
    }
}

impl Output for &mut [u8] {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let this = crate::impl_core::slice_as_uninit_mut(self);
        unsafe {
            let count = write_bytes_output_slice(this, bytes);
            advance_slice(self, count);
        }
    }

    #[inline]
    fn remaining(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl Output for &mut [MaybeUninit<u8>] {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        unsafe {
            let count = write_bytes_output_slice(self, bytes);
            advance_slice(self, count);
        }
    }

    #[inline]
    fn remaining(&self) -> Option<usize> {
        Some(self.len())
    }
}

/// Batches encoder writes into a fixed-size stack buffer.
///
/// Like `BufWriter`, writes at least as large as the buffer bypass it after
/// flushing pending bytes. Call `finish` explicitly; dropping does not flush.
/// In debug builds, dropping with pending bytes panics. With `std`, this check
/// is skipped during unwinding to avoid a double panic.
#[cfg(feature = "serde")]
pub(crate) struct BufferedOutput<O, const N: usize> {
    output: O,
    buffer: [MaybeUninit<u8>; N],
    // Always <= N; buffer[..len] is initialized.
    len: usize,
}

#[cfg(feature = "serde")]
impl<O: Output, const N: usize> BufferedOutput<O, N> {
    #[inline]
    pub(crate) fn new(output: O) -> Self {
        assert!(N > 0, "output buffer must not be empty");
        Self {
            output,
            buffer: crate::impl_core::uninit_array(),
            len: 0,
        }
    }

    #[inline]
    fn flush(&mut self) {
        if self.len != 0 {
            // SAFETY: len <= N, and writes initialize every byte in buffer[..len].
            let bytes = unsafe {
                crate::impl_core::slice_assume_init(self.buffer.get_unchecked(..self.len))
            };
            // Disarm the debug drop check if the output panics, including without std.
            #[cfg(debug_assertions)]
            {
                self.len = 0;
            }
            self.output.write(bytes);
            self.len = 0;
        }
    }

    #[inline]
    pub(crate) fn finish(mut self) {
        self.flush();
    }
}

#[cfg(all(feature = "serde", debug_assertions))]
impl<O, const N: usize> Drop for BufferedOutput<O, N> {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "std")]
        if std::thread::panicking() {
            return;
        }
        debug_assert_eq!(self.len, 0, "BufferedOutput dropped without finish()");
    }
}

#[cfg(feature = "serde")]
impl<O: Output, const N: usize> Output for BufferedOutput<O, N> {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() > N - self.len {
            self.flush();
        }
        if bytes.len() >= N {
            self.output.write(bytes);
        } else {
            // SAFETY: len <= N, and the flush above ensures there is room for this write.
            unsafe { write_bytes_output_slice(self.buffer.get_unchecked_mut(self.len..), bytes) };
            self.len += bytes.len();
        }
    }

    #[inline]
    fn write_byte(&mut self, byte: u8) {
        if self.len == N {
            self.flush();
        }
        // SAFETY: N > 0 and len <= N; flushing a full buffer resets len to zero.
        unsafe { self.buffer.get_unchecked_mut(self.len).write(byte) };
        self.len += 1;
    }
}

/// Wraps a [`fmt::Formatter`] to capture the first write error.
///
/// [`Output::write`] is infallible because the buffer outputs cannot fail, so
/// errors from the formatter are recorded here and surfaced by [`Self::finish`]
/// once encoding is done. Writes after the first error are skipped.
pub(crate) struct FormatterOutput<'a, 'b> {
    f: &'a mut fmt::Formatter<'b>,
    result: fmt::Result,
}

impl<'a, 'b> FormatterOutput<'a, 'b> {
    #[inline]
    pub(crate) fn new(f: &'a mut fmt::Formatter<'b>) -> Self {
        Self { f, result: Ok(()) }
    }

    /// Returns the first error encountered while writing, if any.
    #[inline]
    pub(crate) const fn finish(&self) -> fmt::Result {
        self.result
    }
}

impl Output for FormatterOutput<'_, '_> {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if self.result.is_err() {
            return;
        }
        if cfg!(debug_assertions) {
            core::str::from_utf8(bytes).unwrap();
        }
        self.result = self
            .f
            .write_str(unsafe { core::str::from_utf8_unchecked(bytes) });
    }

    #[inline]
    fn write_byte(&mut self, byte: u8) {
        if self.result.is_err() {
            return;
        }
        self.result = self.f.write_char(byte as char);
    }
}

/// # Safety
///
/// Caller must guarantee `output.len() >= bytes.len()`.
#[inline(always)]
unsafe fn write_bytes_output_slice(output: &mut [MaybeUninit<u8>], bytes: &[u8]) -> usize {
    let src = bytes.as_ptr().cast::<MaybeUninit<u8>>();
    let dst = output.as_mut_ptr();
    let count = bytes.len();
    debug_assert!(output.len() >= count);
    // SAFETY: Caller guarantees `output` is at least `count` bytes long.
    unsafe { dst.copy_from_nonoverlapping(src, count) };
    count
}

/// Safety: Caller must guarantee `slice` is long enough, and that `slice` is not concurrently accessed.
#[inline(always)]
unsafe fn advance_slice<T>(slice: &mut &mut [T], count: usize) {
    debug_assert!(slice.len() >= count);
    let len = slice.len();
    let ptr = slice.as_mut_ptr();
    // SAFETY: Caller must guarantee `slice` is long enough, and that `slice` is not concurrently accessed.
    *slice = core::slice::from_raw_parts_mut(ptr.add(count), len - count);
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(debug_assertions))]
    fn buffered_output_has_no_release_drop_glue() {
        assert!(!core::mem::needs_drop::<BufferedOutput<&mut [u8], 4>>());
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "BufferedOutput dropped without finish()")]
    fn buffered_output_requires_finish_with_pending_bytes() {
        let mut bytes = [0; 4];
        let mut output = BufferedOutput::<_, 4>::new(bytes.as_mut_slice());
        output.write_byte(b'a');
    }

    #[test]
    #[cfg(feature = "std")]
    fn buffered_output_does_not_double_panic() {
        assert!(std::panic::catch_unwind(|| {
            let mut bytes = [0; 4];
            let mut output = BufferedOutput::<_, 4>::new(bytes.as_mut_slice());
            output.write_byte(b'a');
            panic!("original panic");
        })
        .is_err());
    }

    #[test]
    fn buffered_output_preserves_order() {
        // Include a capacity smaller than a SIMD fragment and a one-byte buffer.
        fn check<const N: usize>() {
            let mut bytes = [0; 12];
            let mut output = BufferedOutput::<_, N>::new(bytes.as_mut_slice());
            output.write_byte(b'a');
            output.write(b"bc");
            output.write(b"defghijk");
            output.write_byte(b'l');
            output.write(b"");
            output.finish();
            assert_eq!(&bytes, b"abcdefghijkl");
        }
        check::<1>();
        check::<4>();
        check::<8>();
        check::<64>();
    }

    #[test]
    fn buffered_output_write_boundaries() {
        fn check<const N: usize>() {
            for first in 0..=16 {
                for second in 0..=16 {
                    let mut bytes = [0; 33];
                    let mut output = BufferedOutput::<_, N>::new(bytes.as_mut_slice());
                    output.write(&[b'a'; 16][..first]);
                    output.write(b"");
                    output.write(&[b'b'; 16][..second]);
                    output.write_byte(b'c');
                    output.finish();
                    assert_eq!(&bytes[..first], &[b'a'; 16][..first]);
                    assert_eq!(&bytes[first..first + second], &[b'b'; 16][..second]);
                    assert_eq!(bytes[first + second], b'c');
                    assert!(bytes[first + second + 1..].iter().all(|&b| b == 0));
                }
            }
        }
        check::<1>();
        check::<2>();
        check::<3>();
        check::<4>();
        check::<8>();
        check::<16>();
        check::<64>();
    }
}
