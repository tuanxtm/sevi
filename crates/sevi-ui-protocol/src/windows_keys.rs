//! Windows key routing shared by the native hook and the nonactivating picker.
//! Raw virtual-key values keep the policy testable without Win32 on the build host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerKeyRoute {
    Capture,
    CloseAndPass,
    Pass,
}
pub fn picker_key_route(vk: u32, modifiers: u32) -> PickerKeyRoute {
    let control = modifiers & 2 != 0;
    let alt = modifiers & 4 != 0;
    if modifiers & 8 != 0 || (alt && !control && vk != 0x08) || (vk == 0x1b && control && !alt) {
        PickerKeyRoute::CloseAndPass
    } else if vk == 0x2c || (0xa6..=0xb7).contains(&vk) {
        PickerKeyRoute::Pass
    } else {
        PickerKeyRoute::Capture
    }
}
/// Keyboard state for ToUnicodeEx. Ctrl+Alt is AltGr; plain Ctrl/Alt remain commands.
pub fn translation_state(modifiers: u32) -> Option<[u8; 256]> {
    let altgr = modifiers & 6 == 6;
    if modifiers & 8 != 0 || (modifiers & 6 != 0 && !altgr) {
        return None;
    }
    let mut state = [0; 256];
    if modifiers & 1 != 0 {
        state[0x10] = 0x80;
    }
    if modifiers & 16 != 0 {
        state[0x14] = 1;
    }
    if altgr {
        state[0x11] = 0x80;
        state[0x12] = 0x80;
    }
    Some(state)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn picker_preserves_os_shortcuts_device_keys_and_search_commands() {
        for (vk, modifiers, expected) in [
            (0x09, 4, PickerKeyRoute::CloseAndPass), // Alt+Tab
            (0x73, 4, PickerKeyRoute::CloseAndPass), // Alt+F4
            (0x44, 8, PickerKeyRoute::CloseAndPass), // Win+D
            (0x1b, 2, PickerKeyRoute::CloseAndPass), // Ctrl+Esc
            (0x08, 4, PickerKeyRoute::Capture),      // Alt+Backspace
            (0x51, 6, PickerKeyRoute::Capture),      // AltGr+Q
            (0x1b, 6, PickerKeyRoute::Capture),      // AltGr is not Ctrl+Esc
            (0x41, 2, PickerKeyRoute::Capture),      // Ctrl+A
            (0x56, 2, PickerKeyRoute::Capture),      // Ctrl+V
            (0x74, 0, PickerKeyRoute::Capture),      // F5 stays captured
            (0x1b, 0, PickerKeyRoute::Capture),
            (0xe7, 0, PickerKeyRoute::Capture), // Unicode packet
            (0x2c, 0, PickerKeyRoute::Pass),
            (0xad, 0, PickerKeyRoute::Pass), // mute
            (0xb0, 0, PickerKeyRoute::Pass), // next track
            (0xa6, 0, PickerKeyRoute::Pass), // browser back
            (0xb7, 0, PickerKeyRoute::Pass), // launch app
        ] {
            assert_eq!(
                picker_key_route(vk, modifiers),
                expected,
                "vk {vk:x}, modifiers {modifiers}"
            );
        }
    }
    #[test]
    fn altgr_translation_carries_both_modifiers_and_shift_caps() {
        let altgr = translation_state(6).unwrap();
        assert_eq!((altgr[0x11], altgr[0x12]), (0x80, 0x80));
        let shifted = translation_state(23).unwrap();
        assert_eq!((shifted[0x10], shifted[0x14]), (0x80, 1));
        assert!(translation_state(2).is_none());
        assert!(translation_state(4).is_none());
        assert!(translation_state(8).is_none());
        assert!(translation_state(14).is_none());
        assert!(translation_state(0).unwrap().iter().all(|byte| *byte == 0));
    }
}
