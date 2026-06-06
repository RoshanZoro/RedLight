# RedLight

A lightweight Windows system-tray utility that applies a full-screen **red
filter** — only the red channel is shown, with green and blue removed. This
mimics a red light environment and helps preserve night-adapted vision.

The filter is applied with the Windows Magnification API
(`MagSetFullscreenColorEffect`), the same system facility used by the built-in
"Color filters" accessibility feature. It transforms the actual displayed
pixels, so it affects every application, video, and game, and is fully
click-through (it is not a translucent overlay).

## Features

- Full-screen red filter, toggled instantly.
- Global hotkey: **Alt + F11**.
- System-tray icon with a context menu:
  - **Red filter** — turn the filter on or off.
  - **Turn on at launch** — apply the filter automatically when the app starts.
  - **Start with Windows** — launch automatically at sign-in.
  - **Quit** — restore the screen and exit.
- No window; runs entirely from the tray.
- Settings are stored in `%APPDATA%\RedLight\config.json`.

## Usage

Run `RedLight.exe`. A red icon appears in the system tray.

- Press **Alt + F11** at any time to toggle the filter.
- Right-click the tray icon for options.

Plain **F11** is intentionally left untouched so application shortcuts (such as
fullscreen) continue to work normally; only **Alt + F11** is handled.

## Requirements

- Windows 8 or later (required for the full-screen color effect).
- No administrator rights required.

Note: only one full-screen color effect can be active at a time. This conflicts
with the Windows *Settings → Accessibility → Color filters* feature; leave that
turned off while using RedLight.

## Building

Built with Rust using the GNU (MinGW) toolchain. With Rust and MinGW-w64
installed:

```powershell
cargo build --release
```

The output is `target\release\RedLight.exe`, a standalone executable with no
console window.

## License

MIT
