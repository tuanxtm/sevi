use sevi_engine::keys::*;
use sevi_runtime::Modifiers;
pub fn engine_key(value: u32) -> Option<(u32, bool)> {
    if (0xffb0..=0xffb9).contains(&value) {
        return engine_key(value - 0xffb0 + b'0' as u32);
    }
    if value >= b'a' as u32 && value <= b'z' as u32 {
        return Some((LETTER_KEYS[(value - b'a' as u32) as usize], false));
    }
    if value >= b'A' as u32 && value <= b'Z' as u32 {
        return Some((LETTER_KEYS[(value - b'A' as u32) as usize], true));
    }
    if let Some(i) = "0123456789".chars().position(|c| c as u32 == value) {
        return Some((
            [
                KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9,
            ][i],
            false,
        ));
    }
    if let Some(i) = ")!@#$%^&*(".chars().position(|c| c as u32 == value) {
        return Some((
            [
                KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9,
            ][i],
            true,
        ));
    }
    for (base, shift, key) in [
        ('`', '~', KEY_BACKQUOTE),
        ('-', '_', KEY_MINUS),
        ('=', '+', KEY_EQUALS),
        ('[', '{', KEY_LEFT_BRACKET),
        (']', '}', KEY_RIGHT_BRACKET),
        ('\\', '|', KEY_BACK_SLASH),
        (';', ':', KEY_SEMICOLON),
        ('\'', '"', KEY_QUOTE),
        (',', '<', KEY_COMMA),
        ('.', '>', KEY_DOT),
        ('/', '?', KEY_SLASH),
    ] {
        if value == base as u32 {
            return Some((key, false));
        }
        if value == shift as u32 {
            return Some((key, true));
        }
    }
    Some((
        match value {
            32 => KEY_SPACE,
            0xff0d => KEY_RETURN,
            0xff8d => KEY_ENTER,
            0xff08 => KEY_DELETE,
            0xff09 | 0xfe20 => KEY_TAB,
            0xff1b => KEY_ESC,
            0xff51 => KEY_LEFT,
            0xff53 => KEY_RIGHT,
            0xff52 => KEY_UP,
            0xff54 => KEY_DOWN,
            0xff63 | 0xff50 | 0xff57 | 0xffff | 0xff55 | 0xff56 | 0xff61 | 0xff7f | 0xff14
            | 0xff95 | 0xff9c | 0xff9f => KEY_ESC,
            _ => return None,
        },
        false,
    ))
}
pub fn modifier(value: u32) -> Option<Modifiers> {
    match value {
        0xffe1 | 0xffe2 => Some(Modifiers::SHIFT),
        0xffe3 | 0xffe4 => Some(Modifiers::CONTROL),
        0xffe7..=0xffea => Some(Modifiers::ALT),
        0xffeb | 0xffec => Some(Modifiers::SUPER),
        _ => None,
    }
}
pub fn is_modifier(value: u32) -> bool {
    modifier(value).is_some() || value == 0xffe5 || value == 0xfe03
}
pub fn flags(state: u32) -> Modifiers {
    Modifiers(
        (state & 1)
            | if state & 4 != 0 { 2 } else { 0 }
            | if state & 8 != 0 { 4 } else { 0 }
            | if state & (1 << 26) != 0 { 8 } else { 0 }
            | if state & 2 != 0 { 16 } else { 0 },
    )
}
pub fn keysym(key: u32) -> u32 {
    match key {
        KEY_RETURN => 0xff0d,
        KEY_ENTER => 0xff8d,
        KEY_TAB => 0xff09,
        KEY_ESC => 0xff1b,
        KEY_SPACE => 32,
        KEY_LEFT => 0xff51,
        KEY_RIGHT => 0xff53,
        KEY_UP => 0xff52,
        KEY_DOWN => 0xff54,
        _ => 0,
    }
}
pub fn hotkey_label(hk: impl Into<i64>) -> String {
    let hk = hk.into() as u32;
    if hk & !0x8000 == 0 {
        return "Chưa đặt".into();
    }
    let mut out = String::new();
    for (mask, label) in [
        (0x100, "Ctrl+"),
        (0x200, "Alt+"),
        (0x800, "Shift+"),
        (0x400, "Super+"),
    ] {
        if hk & mask != 0 {
            out.push_str(label)
        }
    }
    let key = hk & 255;
    if key == 254 {
        out.pop();
    } else {
        let c = sevi_engine::data::key_character(key);
        if let Some(letter) = sevi_engine::engine::letter(key) {
            out.push(letter.to_ascii_uppercase() as char)
        } else {
            match key {
                KEY_SPACE => out.push_str("Space"),
                KEY_RETURN => out.push_str("Enter"),
                KEY_TAB => out.push_str("Tab"),
                _ => {
                    if let Some(c) = char::from_u32(c).filter(|c| *c != '\0') {
                        out.push(c)
                    } else {
                        out.push_str(&format!("#{key}"))
                    }
                }
            }
        }
    }
    if out.is_empty() { "—".into() } else { out }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numpad_digits_follow_digit_word_boundaries() {
        for n in 0..10 {
            assert_eq!(engine_key(0xffb0 + n), engine_key(b'0' as u32 + n));
        }
        assert_eq!(engine_key(0xffab), None);
    }
}
