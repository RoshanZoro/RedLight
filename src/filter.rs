//! Full-screen red color filter using the Windows Magnification API.
//!
//! `MagSetFullscreenColorEffect` applies a 5x5 color transformation matrix to
//! everything drawn on screen, system-wide. This is the exact mechanism behind
//! Windows' built-in "Color filters" accessibility feature. It transforms the
//! real pixels, so it behaves like wearing tinted glasses: it is automatically
//! click-through and affects every window, video, and game.

use windows::Win32::UI::Magnification::{
    MagInitialize, MagSetFullscreenColorEffect, MagUninitialize, MAGCOLOREFFECT,
};

/// Color matrix (row-major 5x5) that keeps ONLY the red channel.
///
/// The transform computes `[R' G' B' A' 1] = [R G B A 1] * M`. Column 0 (R')
/// gets the input red; columns 1 (G') and 2 (B') are all zero, so green and
/// blue light are removed entirely. The result is pure red light, like a red
/// flashlight / red-glass filter that preserves night vision.
#[rustfmt::skip]
const RED_ONLY: [f32; 25] = [
    1.0, 0.0, 0.0, 0.0, 0.0, // input R -> output R only
    0.0, 0.0, 0.0, 0.0, 0.0, // input G -> dropped
    0.0, 0.0, 0.0, 0.0, 0.0, // input B -> dropped
    0.0, 0.0, 0.0, 1.0, 0.0, // alpha passthrough
    0.0, 0.0, 0.0, 0.0, 1.0,
];

/// Identity matrix: the normal, unfiltered screen.
#[rustfmt::skip]
const IDENTITY: [f32; 25] = [
    1.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 1.0,
];

pub struct RedFilter {
    initialized: bool,
    active: bool,
}

impl RedFilter {
    pub fn new() -> Self {
        let initialized = unsafe { MagInitialize().as_bool() };
        if !initialized {
            eprintln!("warning: MagInitialize failed; the filter will not work");
        }
        Self {
            initialized,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Turn the red filter on or off.
    pub fn set(&mut self, on: bool) {
        if !self.initialized {
            return;
        }
        let effect = MAGCOLOREFFECT {
            transform: if on { RED_ONLY } else { IDENTITY },
        };
        let ok = unsafe { MagSetFullscreenColorEffect(&effect).as_bool() };
        if ok {
            self.active = on;
        } else {
            eprintln!("warning: MagSetFullscreenColorEffect failed");
        }
    }

    /// Restore the normal screen and release the Magnification API.
    ///
    /// Must be called explicitly before exiting, because the tao event loop
    /// terminates the process and never returns, so `Drop` would not run.
    pub fn shutdown(&mut self) {
        if self.initialized {
            self.set(false);
            unsafe {
                let _ = MagUninitialize();
            }
            self.initialized = false;
        }
    }
}

impl Drop for RedFilter {
    fn drop(&mut self) {
        self.shutdown();
    }
}
