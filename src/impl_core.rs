//! Modified implementations of unstable libcore functions.

#![allow(dead_code)]

use core::mem::{self, MaybeUninit};

/// Reinterprets `&mut [T]` as `&mut [MaybeUninit<T>]`.
///
/// This is safe because `MaybeUninit<T>` is guaranteed to have the same layout as `T`,
/// and an initialized `T` is always a valid `MaybeUninit<T>`.
#[inline(always)]
pub(crate) fn slice_as_uninit_mut<T>(slice: &mut [T]) -> &mut [MaybeUninit<T>] {
    // SAFETY: `MaybeUninit<T>` has the same layout as `T`, and initialized
    // memory is valid `MaybeUninit`.
    unsafe { &mut *(slice as *mut [T] as *mut [MaybeUninit<T>]) }
}

/// `MaybeUninit::uninit_array`
#[inline]
pub(crate) const fn uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
    // SAFETY: An uninitialized `[MaybeUninit<_>; N]` is valid.
    unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() }
}

/// `MaybeUninit::array_assume_init`
#[inline]
pub(crate) unsafe fn array_assume_init<T, const N: usize>(array: [MaybeUninit<T>; N]) -> [T; N] {
    // SAFETY:
    // * The caller guarantees that all elements of the array are initialized
    // * `MaybeUninit<T>` and T are guaranteed to have the same layout
    // * `MaybeUninit` does not drop, so there are no double-frees
    // And thus the conversion is safe
    unsafe { transpose(array).assume_init() }
}

/// `MaybeUninit::transpose`
#[inline(always)]
unsafe fn transpose<T, const N: usize>(array: [MaybeUninit<T>; N]) -> MaybeUninit<[T; N]> {
    unsafe {
        mem::transmute_copy::<[MaybeUninit<T>; N], MaybeUninit<[T; N]>>(&mem::ManuallyDrop::new(
            &array,
        ))
    }
}
