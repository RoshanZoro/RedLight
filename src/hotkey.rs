//! Global **Alt+F11** trigger via a low-level keyboard hook.
//!
//! The hook must return fast, so it only `PostMessage`s the GUI thread. Plain
//! F11 is deliberately left untouched (so apps' fullscreen toggle still works);
//! only Alt+F11 is consumed and turned into a toggle.

use std::cell::Cell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_MENU};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// Posted to the GUI window when Alt+F11 is pressed.
pub const MSG_TOGGLE: u32 = 0x0400 + 1; // WM_APP + 1
const VK_F11: u32 = 0x7A;

thread_local! {
    static TARGET: Cell<isize> = const { Cell::new(0) };
    static HELD: Cell<bool> = const { Cell::new(false) };
}

static mut HOOK: HHOOK = HHOOK(std::ptr::null_mut());

fn alt_down() -> bool {
    unsafe { (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0 }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode;
        let msg = wparam.0 as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

        if vk == VK_F11 && alt_down() {
            if is_down {
                if !HELD.with(|h| h.get()) {
                    HELD.with(|h| h.set(true));
                    let target = TARGET.with(|t| t.get());
                    if target != 0 {
                        let _ = PostMessageW(
                            HWND(target as *mut _),
                            MSG_TOGGLE,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
                return LRESULT(1); // consume Alt+F11
            } else if is_up {
                HELD.with(|h| h.set(false));
                return LRESULT(1);
            }
        } else if vk == VK_F11 && is_up {
            HELD.with(|h| h.set(false));
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Install the keyboard hook. Call once, on the GUI thread.
pub fn install(target_hwnd: isize) {
    TARGET.with(|t| t.set(target_hwnd));
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hinstance, 0) {
            Ok(h) => HOOK = h,
            Err(e) => eprintln!("warning: failed to install keyboard hook: {e}"),
        }
    }
}

/// Remove the hook. Safe to call on shutdown.
pub fn uninstall() {
    unsafe {
        if !HOOK.0.is_null() {
            let _ = UnhookWindowsHookEx(HOOK);
            HOOK = HHOOK(std::ptr::null_mut());
        }
    }
}
