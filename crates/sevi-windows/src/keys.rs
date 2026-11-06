use sevi_engine::keys::*;
use sevi_runtime::Modifiers;
pub fn engine_key(vk: u32) -> Option<u32> {
    if (0x41..=0x5a).contains(&vk) {
        return Some(LETTER_KEYS[(vk - 0x41) as usize]);
    }
    if (0x30..=0x39).contains(&vk) {
        return Some(
            [
                KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9,
            ][(vk - 0x30) as usize],
        );
    }
    if (0x60..=0x69).contains(&vk) {
        return engine_key(vk - 0x60 + 0x30);
    }
    Some(match vk {
        0x20 => KEY_SPACE,
        0x0d => KEY_RETURN,
        0x08 => KEY_DELETE,
        0x09 => KEY_TAB,
        0x1b => KEY_ESC,
        0x25 => KEY_LEFT,
        0x27 => KEY_RIGHT,
        0x26 => KEY_UP,
        0x28 => KEY_DOWN,
        0xba => KEY_SEMICOLON,
        0xbf => KEY_SLASH,
        0xc0 => KEY_BACKQUOTE,
        0xdb => KEY_LEFT_BRACKET,
        0xdc => KEY_BACK_SLASH,
        0xdd => KEY_RIGHT_BRACKET,
        0xde => KEY_QUOTE,
        0xbc => KEY_COMMA,
        0xbe => KEY_DOT,
        0xbd => KEY_MINUS,
        0xbb => KEY_EQUALS,
        0x2d | 0x24 | 0x23 | 0x2e | 0x21 | 0x22 | 0x2c | 0x2a | 0x29 | 0x2f | 0x2b | 0x90
        | 0x91 => KEY_ESC,
        _ => return None,
    })
}
pub fn unmodeled_ends_word(vk: u32) -> bool {
    matches!(vk, 0x6a..=0x6f | 0xe2 | 0xdf | 0x0c)
}
/// A restored word must be followed by the original physical key, including its extended flag.
pub fn forwarded_key(current: Option<(u16, bool)>, engine: u32) -> Option<(u16, bool)> {
    current.or_else(|| {
        let vk = virtual_key(engine);
        (vk != 0).then_some((vk, false))
    })
}
pub fn virtual_key(key: u32) -> u16 {
    match key {
        KEY_RETURN | KEY_ENTER => 0x0d,
        KEY_TAB => 0x09,
        KEY_ESC => 0x1b,
        KEY_SPACE => 0x20,
        KEY_LEFT => 0x25,
        KEY_RIGHT => 0x27,
        KEY_UP => 0x26,
        KEY_DOWN => 0x28,
        _ => 0,
    }
}
pub fn modifier(vk: u32) -> Option<Modifiers> {
    match vk {
        0x10 | 0xa0 | 0xa1 => Some(Modifiers::SHIFT),
        0x11 | 0xa2 | 0xa3 => Some(Modifiers::CONTROL),
        0x12 | 0xa4 | 0xa5 => Some(Modifiers::ALT),
        0x5b | 0x5c => Some(Modifiers::SUPER),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numpad_digits_and_unmodeled_characters() {
        for n in 0..10 {
            assert_eq!(engine_key(0x60 + n), engine_key(0x30 + n));
        }
        for vk in [0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0xdf, 0xe2, 0x0c] {
            assert_eq!(engine_key(vk), None);
            assert!(unmodeled_ends_word(vk));
        }
        assert!(!unmodeled_ends_word(0x74));
    }
    #[test]
    fn forwarding_preserves_home_and_keypad_identity() {
        assert_eq!(
            forwarded_key(Some((0x24, true)), KEY_ESC),
            Some((0x24, true))
        );
        assert_eq!(
            forwarded_key(Some((0x61, false)), KEY_1),
            Some((0x61, false))
        );
        assert_eq!(forwarded_key(None, KEY_RETURN), Some((0x0d, false)));
    }
}
