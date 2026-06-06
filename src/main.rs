// Hide the console window in release builds; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use native_windows_gui as nwg;

mod autostart;
mod config;
mod filter;
mod hotkey;

use std::cell::RefCell;
use std::rc::Rc;

use config::Config;
use filter::RedFilter;

/// Tray-only app: a hidden message window hosts the tray icon + menu and
/// receives the Alt+F11 toggle posted by the keyboard hook.
struct App {
    #[allow(dead_code)]
    window: nwg::MessageWindow,
    // Kept alive so the tray icon handle isn't dropped.
    #[allow(dead_code)]
    icon: nwg::Icon,
    tray: nwg::TrayNotification,
    menu: nwg::Menu,
    m_filter: nwg::MenuItem,
    m_auto: nwg::MenuItem,
    m_boot: nwg::MenuItem,
    m_quit: nwg::MenuItem,

    filter: RefCell<RedFilter>,
    config: RefCell<Config>,
}

impl App {
    /// Apply a filter state and reflect it in the menu + tooltip.
    fn set_filter(&self, on: bool) {
        self.filter.borrow_mut().set(on);
        self.m_filter.set_checked(on);
        let _ = self.tray.set_tip(&format!(
            "RedLight — {}  (Alt+F11)",
            if on { "ON" } else { "OFF" }
        ));
    }

    fn toggle(&self) {
        let on = !self.filter.borrow().is_active();
        self.set_filter(on);
    }
}

fn main() {
    install_crash_logger();
    nwg::init().expect("Failed to init native-windows-gui");

    let mut cfg = Config::load();
    cfg.start_on_boot = autostart::is_enabled();

    // Icon (generated in code, no asset file).
    let mut icon = nwg::Icon::default();
    nwg::Icon::builder()
        .source_bin(Some(&make_icon_bin()))
        .build(&mut icon)
        .expect("icon");

    // Hidden message-only window to host the tray + receive hook messages.
    let mut window = nwg::MessageWindow::default();
    nwg::MessageWindow::builder()
        .build(&mut window)
        .expect("window");
    let hwnd_isize = window.handle.hwnd().map(|h| h as isize).unwrap_or(0);

    // Tray icon.
    let mut tray = nwg::TrayNotification::default();
    nwg::TrayNotification::builder()
        .parent(&window)
        .icon(Some(&icon))
        .tip(Some("RedLight"))
        .build(&mut tray)
        .expect("tray");

    // Tray context menu.
    let mut menu = nwg::Menu::default();
    nwg::Menu::builder()
        .popup(true)
        .parent(&window)
        .build(&mut menu)
        .expect("menu");

    let mut m_filter = nwg::MenuItem::default();
    nwg::MenuItem::builder()
        .text("Red filter")
        .parent(&menu)
        .build(&mut m_filter)
        .expect("m_filter");

    let mut sep1 = nwg::MenuSeparator::default();
    nwg::MenuSeparator::builder()
        .parent(&menu)
        .build(&mut sep1)
        .expect("sep1");

    let mut m_auto = nwg::MenuItem::default();
    nwg::MenuItem::builder()
        .text("Turn on at launch")
        .parent(&menu)
        .build(&mut m_auto)
        .expect("m_auto");

    let mut m_boot = nwg::MenuItem::default();
    nwg::MenuItem::builder()
        .text("Start with Windows")
        .parent(&menu)
        .build(&mut m_boot)
        .expect("m_boot");

    let mut sep2 = nwg::MenuSeparator::default();
    nwg::MenuSeparator::builder()
        .parent(&menu)
        .build(&mut sep2)
        .expect("sep2");

    let mut m_quit = nwg::MenuItem::default();
    nwg::MenuItem::builder()
        .text("Quit")
        .parent(&menu)
        .build(&mut m_quit)
        .expect("m_quit");

    // Install the Alt+F11 keyboard hook (posts MSG_TOGGLE to our window).
    hotkey::install(hwnd_isize);

    let app = Rc::new(App {
        window,
        icon,
        tray,
        menu,
        m_filter,
        m_auto,
        m_boot,
        m_quit,
        filter: RefCell::new(RedFilter::new()),
        config: RefCell::new(cfg),
    });

    // Initial state.
    app.m_auto.set_checked(app.config.borrow().auto_on_start);
    app.m_boot.set_checked(app.config.borrow().start_on_boot);
    app.set_filter(app.config.borrow().auto_on_start);

    // Alt+F11 toggle arrives as a posted window message.
    let raw_app = Rc::clone(&app);
    let _raw = nwg::bind_raw_event_handler(&app.window.handle, 0x0052_4544, move |_h, msg, _w, _l| {
        if msg == hotkey::MSG_TOGGLE {
            raw_app.toggle();
        }
        None
    })
    .ok();

    // Tray + menu events.
    let h_app = Rc::clone(&app);
    let _handler = nwg::full_bind_event_handler(&app.window.handle, move |evt, _data, handle| {
        use nwg::Event as E;
        let app = &h_app;
        match evt {
            E::OnContextMenu => {
                let (x, y) = nwg::GlobalCursor::position();
                app.menu.popup(x, y);
            }
            E::OnMenuItemSelected => {
                if handle == app.m_filter.handle {
                    app.toggle();
                } else if handle == app.m_auto.handle {
                    let v = !app.config.borrow().auto_on_start;
                    app.config.borrow_mut().auto_on_start = v;
                    app.config.borrow().save();
                    app.m_auto.set_checked(v);
                } else if handle == app.m_boot.handle {
                    let v = !app.config.borrow().start_on_boot;
                    match autostart::set(v) {
                        Ok(()) => {
                            app.config.borrow_mut().start_on_boot = v;
                            app.config.borrow().save();
                            app.m_boot.set_checked(v);
                        }
                        Err(e) => eprintln!("autostart failed: {e}"),
                    }
                } else if handle == app.m_quit.handle {
                    app.filter.borrow_mut().shutdown();
                    hotkey::uninstall();
                    nwg::stop_thread_dispatch();
                }
            }
            _ => {}
        }
    });

    nwg::dispatch_thread_events();

    app.filter.borrow_mut().shutdown();
    hotkey::uninstall();
}

/// Write any panic to a crash log (release has no console).
fn install_crash_logger() {
    std::panic::set_hook(Box::new(|info| {
        if let Some(dir) = dirs::config_dir() {
            let path = dir.join("RedLight");
            let _ = std::fs::create_dir_all(&path);
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path.join("crash.log"))
            {
                let _ = writeln!(f, "PANIC: {info}");
            }
        }
    }));
}

/// Build a 32x32 red circle icon as in-memory `.ico` bytes (no asset file).
fn make_icon_bin() -> Vec<u8> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let center = (SIZE as f32 - 1.0) / 2.0;
    let radius = center;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                let i = ((y * SIZE + x) * 4) as usize;
                rgba[i] = 220;
                rgba[i + 1] = 20;
                rgba[i + 2] = 20;
                rgba[i + 3] = if dist > radius - 1.5 { 180 } else { 255 };
            }
        }
    }
    rgba_to_ico(&rgba, SIZE)
}

/// Wrap a 32-bit RGBA buffer in a single-image Windows `.ico` container.
fn rgba_to_ico(rgba: &[u8], size: u32) -> Vec<u8> {
    let s = size as usize;
    let xor_size = s * s * 4;
    let and_row = ((s + 31) / 32) * 4;
    let and_size = and_row * s;
    let image_size = 40 + xor_size + and_size;

    let mut v = Vec::with_capacity(22 + image_size);
    v.extend_from_slice(&0u16.to_le_bytes()); // reserved
    v.extend_from_slice(&1u16.to_le_bytes()); // type = icon
    v.extend_from_slice(&1u16.to_le_bytes()); // image count
    v.push(size as u8);
    v.push(size as u8);
    v.push(0);
    v.push(0);
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&32u16.to_le_bytes());
    v.extend_from_slice(&(image_size as u32).to_le_bytes());
    v.extend_from_slice(&22u32.to_le_bytes());
    v.extend_from_slice(&40u32.to_le_bytes());
    v.extend_from_slice(&(size as i32).to_le_bytes());
    v.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&32u16.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&0i32.to_le_bytes());
    v.extend_from_slice(&0i32.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..s).rev() {
        for x in 0..s {
            let i = (y * s + x) * 4;
            v.push(rgba[i + 2]);
            v.push(rgba[i + 1]);
            v.push(rgba[i]);
            v.push(rgba[i + 3]);
        }
    }
    v.resize(v.len() + and_size, 0);
    v
}
