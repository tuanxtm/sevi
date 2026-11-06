use crate::keys::MAX_BUFF;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum Language {
    English = 0,
    #[default]
    Vietnamese = 1,
}
impl Language {
    pub fn toggled(self) -> Self {
        if self == Self::English {
            Self::Vietnamese
        } else {
            Self::English
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum TelexStyle {
    #[default]
    Telex = 0,
    Simple1 = 1,
    Simple2 = 2,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineConfig {
    pub language: Language,
    pub spelling: bool,
    pub modern_orthography: bool,
    pub restore_if_invalid: bool,
    pub auto_switch: bool,
    pub teencode: bool,
    pub telex_style: TelexStyle,
}
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            language: Language::Vietnamese,
            spelling: true,
            modern_orthography: true,
            restore_if_invalid: true,
            auto_switch: true,
            teencode: true,
            telex_style: TelexStyle::Telex,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum HookCode {
    #[default]
    Nothing = 0,
    Process = 1,
    BreakWord = 2,
    Restore = 3,
    UnusedMacro = 4,
    RestoreAndReset = 5,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HookResult {
    pub code: HookCode,
    pub backspaces: usize,
    pub new_chars: usize,
    pub ext_code: u8,
    /// Engine order: the last character is first, as in the original hook state.
    pub chars: [u32; MAX_BUFF],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Keyboard,
    Mouse,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventState {
    KeyDown,
    KeyUp,
    MouseDown,
    MouseUp,
}
