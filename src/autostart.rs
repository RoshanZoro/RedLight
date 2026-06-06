//! Start-with-Windows support via the per-user Run registry key.

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "RedLight";

fn exe_command() -> std::io::Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\"", exe.display()))
}

/// Enable or disable launching RedLight when the current user logs in.
pub fn set(enable: bool) -> std::io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if enable {
        let (key, _) = hkcu.create_subkey(RUN_KEY)?;
        key.set_value(VALUE_NAME, &exe_command()?)?;
    } else {
        if let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE) {
            let _ = key.delete_value(VALUE_NAME);
        }
    }
    Ok(())
}

/// Whether RedLight is currently registered to start with Windows.
pub fn is_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) {
        Ok(key) => key.get_value::<String, _>(VALUE_NAME).is_ok(),
        Err(_) => false,
    }
}
