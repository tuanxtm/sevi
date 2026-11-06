use serde_json::{Value, json};
use sevi_engine::Language;
use sevi_runtime::{
    DesktopSession, Platform,
    prefs::{JsonPreferences, MemoryPreferences, Preferences},
    typing_log::{TypingLog, read_log_tail},
};
use std::{fs, io, path::PathBuf};
fn scratch(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/persistence-tests/{name}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn stats_are_buffered_survive_reload_and_do_not_return_after_clear() {
    let root = scratch("stats");
    let mut prefs = JsonPreferences::new(root.join("config"), root.join("data")).unwrap();
    prefs.set_int("TLStat_day_words", 1).unwrap();
    assert!(!root.join("data/stats.json").exists());
    prefs.set_int("Spelling", 1).unwrap();
    prefs.reload().unwrap();
    assert_eq!(prefs.int("TLStat_day_words", 0), 1);
    prefs.flush().unwrap();
    let disk: Value =
        serde_json::from_slice(&fs::read(root.join("data/stats.json")).unwrap()).unwrap();
    assert_eq!(disk["TLStat_day_words"], 1);
    prefs.set_int("TLStat_day_words", 2).unwrap();
    prefs.reload().unwrap();
    assert_eq!(prefs.int("TLStat_day_words", 0), 2);
    prefs.clear_stats().unwrap();
    prefs.reload().unwrap();
    prefs.flush().unwrap();
    assert_eq!(prefs.int("TLStat_day_words", 0), 0);
    assert_eq!(prefs.int("Spelling", 0), 1);
    assert_eq!(
        fs::read_to_string(root.join("data/stats.json")).unwrap(),
        "{}"
    );
}
#[test]
fn failed_stats_flush_keeps_pending_values_for_retry() {
    let root = scratch("retry");
    let data = root.join("data");
    let mut prefs = JsonPreferences::new(root.join("config"), data.clone()).unwrap();
    fs::write(&data, b"blocked directory").unwrap();
    prefs.set_int("TLStat_day_words", 7).unwrap();
    assert!(prefs.flush().is_err());
    prefs.reload().unwrap();
    assert_eq!(prefs.int("TLStat_day_words", 0), 7);
    fs::rename(&data, root.join("blocked-directory.backup")).unwrap();
    prefs.flush().unwrap();
    let restored = JsonPreferences::new(root.join("config"), data).unwrap();
    assert_eq!(restored.int("TLStat_day_words", 0), 7);
}
#[derive(Default)]
struct ObservedPreferences {
    values: MemoryPreferences,
    writes: usize,
    fail: bool,
}
impl Preferences for ObservedPreferences {
    fn get(&self, key: &str) -> Option<Value> {
        self.values.get(key)
    }
    fn keys(&self) -> Vec<String> {
        self.values.keys()
    }
    fn remove(&mut self, key: &str) -> io::Result<()> {
        self.values.remove(key)
    }
    fn set(&mut self, key: &str, value: Value) -> io::Result<()> {
        self.writes += 1;
        if self.fail {
            return Err(io::Error::other("simulated failed preference write"));
        }
        self.values.set(key, value)
    }
}
#[test]
fn mac_stats_snapshot_precedes_flush_and_unchanged_stats_are_not_rewritten() {
    let mut prefs = ObservedPreferences::default();
    let mut log = TypingLog::default();
    log.enabled = true;
    log.auto_switch("double_key", Language::English, &mut prefs, Platform::Mac);
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(log.stats_snapshot(&prefs)[&day]["auto_double_key"], 1);
    assert_eq!(prefs.writes, 0);
    prefs.fail = true;
    log.take(&mut prefs);
    assert_eq!(prefs.writes, 1);
    prefs.fail = false;
    log.take(&mut prefs);
    assert_eq!(prefs.writes, 2);
    log.take(&mut prefs);
    assert_eq!(prefs.writes, 2);
    log.clear(&mut prefs, Platform::Mac);
    assert_eq!(log.stats_snapshot(&prefs), json!({}));
    log.take(&mut prefs);
    assert_eq!(prefs.get("TypingLogStats"), None);
}
#[test]
fn tail_reader_retains_boundary_record_and_skips_partial_unicode_record() {
    let root = scratch("tail");
    let path = root.join("typing-log.jsonl");
    let first = "{\"e\":\"ngựa\"}\n";
    let last = "{\"e\":\"new\"}\n";
    fs::write(&path, format!("{first}{last}")).unwrap();
    assert_eq!(
        read_log_tail(&path, last.len(), 300).unwrap(),
        vec![json!({"e":"new"})]
    );
    assert_eq!(
        read_log_tail(&path, last.len() + 5, 300).unwrap(),
        vec![json!({"e":"new"})]
    );
    assert_eq!(
        read_log_tail(&path, 1024, 1).unwrap(),
        vec![json!({"e":"new"})]
    );
    assert_eq!(
        read_log_tail(&path, 1024, 300).unwrap(),
        vec![json!({"e":"new"}), json!({"e":"ngựa"})]
    );
}
#[test]
fn mac_smart_memory_serializes_after_the_event() {
    let mut mac = DesktopSession::new(Platform::Mac, Box::new(MemoryPreferences::default()));
    mac.focus_changed("test.app", 0.0);
    assert!(mac.smart_preferences_dirty());
    assert!(mac.preferences.binary("smartSwitchKey").is_empty());
    mac.flush_smart_preferences().unwrap();
    assert!(!mac.smart_preferences_dirty());
    assert!(!mac.preferences.binary("smartSwitchKey").is_empty());
    let mut windows =
        DesktopSession::new(Platform::Windows, Box::new(MemoryPreferences::default()));
    windows.focus_changed("test.exe", 0.0);
    assert!(!windows.smart_preferences_dirty());
    assert!(!windows.preferences.binary("smartSwitchKey").is_empty());
}
