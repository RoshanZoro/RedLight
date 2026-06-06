//! Recolor the system mouse cursors to match the red filter.
//!
//! `MagSetFullscreenColorEffect` only transforms the desktop framebuffer. The
//! mouse pointer is a hardware (GPU) overlay that the graphics card composites
//! *after* that transform, during scan-out, so the color matrix never touches
//! it and the pointer stays bright white.
//!
//! To make the pointer obey the filter we replace each standard system cursor
//! with a copy whose pixels have been pushed through the same "red channel
//! only" transform (drop green + blue, keep red + alpha). Turning the filter
//! off reloads the user's normal cursor scheme from the registry.

use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC,
    SetDIBits, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetIconInfo, LoadImageW, SetSystemCursor, SystemParametersInfoW, HCURSOR,
    HICON, ICONINFO, IMAGE_CURSOR, LR_DEFAULTSIZE, LR_SHARED, SPI_SETCURSORS, SYSTEM_CURSOR_ID,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

/// The standard system cursors we recolor. `SetSystemCursor` replaces these
/// system-wide; restoring is a single `SPI_SETCURSORS` call.
const CURSORS: [SYSTEM_CURSOR_ID; 13] = [
    SYSTEM_CURSOR_ID(32512), // OCR_NORMAL    – arrow
    SYSTEM_CURSOR_ID(32513), // OCR_IBEAM     – text select
    SYSTEM_CURSOR_ID(32514), // OCR_WAIT      – hourglass
    SYSTEM_CURSOR_ID(32515), // OCR_CROSS     – crosshair
    SYSTEM_CURSOR_ID(32516), // OCR_UP        – vertical arrow
    SYSTEM_CURSOR_ID(32642), // OCR_SIZENWSE  – diagonal resize
    SYSTEM_CURSOR_ID(32643), // OCR_SIZENESW  – diagonal resize
    SYSTEM_CURSOR_ID(32644), // OCR_SIZEWE    – horizontal resize
    SYSTEM_CURSOR_ID(32645), // OCR_SIZENS    – vertical resize
    SYSTEM_CURSOR_ID(32646), // OCR_SIZEALL   – move
    SYSTEM_CURSOR_ID(32648), // OCR_NO        – unavailable
    SYSTEM_CURSOR_ID(32649), // OCR_HAND      – link select
    SYSTEM_CURSOR_ID(32650), // OCR_APPSTARTING – arrow + hourglass
];

/// Apply (`on == true`) or remove the red cursor tint.
pub fn set_red(on: bool) {
    if on {
        for id in CURSORS {
            unsafe { recolor_one(id) };
        }
    } else {
        restore();
    }
}

/// Reload the user's normal cursor scheme, undoing every `SetSystemCursor`.
pub fn restore() {
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_SETCURSORS,
            0,
            None,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
}

/// Replace one system cursor with a red-filtered copy of itself.
unsafe fn recolor_one(id: SYSTEM_CURSOR_ID) {
    // Load the *current* system cursor for this id (shared handle: do not free).
    let handle = match LoadImageW(
        None,
        PCWSTR(id.0 as usize as *const u16),
        IMAGE_CURSOR,
        0,
        0,
        LR_DEFAULTSIZE | LR_SHARED,
    ) {
        Ok(h) if !h.is_invalid() => h,
        _ => return,
    };

    // GetIconInfo hands us *fresh copies* of the mask + color bitmaps, which we
    // own and must delete.
    let mut info = ICONINFO::default();
    if GetIconInfo(HICON(handle.0), &mut info).is_err() {
        return;
    }

    if info.hbmColor.is_invalid() {
        // Monochrome cursor (e.g. the I-beam / crosshair): no color bitmap,
        // just a 1-bit AND/XOR mask. Synthesize a red color cursor from it.
        install_from_mask(id, &info);
    } else {
        // Color cursor: recolor its pixels in place, then rebuild + install.
        // CreateIconIndirect copies the bitmaps, so we free our copies after.
        recolor_bitmap(info.hbmColor);
        if let Ok(new_cur) = CreateIconIndirect(&info) {
            let _ = SetSystemCursor(HCURSOR(new_cur.0), id);
        }
    }

    // The bitmap copies from GetIconInfo are ours to release.
    if !info.hbmColor.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
    }
    if !info.hbmMask.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
    }
}

/// Build and install a red 32-bit cursor from a monochrome cursor's mask.
///
/// A mono cursor's `hbmMask` is `width` x `2*height`: the top half is the AND
/// mask, the bottom half the XOR mask. The classic per-pixel meaning is:
///   AND=1,XOR=0 → transparent      AND=1,XOR=1 → invert screen
///   AND=0,XOR=0 → black            AND=0,XOR=1 → white
/// We map black→black, white & invert→red, transparent→transparent, producing
/// a 32-bit BGRA cursor whose alpha channel carries the transparency.
unsafe fn install_from_mask(id: SYSTEM_CURSOR_ID, info: &ICONINFO) {
    let mut bmp = BITMAP::default();
    if GetObjectW(
        HGDIOBJ(info.hbmMask.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut c_void),
    ) == 0
    {
        return;
    }
    let width = bmp.bmWidth;
    let full_h = bmp.bmHeight;
    if width <= 0 || full_h <= 0 || full_h % 2 != 0 {
        return;
    }
    let height = full_h / 2;

    let hdc = GetDC(None);
    if hdc.is_invalid() {
        return;
    }

    // Read the 1-bpp mask top-down. BITMAPINFO needs room for a 2-entry palette.
    let header = std::mem::size_of::<BITMAPINFOHEADER>();
    let mut info_buf = vec![0u8; header + 2 * std::mem::size_of::<u32>()];
    let bi = info_buf.as_mut_ptr() as *mut BITMAPINFO;
    (*bi).bmiHeader = BITMAPINFOHEADER {
        biSize: header as u32,
        biWidth: width,
        biHeight: -full_h, // negative: top-down, so AND mask comes first
        biPlanes: 1,
        biBitCount: 1,
        biCompression: BI_RGB.0,
        biClrUsed: 2,
        ..Default::default()
    };

    let stride = (((width + 31) / 32) * 4) as usize; // DIB rows are DWORD-aligned
    let mut mask = vec![0u8; stride * full_h as usize];
    let read = GetDIBits(
        hdc,
        info.hbmMask,
        0,
        full_h as u32,
        Some(mask.as_mut_ptr() as *mut c_void),
        bi,
        DIB_RGB_COLORS,
    );
    if read == 0 {
        ReleaseDC(None, hdc);
        return;
    }

    let bit = |row: i32, x: i32| -> u8 {
        let byte = mask[row as usize * stride + (x / 8) as usize];
        (byte >> (7 - (x % 8))) & 1
    };

    // Allocate a top-down 32-bpp DIB section for the new color bitmap.
    let mut cbi = BITMAPINFO::default();
    cbi.bmiHeader = BITMAPINFOHEADER {
        biSize: header as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let color = match CreateDIBSection(hdc, &cbi, DIB_RGB_COLORS, &mut bits, None, 0) {
        Ok(b) if !b.is_invalid() && !bits.is_null() => b,
        _ => {
            ReleaseDC(None, hdc);
            return;
        }
    };

    let px = std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let and = bit(y, x); // AND mask: rows 0..height
            let xor = bit(y + height, x); // XOR mask: rows height..2*height
            let i = ((y * width + x) * 4) as usize;
            // BGRA
            let (b, g, r, a) = match (and, xor) {
                (1, 0) => (0, 0, 0, 0),       // transparent
                (0, 0) => (0, 0, 0, 255),     // black
                _ => (0, 0, 255, 255),        // white or invert -> red
            };
            px[i] = b;
            px[i + 1] = g;
            px[i + 2] = r;
            px[i + 3] = a;
        }
    }

    // A zeroed AND mask = fully opaque; the 32-bit alpha governs transparency.
    let mono_stride = (((width + 15) / 16) * 2) as usize; // DDB rows are WORD-aligned
    let zero_mask = vec![0u8; mono_stride * height as usize];
    let new_mask = CreateBitmap(width, height, 1, 1, Some(zero_mask.as_ptr() as *const c_void));

    let new_info = ICONINFO {
        fIcon: BOOL(0), // cursor, not icon
        xHotspot: info.xHotspot,
        yHotspot: info.yHotspot,
        hbmMask: new_mask,
        hbmColor: color,
    };
    if let Ok(new_cur) = CreateIconIndirect(&new_info) {
        let _ = SetSystemCursor(HCURSOR(new_cur.0), id);
    }

    // CreateIconIndirect copied the bitmaps; release ours.
    let _ = DeleteObject(HGDIOBJ(color.0));
    if !new_mask.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(new_mask.0));
    }
    ReleaseDC(None, hdc);
}

/// Push a 32-bit cursor color bitmap through the red-only transform in place:
/// keep R and A, zero G and B. Pixels are stored bottom-up as BGRA.
unsafe fn recolor_bitmap(hbm: HBITMAP) {
    let mut bmp = BITMAP::default();
    let got = GetObjectW(
        HGDIOBJ(hbm.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut c_void),
    );
    if got == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
        return;
    }

    let width = bmp.bmWidth;
    let height = bmp.bmHeight;
    let pixels = (width * height) as usize;
    let mut buf = vec![0u8; pixels * 4];

    let mut bi = BITMAPINFO::default();
    bi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: height, // positive: bottom-up; fine for a per-pixel transform
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };

    let hdc = GetDC(None);
    if hdc.is_invalid() {
        return;
    }

    let read = GetDIBits(
        hdc,
        hbm,
        0,
        height as u32,
        Some(buf.as_mut_ptr() as *mut c_void),
        &mut bi,
        DIB_RGB_COLORS,
    );

    if read != 0 {
        // BGRA: index 0 = blue, 1 = green, 2 = red, 3 = alpha.
        for px in buf.chunks_exact_mut(4) {
            px[0] = 0; // blue  -> dropped
            px[1] = 0; // green -> dropped
                       // red and alpha pass through unchanged
        }

        let _ = SetDIBits(
            hdc,
            hbm,
            0,
            height as u32,
            buf.as_ptr() as *const c_void,
            &bi,
            DIB_RGB_COLORS,
        );
    }

    ReleaseDC(None, hdc);
}
