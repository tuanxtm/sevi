use crate::wide;
use serde_json::Value;
use sevi_runtime::prefs::Preferences;
use std::{io, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
        System::Registry::*,
        UI::Shell::{CSIDL_LOCAL_APPDATA, SHGFP_TYPE_CURRENT, SHGetFolderPathW},
    },
    core::PCWSTR,
};
const ROOT: &str = "Software\\Sevi";
const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
impl Key {
    fn open(path: &str, access: REG_SAM_FLAGS, create: bool) -> io::Result<Self> {
        let mut key = HKEY::default();
        let path = wide(path);
        let status = unsafe {
            if create {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(path.as_ptr()),
                    Some(0),
                    None,
                    REG_OPTION_NON_VOLATILE,
                    access,
                    None,
                    &mut key,
                    None,
                )
            } else {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(path.as_ptr()),
                    Some(0),
                    access,
                    &mut key,
                )
            }
        };
        if status == ERROR_SUCCESS {
            Ok(Self(key))
        } else {
            Err(io::Error::from_raw_os_error(status.0 as i32))
        }
    }
    fn read(&self, name: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
        let name = wide(name);
        let (mut kind, mut size) = (REG_VALUE_TYPE::default(), 0u32);
        unsafe {
            if RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            ) != ERROR_SUCCESS
            {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            if RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr()),
                Some(&mut size),
            ) != ERROR_SUCCESS
            {
                return None;
            }
            data.truncate(size as usize);
            Some((kind, data))
        }
    }
    fn write(&self, name: &str, kind: REG_VALUE_TYPE, data: &[u8]) -> io::Result<()> {
        let name = wide(name);
        let status =
            unsafe { RegSetValueExW(self.0, PCWSTR(name.as_ptr()), Some(0), kind, Some(data)) };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status.0 as i32))
        }
    }
    fn remove(&self, name: &str) -> io::Result<()> {
        let name = wide(name);
        let status = unsafe { RegDeleteValueW(self.0, PCWSTR(name.as_ptr())) };
        if status == ERROR_SUCCESS || status.0 == 2 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status.0 as i32))
        }
    }
    fn dword(&self, name: &str) -> Option<i32> {
        let (kind, bytes) = self.read(name)?;
        if kind != REG_DWORD || bytes.len() != 4 {
            return None;
        }
        Some(i32::from_le_bytes(bytes.try_into().ok()?))
    }
}
#[derive(Default)]
pub struct RegistryPreferences;
impl Preferences for RegistryPreferences {
    fn get(&self, name: &str) -> Option<Value> {
        let (kind, data) = Key::open(ROOT, KEY_READ, true).ok()?.read(name)?;
        if kind == REG_DWORD && data.len() == 4 {
            Some(Value::from(i32::from_le_bytes(data.try_into().ok()?)))
        } else if kind == REG_SZ {
            let units: Vec<_> = data
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .take_while(|c| *c != 0)
                .collect();
            Some(Value::from(String::from_utf16_lossy(&units)))
        } else if kind == REG_BINARY {
            Some(Value::Array(data.into_iter().map(Value::from).collect()))
        } else {
            None
        }
    }
    fn set(&mut self, name: &str, value: Value) -> io::Result<()> {
        let key = Key::open(ROOT, KEY_WRITE, true)?;
        match value {
            Value::Number(n) => key.write(
                name,
                REG_DWORD,
                &(n.as_i64()
                    .ok_or_else(|| io::Error::other("integer registry value"))?
                    as i32)
                    .to_le_bytes(),
            ),
            Value::Bool(b) => key.write(name, REG_DWORD, &i32::from(b).to_le_bytes()),
            Value::String(s) => {
                let bytes: Vec<_> = wide(&s).into_iter().flat_map(u16::to_le_bytes).collect();
                key.write(name, REG_SZ, &bytes)
            }
            _ => Err(io::Error::other("unsupported registry value type")),
        }
    }
    fn remove(&mut self, name: &str) -> io::Result<()> {
        Key::open(ROOT, KEY_WRITE, true)?.remove(name)
    }
    fn keys(&self) -> Vec<String> {
        let Ok(key) = Key::open(ROOT, KEY_READ, true) else {
            return vec![];
        };
        let mut names = vec![];
        for i in 0.. {
            let mut buffer = vec![0u16; 16384];
            let mut len = buffer.len() as u32;
            let status = unsafe {
                RegEnumValueW(
                    key.0,
                    i,
                    Some(windows::core::PWSTR(buffer.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            if status != ERROR_SUCCESS {
                break;
            }
            names.push(String::from_utf16_lossy(&buffer[..len as usize]));
        }
        names
    }
    fn binary(&self, name: &str) -> Vec<u8> {
        Key::open(ROOT, KEY_READ, true)
            .ok()
            .and_then(|k| k.read(name))
            .map(|(_, b)| b)
            .unwrap_or_default()
    }
    fn set_binary(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        Key::open(ROOT, KEY_WRITE, true)?.write(name, REG_BINARY, data)
    }
}
pub fn data_dir() -> io::Result<PathBuf> {
    let mut path = [0u16; 260];
    unsafe {
        SHGetFolderPathW(
            None,
            CSIDL_LOCAL_APPDATA as i32,
            None,
            SHGFP_TYPE_CURRENT.0 as u32,
            &mut path,
        )
        .map_err(io::Error::other)?;
    }
    let n = path.iter().position(|c| *c == 0).unwrap_or(path.len());
    let dir = PathBuf::from(String::from_utf16_lossy(&path[..n])).join("Sevi");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
pub fn launch_at_login() -> bool {
    Key::open(RUN, KEY_READ, false)
        .ok()
        .is_some_and(|k| k.read("Sevi").is_some())
}
pub fn set_launch_at_login(on: bool) -> io::Result<()> {
    let key = Key::open(RUN, KEY_WRITE, false)?;
    if on {
        let command = format!("\"{}\"", std::env::current_exe()?.display());
        let bytes: Vec<_> = wide(&command)
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect();
        key.write("Sevi", REG_SZ, &bytes)
    } else {
        key.remove("Sevi")
    }
}
pub fn migrate_from_open_key() -> io::Result<()> {
    let mut prefs = RegistryPreferences;
    let source = || Key::open("SOFTWARE\\TuyenMai\\OpenKey", KEY_READ, false).ok();
    if prefs.get("MigratedTelexStyle").is_none() {
        if prefs.get("TelexStyle").is_some() {
            prefs.set_int("MigratedTelexStyle", 1)?;
        } else if let Some(style) = source().and_then(|k| k.dword("vInputType")) {
            if style == 2 || style == 3 {
                prefs.set_int("TelexStyle", style - 1)?;
            }
            prefs.set_int("MigratedTelexStyle", 1)?;
        }
    }
    if prefs.get("MigratedFromOpenKey").is_some() {
        return Ok(());
    }
    prefs.set_int("MigratedFromOpenKey", 1)?;
    let Some(old) = source() else { return Ok(()) };
    for (from, to) in [
        ("vLanguage", "InputMethod"),
        ("vCheckSpelling", "Spelling"),
        ("vRestoreIfWrongSpelling", "RestoreIfInvalidWord"),
        ("vUseModernOrthography", "ModernOrthography"),
        ("vUseSmartSwitchKey", "UseSmartSwitchKey"),
        ("vSwitchKeyStatus", "SwitchKeyStatus"),
    ] {
        if prefs.get(to).is_none()
            && let Some(n) = old.dword(from)
        {
            prefs.set_int(to, n)?;
        }
    }
    if prefs.get("smartSwitchKey").is_none()
        && let Some((_, data)) = old.read("smartSwitchKey")
        && !data.is_empty()
    {
        prefs.set_binary("smartSwitchKey", &data)?;
    }
    Ok(())
}
