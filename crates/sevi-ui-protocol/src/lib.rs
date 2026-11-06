//! Private parent/child UI messages. No renderer or OS calls enter the typing session.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
};

pub const VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Windows,
    MacOS,
    #[default]
    Linux,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum View {
    #[default]
    Settings,
    Help,
    About,
    Credits,
    Excluded,
    Emoji,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Setting {
    SwitchKeyStatus,
    AutoSwitchByTyping,
    PauseAutoAfterManual,
    UseSmartSwitchKey,
    SmartSwitchPerSite,
    TelexStyle,
    ModernOrthography,
    Spelling,
    RestoreIfInvalidWord,
    Teencode,
    EmojiPicker,
    EmojiHotkey,
    EmojiSkinTone,
    AutoUpdate,
}
impl Setting {
    pub const ALL: [Self; 14] = [
        Self::SwitchKeyStatus,
        Self::AutoSwitchByTyping,
        Self::PauseAutoAfterManual,
        Self::UseSmartSwitchKey,
        Self::SmartSwitchPerSite,
        Self::TelexStyle,
        Self::ModernOrthography,
        Self::Spelling,
        Self::RestoreIfInvalidWord,
        Self::Teencode,
        Self::EmojiPicker,
        Self::EmojiHotkey,
        Self::EmojiSkinTone,
        Self::AutoUpdate,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::SwitchKeyStatus => "SwitchKeyStatus",
            Self::AutoSwitchByTyping => "AutoSwitchByTyping",
            Self::PauseAutoAfterManual => "PauseAutoAfterManual",
            Self::UseSmartSwitchKey => "UseSmartSwitchKey",
            Self::SmartSwitchPerSite => "SmartSwitchPerSite",
            Self::TelexStyle => "TelexStyle",
            Self::ModernOrthography => "ModernOrthography",
            Self::Spelling => "Spelling",
            Self::RestoreIfInvalidWord => "RestoreIfInvalidWord",
            Self::Teencode => "Teencode",
            Self::EmojiPicker => "EmojiPicker",
            Self::EmojiHotkey => "EmojiHotkey",
            Self::EmojiSkinTone => "EmojiSkinTone",
            Self::AutoUpdate => "AutoUpdate",
        }
    }
    pub fn default_value(self, platform: Platform) -> i32 {
        match self {
            Self::SwitchKeyStatus => 0x7a000206,
            Self::SmartSwitchPerSite => i32::from(platform == Platform::Windows),
            Self::TelexStyle | Self::EmojiPicker | Self::EmojiHotkey | Self::EmojiSkinTone => 0,
            _ => 1,
        }
    }
    pub fn valid(self, value: i32) -> bool {
        match self {
            Self::SwitchKeyStatus | Self::EmojiHotkey => true,
            Self::TelexStyle => (0..=2).contains(&value),
            Self::EmojiSkinTone => (0..=5).contains(&value),
            _ => value == 0 || value == 1,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub excluded: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub per_site: bool,
    pub teencode: bool,
    pub excluded_apps: bool,
    pub per_app_auto: bool,
    pub login: bool,
    pub updates: bool,
    pub permission: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiSnapshot {
    pub protocol: u32,
    pub platform: Platform,
    pub version: String,
    pub language_vi: bool,
    pub settings: BTreeMap<Setting, i32>,
    pub capabilities: Capabilities,
    pub login: bool,
    pub permission_granted: bool,
    pub current_app: String,
    pub app_auto: bool,
    pub apps: Vec<AppEntry>,
    pub emoji_usage: BTreeMap<String, f64>,
    pub status: String,
}
impl Default for UiSnapshot {
    fn default() -> Self {
        Self {
            protocol: VERSION,
            platform: Platform::Linux,
            version: env!("CARGO_PKG_VERSION").into(),
            language_vi: true,
            settings: BTreeMap::new(),
            capabilities: Capabilities::default(),
            login: false,
            permission_granted: true,
            current_app: String::new(),
            app_auto: true,
            apps: vec![],
            emoji_usage: BTreeMap::new(),
            status: String::new(),
        }
    }
}
impl UiSnapshot {
    pub fn int(&self, setting: Setting) -> i32 {
        self.settings
            .get(&setting)
            .copied()
            .unwrap_or_else(|| setting.default_value(self.platform))
    }
    pub fn enabled(&self, setting: Setting) -> bool {
        self.int(setting) != 0
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UiCommand {
    Set { setting: Setting, value: i32 },
    SetLanguage(bool),
    SetLogin(bool),
    SetAppAuto(bool),
    SetExcluded { id: String, excluded: bool },
    AddExcludedApp,
    ResetExcludedApps,
    Refresh(View),
    CheckUpdates,
    AcceptUpdate,
    GrantPermission,
    OpenLink(String),
    Capture(bool),
    PickerReady(u64),
    InsertEmoji { text: String, base: String },
    PickerClosed,
    Close,
    Quit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PickerInput {
    Text(String),
    Backspace { word: bool, all: bool },
    SelectAll,
    Paste,
    Move { dx: i32, dy: i32 },
    Enter,
    Escape,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UiEvent {
    Snapshot(Box<UiSnapshot>),
    Language(bool),
    Open(View),
    Input(PickerInput),
    UpdateOffer { version: String, notes: String },
    Error(String),
    Shutdown,
}

pub mod host;

pub fn read<T: serde::de::DeserializeOwned>(reader: &mut impl BufRead) -> io::Result<Option<T>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incomplete UI message",
                ))
            };
        }
        let n = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |i| i + 1);
        if bytes.len() + n > MAX_MESSAGE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "UI message too large",
            ));
        }
        let done = available[n - 1] == b'\n';
        bytes.extend_from_slice(&available[..n]);
        reader.consume(n);
        if done {
            break;
        }
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
pub fn write<T: Serialize>(writer: &mut impl Write, message: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(message).map_err(io::Error::other)?;
    if bytes.len() + 1 > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "UI message too large",
        ));
    }
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_roundtrip_preserves_unicode_and_coalesced_messages() {
        let mut bytes = vec![];
        write(
            &mut bytes,
            &UiCommand::InsertEmoji {
                text: "👨‍👩‍👧‍👦".into(),
                base: "👨‍👩‍👧‍👦".into(),
            },
        )
        .unwrap();
        write(&mut bytes, &UiCommand::SetLanguage(false)).unwrap();
        let mut reader = io::BufReader::with_capacity(3, bytes.as_slice());
        assert!(
            matches!(read::<UiCommand>(&mut reader).unwrap(), Some(UiCommand::InsertEmoji { text, base }) if text == "👨‍👩‍👧‍👦" && text == base)
        );
        assert!(matches!(
            read::<UiCommand>(&mut reader).unwrap(),
            Some(UiCommand::SetLanguage(false))
        ));
        assert!(read::<UiCommand>(&mut reader).unwrap().is_none());
    }
    #[test]
    fn rejects_truncated_oversized_and_unknown_setting_messages() {
        assert!(read::<UiCommand>(&mut io::Cursor::new(b"{\"Close\"")).is_err());
        assert!(read::<UiCommand>(&mut io::Cursor::new(vec![b'x'; MAX_MESSAGE + 1])).is_err());
        assert!(
            read::<UiCommand>(&mut io::Cursor::new(
                b"{\"Set\":{\"setting\":\"arbitrary\",\"value\":1}}\n"
            ))
            .is_err()
        );
        assert!(!Setting::TelexStyle.valid(3));
    }
}
