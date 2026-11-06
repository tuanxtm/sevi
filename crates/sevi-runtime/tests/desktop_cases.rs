use sevi_engine::{Language, keys::*};
use sevi_runtime::{
    DesktopSession, Effect, EventResult, Modifiers, Platform,
    prefs::{MemoryPreferences, Preferences},
};
fn effects(result: &EventResult, text: &mut Vec<char>) {
    for e in &result.effects {
        match e {
            Effect::Text(s) => text.extend(s.chars()),
            Effect::Backspaces(n) => {
                text.truncate(text.len().saturating_sub(*n));
            }
            Effect::ControlKey { key, .. } => match *key {
                KEY_SPACE => text.push(' '),
                KEY_RETURN | KEY_ENTER => text.push('\n'),
                KEY_TAB => text.push('\t'),
                _ => (),
            },
            _ => (),
        }
    }
}
fn key(
    session: &mut DesktopSession,
    text: &mut Vec<char>,
    key: u32,
    shift: bool,
    raw: char,
    now: &mut f64,
) {
    key_with_flags(session, text, key, shift, raw, now, Modifiers::default());
}
fn key_with_flags(
    session: &mut DesktopSession,
    text: &mut Vec<char>,
    key: u32,
    shift: bool,
    raw: char,
    now: &mut f64,
    flags: Modifiers,
) {
    *now += 0.03;
    let result = session.key_down(key, shift, flags, *now);
    effects(&result, text);
    if !result.consumed {
        if key == KEY_DELETE {
            text.pop();
        } else {
            text.push(raw);
        }
    }
}
fn run(source: &str, platform: Platform) {
    for line in source
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let f: Vec<_> = line.split('\t').collect();
        let (name, start, input, expected) = (f[0], f[1], f[2].replace("tô thí", "too this"), f[3]);
        let mut p = MemoryPreferences::default();
        p.set_int("InputMethod", i32::from(start == "vi")).unwrap();
        let mut s = DesktopSession::new(platform, Box::new(p));
        let mut text = vec![];
        let mut now = 0.0;
        let mut caret = None;
        let mut flags = Modifiers::default();
        let mut chars = input.chars();
        while let Some(c) = chars.next() {
            if c == '{' {
                let command: String = chars.by_ref().take_while(|c| *c != '}').collect();
                if command == "alt+z" {
                    now += 0.03;
                    effects(&s.modifier(Modifiers::ALT, true, now), &mut text);
                    now += 0.03;
                    let r = s.key_down(KEY_Z, false, Modifiers::ALT, now);
                    assert!(r.consumed, "hotkey {name}");
                    effects(&r, &mut text);
                    now += 0.03;
                    effects(&s.modifier(Modifiers::ALT, false, now), &mut text);
                } else if command == "caps" {
                    flags.0 ^= Modifiers::CAPS.0;
                } else if let Some(value) = command.strip_prefix("cmd:emoji ") {
                    effects(&s.insert_text(value), &mut text);
                } else if command == "wait" {
                    now += 1.5;
                    s.tick(now);
                } else if command == "home" || command == "Home" {
                    now += 0.03;
                    let r = s.key_down(KEY_ESC, false, Modifiers::default(), now);
                    effects(&r, &mut text);
                    caret = Some(0);
                } else if command == "add" || command == "KP_Add" {
                    s.unmodeled_key(true);
                    text.push('+');
                } else if let Some(number) = command
                    .strip_prefix("num")
                    .or_else(|| command.strip_prefix("KP_"))
                {
                    let digit = number
                        .chars()
                        .next()
                        .filter(char::is_ascii_digit)
                        .expect("numpad digit");
                    let (keycode, shift) = sevi_test_tools::key_for(digit).unwrap();
                    key_with_flags(&mut s, &mut text, keycode, shift, digit, &mut now, flags);
                } else {
                    panic!("unsupported command {command}")
                }
                continue;
            }
            if c == '\\' {
                let c = chars.next().unwrap();
                let (keycode, raw) = match c {
                    'b' => (KEY_DELETE, '\0'),
                    'n' => (KEY_RETURN, '\n'),
                    't' => (KEY_TAB, '\t'),
                    _ => panic!("unsupported escape {c}"),
                };
                key_with_flags(&mut s, &mut text, keycode, false, raw, &mut now, flags);
                continue;
            }
            if let Some((keycode, shift)) = sevi_test_tools::key_for(c) {
                if let Some(at) = caret {
                    let tail = text.split_off(at);
                    key_with_flags(&mut s, &mut text, keycode, shift, c, &mut now, flags);
                    caret = Some(text.len());
                    text.extend(tail);
                } else {
                    key_with_flags(&mut s, &mut text, keycode, shift, c, &mut now, flags);
                }
            } else {
                text.push(c)
            }
        }
        assert_eq!(
            text.iter().collect::<String>(),
            expected,
            "{platform:?}: {name}"
        );
    }
}
#[test]
fn linux_desktop_cases() {
    run(
        include_str!("../../../Windows/Tests/cases.tsv"),
        Platform::Linux,
    );
    run(
        include_str!("../../../Linux/Tests/cases-linux.tsv"),
        Platform::Linux,
    );
}
#[test]
fn windows_desktop_cases() {
    run(
        include_str!("../../../Windows/Tests/cases.tsv"),
        Platform::Windows,
    );
}
#[test]
fn mac_desktop_cases() {
    run(
        include_str!("../../../Windows/Tests/cases.tsv"),
        Platform::Mac,
    );
}

#[test]
fn mac_excluded_apps_keep_language_hotkey_without_remembering() {
    let mut p = MemoryPreferences::default();
    p.set("ExcludedApps", serde_json::json!(["remote.app"]))
        .unwrap();
    let mut s = DesktopSession::new(Platform::Mac, Box::new(p));
    s.focus_changed("remote.app", 0.0);
    assert!(s.excluded);
    s.modifier(Modifiers::ALT, true, 0.1);
    assert!(s.key_down(KEY_Z, false, Modifiers::ALT, 0.2).consumed);
    assert_eq!(s.language(), Language::English);
    assert!(s.preferences.binary("smartSwitchKey").is_empty());
    let result = s.key_down(KEY_A, false, Modifiers::default(), 0.3);
    assert!(!result.consumed);
    assert!(result.effects.is_empty());
}

#[test]
fn mac_rewrite_and_undo_request_different_native_output_policies() {
    let mut s = DesktopSession::new(Platform::Mac, Box::<MemoryPreferences>::default());
    let mut now = 0.;
    for c in "more ".chars() {
        now += 0.1;
        let (k, shift) = sevi_test_tools::key_for(c).unwrap();
        let r = s.key_down(k, shift, Modifiers::default(), now);
        if c == ' ' {
            assert!(r.effects.iter().any(|e| matches!(
                e,
                Effect::MacRewrite {
                    selection: true,
                    ..
                }
            )));
        }
    }
    let r = s.key_down(KEY_DELETE, false, Modifiers::default(), now + 0.1);
    assert!(r.consumed);
    assert_eq!(
        r.effects[0],
        Effect::MacRewrite {
            autocomplete: true,
            selection: false
        }
    );
}
#[test]
fn manual_pause_expires() {
    let mut s = DesktopSession::new(Platform::Linux, Box::<MemoryPreferences>::default());
    s.set_language_user(Language::English, 10.0, "hotkey", true);
    assert!(!s.engine.config.auto_switch);
    s.tick(14.99);
    assert!(!s.engine.config.auto_switch);
    s.tick(15.01);
    assert!(s.engine.config.auto_switch);
}

#[test]
fn windows_password_input_bypasses_transformations() {
    let mut s = DesktopSession::new(Platform::Windows, Box::<MemoryPreferences>::default());
    s.log.secure = true;
    for (i, c) in "tieengs".chars().enumerate() {
        let (k, shift) = sevi_test_tools::key_for(c).unwrap();
        let result = s.key_down(k, shift, Modifiers::default(), i as f64);
        assert!(!result.consumed);
        assert!(result.effects.is_empty());
    }
    s.log.secure = false;
    let mut text = vec![];
    let mut now = 10.0;
    for c in "aa".chars() {
        let (k, shift) = sevi_test_tools::key_for(c).unwrap();
        key(&mut s, &mut text, k, shift, c, &mut now);
    }
    assert_eq!(text.iter().collect::<String>(), "â");
}

#[test]
fn windows_failed_output_forgets_correction_and_bypasses_even_after_word_breaks() {
    let mut s = DesktopSession::new(Platform::Windows, Box::<MemoryPreferences>::default());
    let mut now = 0.0;
    let mut text = vec![];
    for c in "more ".chars() {
        let (k, shift) = sevi_test_tools::key_for(c).unwrap();
        key(&mut s, &mut text, k, shift, c, &mut now);
    }
    let language = s.language();
    let config = s.engine.config.clone();
    s.discard_failed_output();
    assert!(s.insertion_blocked);
    assert_eq!(s.language(), language);
    assert_eq!(s.engine.config.language, config.language);
    assert_eq!(s.engine.config.auto_switch, config.auto_switch);
    assert!(s.engine.undo_data.is_empty());
    assert!(s.engine.restored_keys.is_empty());
    assert!(s.engine.switch_request.is_none());
    // A failed correction must not leave Backspace undo or cross-word rewrites armed.
    for k in [
        KEY_DELETE, KEY_A, KEY_A, KEY_SPACE, KEY_RETURN, KEY_A, KEY_A,
    ] {
        now += 0.03;
        let result = s.key_down(k, false, Modifiers::default(), now);
        assert!(!result.consumed);
        assert!(result.effects.is_empty());
    }
    s.click();
    s.reload_settings(now);
    s.site_changed("example.com");
    assert!(s.insertion_blocked);
    // Language hotkeys remain available without lifting the insertion guard.
    s.modifier(Modifiers::ALT, true, now + 0.1);
    assert!(s.key_down(KEY_Z, false, Modifiers::ALT, now + 0.2).consumed);
    s.modifier(Modifiers::ALT, false, now + 0.3);
    assert!(s.insertion_blocked);
    // This is the host's foreground-change recovery, not a timed/word retry.
    s.insertion_blocked = false;
    s.reset_typing();
    s.set_language_quiet(Language::Vietnamese);
    text.clear();
    for c in "aa".chars() {
        key(&mut s, &mut text, KEY_A, false, c, &mut now);
    }
    assert_eq!(text.iter().collect::<String>(), "â");
}

#[test]
fn windows_failed_output_cannot_restore_navigation_snapshots() {
    for language in [Language::English, Language::Vietnamese] {
        let mut s = DesktopSession::new(Platform::Windows, Box::<MemoryPreferences>::default());
        s.set_language_quiet(language);
        s.key_down(KEY_A, false, Modifiers::default(), 0.1);
        s.key_down(KEY_LEFT, false, Modifiers::default(), 0.2);
        s.discard_failed_output();
        s.insertion_blocked = false;
        // Returning from an old caret excursion must not resurrect the old 'a'.
        let mut fresh = DesktopSession::new(Platform::Windows, Box::<MemoryPreferences>::default());
        fresh.set_language_quiet(language);
        for (index, k) in [KEY_RIGHT, KEY_A, KEY_SPACE, KEY_A, KEY_A, KEY_SPACE]
            .into_iter()
            .enumerate()
        {
            let now = 0.3 + index as f64 * 0.03;
            assert_eq!(
                s.key_down(k, false, Modifiers::default(), now),
                fresh.key_down(k, false, Modifiers::default(), now),
                "{language:?}, key {index}"
            );
        }
    }
}

#[test]
fn windows_site_memory_is_enabled_by_default() {
    let mut s = DesktopSession::new(Platform::Windows, Box::<MemoryPreferences>::default());
    s.focus_changed("chrome.exe", 0.0);
    s.site_changed("example.com");
    s.set_language_user(Language::English, 1.0, "menu", false);
    s.site_changed("other.example");
    s.set_language_user(Language::Vietnamese, 2.0, "menu", false);
    s.site_changed("example.com");
    assert_eq!(s.language(), Language::English);
    let mut mac = DesktopSession::new(Platform::Mac, Box::<MemoryPreferences>::default());
    mac.focus_changed("com.google.Chrome", 0.0);
    mac.site_changed("example.com");
    assert!(mac.site.is_empty());
}

#[test]
fn windows_other_ime_keeps_keys_and_resets_at_word_boundaries() {
    let mut p = MemoryPreferences::default();
    p.set_int("InputMethod", 0).unwrap();
    let mut s = DesktopSession::new(Platform::Windows, Box::new(p));
    s.other_language = true;
    let mut text = vec![];
    let mut now = 0.0;
    for c in "tieengs ".chars() {
        let (k, shift) = sevi_test_tools::key_for(c).unwrap();
        key(&mut s, &mut text, k, shift, c, &mut now);
    }
    assert_eq!(text.iter().collect::<String>(), "tieengs ");
    s.other_language = false;
    for c in "vieetj ".chars() {
        let (k, shift) = sevi_test_tools::key_for(c).unwrap();
        key(&mut s, &mut text, k, shift, c, &mut now);
    }
    assert_eq!(text.iter().collect::<String>(), "tieengs việt ");
}

#[test]
fn mac_hotkey_repeats_are_swallowed_and_holding_alt_allows_another_press() {
    let mut s = DesktopSession::new(Platform::Mac, Box::<MemoryPreferences>::default());
    s.modifier(Modifiers::ALT, true, 0.1);
    assert!(s.key_down(KEY_Z, false, Modifiers::ALT, 0.2).consumed);
    assert_eq!(s.language(), Language::English);
    let repeated = s.key_down_event(KEY_Z, false, Modifiers::ALT, true, 0.3);
    assert!(repeated.consumed);
    assert!(repeated.effects.is_empty());
    assert_eq!(s.language(), Language::English);
    assert!(s.key_down(KEY_Z, false, Modifiers::ALT, 0.4).consumed);
    assert_eq!(s.language(), Language::Vietnamese);
}
#[test]
fn mac_modifier_hotkey_is_cancelled_until_every_modifier_is_released() {
    for use_click in [false, true] {
        let mut p = MemoryPreferences::default();
        p.set_int("SwitchKeyStatus", 0x9fe).unwrap();
        let mut s = DesktopSession::new(Platform::Mac, Box::new(p));
        s.modifier(Modifiers::CONTROL, true, 0.1);
        s.modifier(Modifiers::SHIFT, true, 0.2);
        s.modifier(Modifiers::ALT, true, 0.3);
        if use_click {
            s.click();
        } else {
            s.key_down(KEY_K, true, Modifiers(7), 0.4);
        }
        for (bit, time) in [
            (Modifiers::ALT, 0.5),
            (Modifiers::SHIFT, 0.6),
            (Modifiers::CONTROL, 0.7),
        ] {
            assert!(!s.modifier(bit, false, time).consumed);
        }
        assert_eq!(s.language(), Language::Vietnamese);
        s.modifier(Modifiers::CONTROL, true, 0.8);
        s.modifier(Modifiers::SHIFT, true, 0.9);
        assert!(s.modifier(Modifiers::SHIFT, false, 1.).consumed);
        assert_eq!(s.language(), Language::English);
    }
}
#[test]
fn unmodeled_keys_cancel_pending_hotkeys_without_recording_history() {
    let mut p = MemoryPreferences::default();
    p.set_int("SwitchKeyStatus", 0x9fe).unwrap();
    p.set_int("TypingLogEnabled", 1).unwrap();
    let mut s = DesktopSession::new(Platform::Windows, Box::new(p));
    s.key_down(KEY_A, false, Modifiers::default(), 0.1);
    s.modifier(Modifiers::CONTROL, true, 0.2);
    s.modifier(Modifiers::SHIFT, true, 0.3);
    s.unmodeled_key(true);
    assert!(!s.modifier(Modifiers::SHIFT, false, 0.4).consumed);
    assert_eq!(s.language(), Language::Vietnamese);
    assert!(s.log.pending.is_empty());
    s.key_down(KEY_SPACE, false, Modifiers::default(), 0.5);
    assert!(s.log.pending.is_empty());
}

#[test]
fn caps_lock_restoration_keeps_semicolon_and_quote() {
    for platform in [Platform::Windows, Platform::Linux, Platform::Mac] {
        run(
            "caps_semicolon\tvi\tNAS{caps};{caps}\tNAS;\ncaps_quote\tvi\tNAS{caps}'{caps}\tNAS'\n",
            platform,
        );
    }
}

#[test]
fn mac_emoji_repeat_and_release_do_not_toggle_language() {
    let mut prefs = MemoryPreferences::default();
    prefs.set_int("SwitchKeyStatus", 0x9fe).unwrap();
    prefs.set_int("EmojiPicker", 1).unwrap();
    prefs
        .set_int("EmojiHotkey", (0x900 | KEY_E) as i32)
        .unwrap();
    let mut session = DesktopSession::new(Platform::Mac, Box::new(prefs));
    session.modifier(Modifiers::CONTROL, true, 0.1);
    session.modifier(Modifiers::SHIFT, true, 0.2);
    let first = session.key_down(KEY_E, true, Modifiers(3), 0.3);
    assert!(first.consumed);
    assert_eq!(first.effects, [Effect::OpenEmoji]);
    let repeat = session.key_down_event(KEY_E, true, Modifiers(3), true, 0.4);
    assert!(repeat.consumed);
    assert!(repeat.effects.is_empty());
    assert!(!session.modifier(Modifiers::SHIFT, false, 0.5).consumed);
    assert!(!session.modifier(Modifiers::CONTROL, false, 0.6).consumed);
    assert_eq!(session.language(), Language::Vietnamese);
    session.modifier(Modifiers::CONTROL, true, 0.7);
    session.modifier(Modifiers::SHIFT, true, 0.8);
    assert!(session.modifier(Modifiers::SHIFT, false, 0.9).consumed);
    assert_eq!(session.language(), Language::English);
}

#[test]
fn mac_startup_clears_invalid_emoji_chords_and_keeps_valid_ones() {
    use sevi_runtime::session::{same_hotkey_keys, valid_mac_emoji_hotkey};
    assert!(same_hotkey_keys(0x7a000206, 0x8206));
    assert!(!valid_mac_emoji_hotkey(0x800 | KEY_E, 0x7a000206));
    for (emoji, expected) in [
        (0x800 | KEY_E, 0),
        (0x206, 0),
        (0x8206, 0),
        (0x900 | KEY_E, 0x900 | KEY_E),
    ] {
        let mut prefs = MemoryPreferences::default();
        prefs.set_int("EmojiPicker", 1).unwrap();
        prefs.set_int("EmojiHotkey", emoji as i32).unwrap();
        let session = DesktopSession::new(Platform::Mac, Box::new(prefs));
        assert_eq!(session.emoji_hotkey, expected);
        assert_eq!(session.preferences.int("EmojiHotkey", -1), expected as i32);
    }
}

#[test]
fn click_or_switch_discards_the_saved_english_navigation_word() {
    for platform in [Platform::Windows, Platform::Linux, Platform::Mac] {
        for interruption in ["click", "switch", "round_trip"] {
            let mut prefs = MemoryPreferences::default();
            prefs.set_int("InputMethod", 0).unwrap();
            prefs.set_int("PauseAutoAfterManual", 0).unwrap();
            let mut s = DesktopSession::new(platform, Box::new(prefs));
            let mut text = vec![];
            let mut now = 0.;
            for c in "meeting th".chars() {
                let (k, shift) = sevi_test_tools::key_for(c).unwrap();
                key(&mut s, &mut text, k, shift, c, &mut now);
            }
            effects(
                &s.key_down(KEY_LEFT, false, Modifiers::default(), now + 0.01),
                &mut text,
            );
            match interruption {
                "click" => {
                    s.click();
                }
                "switch" => {
                    s.set_language_user(Language::Vietnamese, now + 0.02, "hotkey", false);
                    s.set_language_user(Language::English, now + 0.03, "hotkey", false);
                }
                _ => (),
            }
            assert!(s.engine.config.auto_switch);
            effects(
                &s.key_down(KEY_RIGHT, false, Modifiers::default(), now + 0.04),
                &mut text,
            );
            now += 0.05;
            for c in "if ".chars() {
                let (k, shift) = sevi_test_tools::key_for(c).unwrap();
                key(&mut s, &mut text, k, shift, c, &mut now);
            }
            assert_eq!(
                text.iter().collect::<String>(),
                if interruption == "round_trip" {
                    "meeting thì "
                } else {
                    "meeting thif "
                },
                "{platform:?}: {interruption}"
            );
        }
    }
}
#[test]
fn removed_history_stays_off_after_reload_on_every_platform() {
    for platform in [Platform::Windows, Platform::Linux, Platform::Mac] {
        let mut p = MemoryPreferences::default();
        p.set_int("TypingLogEnabled", 1).unwrap();
        let mut session = DesktopSession::new(platform, Box::new(p));
        assert!(!session.log.enabled);
        session.reload_settings(1.);
        session.key_down(KEY_A, false, Modifiers::default(), 1.1);
        session.key_down(KEY_SPACE, false, Modifiers::default(), 1.2);
        assert!(!session.log.enabled);
        assert!(session.log.pending.is_empty());
    }
}
