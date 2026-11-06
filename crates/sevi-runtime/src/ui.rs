//! Shared UI-to-preferences translation; platform services remain in their hosts.
use crate::prefs::Preferences;
use sevi_ui_protocol::{Platform, Setting, UiSnapshot};
use std::io;

pub fn snapshot(prefs: &dyn Preferences, platform: Platform) -> UiSnapshot {
    let mut result = UiSnapshot {
        platform,
        language_vi: prefs.int("InputMethod", 1) != 0,
        ..Default::default()
    };
    for setting in Setting::ALL {
        result.settings.insert(
            setting,
            prefs.int(setting.key(), setting.default_value(platform)),
        );
    }
    result.capabilities.per_site = platform != Platform::Linux;
    result.capabilities.teencode = platform == Platform::MacOS;
    result.capabilities.excluded_apps = platform == Platform::MacOS;
    result.capabilities.per_app_auto = platform == Platform::MacOS;
    result.capabilities.login = platform != Platform::Linux;
    result.capabilities.permission = platform == Platform::MacOS;
    result.emoji_usage = if platform == Platform::Windows {
        prefs
            .string("EmojiUsage")
            .lines()
            .filter_map(|line| {
                let (key, value) = line.split_once('\t')?;
                Some((key.to_owned(), value.parse().ok()?))
            })
            .collect()
    } else {
        prefs
            .get("EmojiUsage")
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    };
    result
}
pub fn set(prefs: &mut dyn Preferences, setting: Setting, value: i32) -> io::Result<()> {
    if !setting.valid(value) {
        return Err(io::Error::other("Invalid setting value."));
    }
    let switch = prefs.int("SwitchKeyStatus", 0x7a000206) as u32;
    let switch = if switch == 0 { 0x7a000206 } else { switch };
    let emoji = prefs.int("EmojiHotkey", 0) as u32;
    let same = |a: u32, b: u32| a & 0xfff == b & 0xfff;
    match setting {
        Setting::EmojiHotkey
            if value != 0 && ((value as u32 & 0x700 == 0) || same(value as u32, switch)) =>
        {
            return Err(io::Error::other(
                "Choose a different shortcut with Control, Alt/Option or Super/Command.",
            ));
        }
        Setting::SwitchKeyStatus
            if prefs.int("EmojiPicker", 0) != 0 && emoji != 0 && same(value as u32, emoji) =>
        {
            return Err(io::Error::other(
                "That shortcut already opens the emoji picker.",
            ));
        }
        Setting::EmojiPicker if value != 0 && same(switch, emoji) => {
            prefs.set_int("EmojiHotkey", 0)?
        }
        _ => (),
    }
    prefs.set_int(setting.key(), value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::MemoryPreferences;
    #[test]
    fn preserves_native_hotkey_bits_and_rejects_collisions() {
        let mut p = MemoryPreferences::default();
        p.set_int("SwitchKeyStatus", 0x7a000206).unwrap();
        assert!(set(&mut p, Setting::EmojiHotkey, 0x8206).is_err());
        assert!(set(&mut p, Setting::EmojiHotkey, 0x80e).is_err());
        set(&mut p, Setting::EmojiHotkey, 0x90e).unwrap();
        set(&mut p, Setting::EmojiPicker, 1).unwrap();
        assert!(set(&mut p, Setting::SwitchKeyStatus, 0x890e).is_err());
        assert!(set(&mut p, Setting::TelexStyle, 4).is_err());
        assert_eq!(
            snapshot(&p, Platform::Windows).int(Setting::SwitchKeyStatus),
            0x7a000206
        );
    }
}
