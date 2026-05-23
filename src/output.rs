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

impl Output for &mut fmt::Formatter<'_> {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if cfg!(debug_assertions) {
            core::str::from_utf8(bytes).unwrap();
        }
        let _ = self.write_str(unsafe { core::str::from_utf8_unchecked(bytes) });
    }

    #[inline]
    fn write_byte(&mut self, byte: u8) {
        let _ = self.write_char(byte as char);
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
