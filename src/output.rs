use core::mem::MaybeUninit;

/// Internal trait for the uninitialized output buffer of the encoders and decoders.
pub(crate) trait Output {
    fn write(&mut self, bytes: &[u8]);

    #[inline]
    fn write_byte(&mut self, byte: u8) {
        self.write(&[byte])
    }

    /// Returns the number of bytes left in the output.
    fn remaining(&self) -> usize;

    /// Returns a pointer to the start of the remaining output.
    ///
    /// Writes through the pointer must stay within the remaining length, and the output must not
    /// be used again while the pointer is in use.
    fn as_mut_ptr(&mut self) -> *mut u8;
}

impl Output for &mut [MaybeUninit<u8>] {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let count = bytes.len();
        let len = self.len();
        debug_assert!(len >= count);
        let ptr = <[MaybeUninit<u8>]>::as_mut_ptr(self);
        // SAFETY: Callers guarantee the output has room for `bytes`, so the copy and the advanced
        // tail stay within the slice. `bytes` cannot overlap the uninitialized output.
        unsafe {
            ptr.copy_from_nonoverlapping(bytes.as_ptr().cast(), count);
            *self = core::slice::from_raw_parts_mut(ptr.add(count), len - count);
        }
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        <[MaybeUninit<u8>]>::as_mut_ptr(self).cast()
    }
}
