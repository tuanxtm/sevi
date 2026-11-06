use sevi_engine::{Driver, EngineConfig, Language, TelexStyle};
use sevi_test_tools::{Harness, key_for};
use std::path::Path;
fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}
#[test]
fn checked_in_syllable_snapshot() {
    let source =
        std::fs::read_to_string(root().join("Tools/autoswitch/rules/golden/telex_vn_mode.tsv"))
            .unwrap();
    let mut h = Harness::new(EngineConfig::default());
    for line in source.lines() {
        let (keys, expected) = line.split_once('\t').unwrap();
        h.reset(Language::Vietnamese);
        for c in keys.chars().chain(std::iter::once(' ')) {
            h.press(c)
        }
        assert_eq!(h.line(), expected, "keys: {keys}")
    }
}
#[test]
fn independent_engine_instances() {
    let mut vi = Driver::default();
    let mut en = Driver::new(EngineConfig {
        language: Language::English,
        auto_switch: false,
        ..Default::default()
    });
    let mut output = vec![];
    for c in "truowfng ".chars() {
        let (k, s) = key_for(c).unwrap();
        let edit = vi.press(k, s, 0.0);
        output.truncate(output.len().saturating_sub(edit.delete_count));
        output.extend(edit.text);
        let _ = en.press(k, s, 0.0);
        assert_eq!(en.language(), Language::English)
    }
    assert_eq!(String::from_utf16(&output).unwrap(), "trường ")
}
#[test]
fn all_telex_styles_are_distinct() {
    for (style, w) in [
        (TelexStyle::Telex, "ư"),
        (TelexStyle::Simple1, "w"),
        (TelexStyle::Simple2, "ư"),
    ] {
        let mut h = Harness::new(EngineConfig {
            auto_switch: false,
            telex_style: style,
            ..Default::default()
        });
        h.press('w');
        assert_eq!(h.text(), w)
    }
}

#[test]
fn standalone_bracket_preserves_spelling_state() {
    let mut h = Harness::new(EngineConfig::default());
    for c in "ieefu[reaz)s.wwa@muokti".chars() {
        h.press(c)
    }
    assert_eq!(h.text(), "iêuờreaz)s.wamuokti");
    assert_eq!(h.engine.config.language, Language::Vietnamese);
}
