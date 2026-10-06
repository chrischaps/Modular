//! Denormal (subnormal) protection.
//!
//! When a recursive filter or a reverb tail decays towards silence its state
//! eventually drops below `f32::MIN_POSITIVE` (about 1.2e-38). Those tiny
//! "denormal" numbers are handled in microcode on most CPUs and can be 10-100x
//! slower, so a patch that is *quieter* ends up costing *more* CPU.
//!
//! Two layers of defence:
//!
//! - [`DenormalGuard`] sets the CPU's flush-to-zero (FTZ) and
//!   denormals-are-zero (DAZ) modes for the duration of an audio callback.
//!   This covers every module at once, but only on x86_64.
//! - [`flush`] is a portable fallback for feedback loops. It snaps values far
//!   below audibility to exactly zero, so state can never decay into the
//!   denormal range, whatever the platform.

/// Level below which [`flush`] snaps to zero: -300 dBFS, far below anything
/// audible yet far above the denormal range (about -758 dBFS).
const FLUSH_THRESHOLD: f32 = 1e-15;

/// Snaps a feedback-loop value to zero once it is far below audibility.
///
/// Compiles to a compare and a mask (no branch), cheap enough for every sample
/// of every filter state.
///
/// The classic alternative, `(x + tiny) - tiny`, is not used here: rounding
/// makes slowly decaying state get stuck at a small non-zero value instead of
/// reaching silence.
#[inline(always)]
pub fn flush(x: f32) -> f32 {
    if x.abs() < FLUSH_THRESHOLD {
        0.0
    } else {
        x
    }
}

/// Enables flush-to-zero and denormals-are-zero until dropped.
///
/// Create one at the top of the audio callback; the previous floating-point
/// mode is restored when it goes out of scope, so the host thread is left as
/// it was found. On targets other than x86_64 this does nothing, and the
/// [`flush`] fallback in feedback loops does the work instead.
pub struct DenormalGuard {
    #[cfg(target_arch = "x86_64")]
    saved_csr: u32,
}

#[cfg(target_arch = "x86_64")]
mod mxcsr {
    /// MXCSR bit 15: results that would be denormal are written as zero.
    pub const FLUSH_TO_ZERO: u32 = 1 << 15;
    /// MXCSR bit 6: denormal inputs are read as zero.
    pub const DENORMALS_ARE_ZERO: u32 = 1 << 6;

    // `_mm_getcsr`/`_mm_setcsr` are deprecated in favour of inline assembly,
    // but they remain the clearest way to say what is happening here.
    #[allow(deprecated)]
    pub fn get() -> u32 {
        // SAFETY: SSE is part of the x86_64 baseline; reading MXCSR has no
        // side effects.
        unsafe { std::arch::x86_64::_mm_getcsr() }
    }

    #[allow(deprecated)]
    pub fn set(csr: u32) {
        // SAFETY: SSE is part of the x86_64 baseline; only the FTZ/DAZ mode
        // bits are changed, and the guard restores the saved value on drop.
        unsafe { std::arch::x86_64::_mm_setcsr(csr) }
    }
}

impl DenormalGuard {
    /// Turns on FTZ/DAZ for the current thread.
    #[inline]
    pub fn new() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let saved_csr = mxcsr::get();
            mxcsr::set(saved_csr | mxcsr::FLUSH_TO_ZERO | mxcsr::DENORMALS_ARE_ZERO);
            Self { saved_csr }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            Self {}
        }
    }

    /// Whether this platform has hardware denormal flushing.
    pub const fn is_supported() -> bool {
        cfg!(target_arch = "x86_64")
    }
}

impl Default for DenormalGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DenormalGuard {
    #[inline]
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        mxcsr::set(self.saved_csr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    #[test]
    fn test_flush_passes_real_signals() {
        for &x in &[1.0_f32, -0.5, 1e-3, -1e-6, 1e-10] {
            assert_eq!(flush(x), x, "flush must not touch audible values");
        }
    }

    #[test]
    fn test_flush_zeroes_denormals_and_near_denormals() {
        let denormal = f32::MIN_POSITIVE / 4.0;
        assert!(denormal.is_subnormal());
        assert_eq!(flush(black_box(denormal)), 0.0);
        assert_eq!(flush(black_box(-denormal)), 0.0);
        assert_eq!(flush(black_box(1e-30)), 0.0);
        assert_eq!(flush(black_box(-1e-16)), 0.0);
    }

    #[test]
    fn test_decaying_feedback_reaches_exact_zero() {
        // A one-pole decay without protection lingers in the denormal range
        // for thousands of samples; with flush it lands on zero.
        let mut state = 1.0_f32;
        for _ in 0..20_000 {
            state = flush(black_box(state) * 0.99);
            assert!(!state.is_subnormal(), "state went denormal: {:e}", state);
        }
        assert_eq!(state, 0.0);
    }

    #[test]
    fn test_guard_flushes_and_restores() {
        let tiny = black_box(f32::MIN_POSITIVE);
        let before = black_box(tiny) * black_box(0.25);
        assert!(before.is_subnormal(), "default mode keeps denormals");

        {
            let _guard = DenormalGuard::new();
            let during = black_box(tiny) * black_box(0.25);
            if DenormalGuard::is_supported() {
                assert_eq!(during, 0.0, "FTZ should flush denormal results");
            }
        }

        let after = black_box(tiny) * black_box(0.25);
        assert!(after.is_subnormal(), "mode must be restored on drop");
    }
}
