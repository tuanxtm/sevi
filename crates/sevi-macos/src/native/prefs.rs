//! NSUserDefaults retains the existing property-list types, domains and binary memory.
use objc2::{
    class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
};
use objc2_foundation::{NSData, NSString, NSUserDefaults};
use serde_json::{Value, json};
use sevi_runtime::prefs::Preferences;
use std::{io, path::PathBuf};
#[derive(Default)]
pub struct Defaults;
pub fn object(value: &Value) -> Option<Retained<AnyObject>> {
    let data = NSData::with_bytes(&serde_json::to_vec(value).ok()?);
    unsafe {
        msg_send![class!(NSJSONSerialization), JSONObjectWithData: &*data, options: 4usize, error: std::ptr::null_mut::<*mut AnyObject>()]
    }
}
pub fn value(object: &AnyObject) -> Option<Value> {
    unsafe {
        let data: Option<Retained<NSData>> = msg_send![class!(NSJSONSerialization), dataWithJSONObject: object, options: 4usize, error: std::ptr::null_mut::<*mut AnyObject>()];
        serde_json::from_slice(&data?.to_vec()).ok()
    }
}
pub fn text(object: Option<Retained<NSString>>) -> String {
    object.map(|s| s.to_string()).unwrap_or_default()
}
impl Preferences for Defaults {
    fn get(&self, key: &str) -> Option<Value> {
        let v = NSUserDefaults::standardUserDefaults().objectForKey(&NSString::from_str(key))?;
        value(&v)
    }
    fn set(&mut self, key: &str, v: Value) -> io::Result<()> {
        let v = object(&v).ok_or_else(|| io::Error::other("invalid preference value"))?;
        unsafe {
            NSUserDefaults::standardUserDefaults()
                .setObject_forKey(Some(&v), &NSString::from_str(key));
        }
        Ok(())
    }
    fn remove(&mut self, key: &str) -> io::Result<()> {
        NSUserDefaults::standardUserDefaults().removeObjectForKey(&NSString::from_str(key));
        Ok(())
    }
    fn keys(&self) -> Vec<String> {
        let d = NSUserDefaults::standardUserDefaults().dictionaryRepresentation();
        d.allKeys().iter().map(|s| s.to_string()).collect()
    }
    fn binary(&self, key: &str) -> Vec<u8> {
        NSUserDefaults::standardUserDefaults()
            .dataForKey(&NSString::from_str(key))
            .map(|d| d.to_vec())
            .unwrap_or_default()
    }
    fn set_binary(&mut self, key: &str, data: &[u8]) -> io::Result<()> {
        let d = NSData::with_bytes(data);
        unsafe {
            NSUserDefaults::standardUserDefaults()
                .setObject_forKey(Some(&d), &NSString::from_str(key));
        }
        Ok(())
    }
}
pub fn support_dir() -> PathBuf {
    // The app runs in the user's normal desktop domain, like the former host.
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support/Sevi")
}
pub fn register_defaults() {
    let d = NSUserDefaults::standardUserDefaults();
    let defaults = json!({"Spelling":1,"RestoreIfInvalidWord":1,"ModernOrthography":1,"UseMacro":1,"UseMacroInEnglishMode":1,"vAutoCapsMacro":0,
        "UseSmartSwitchKey":1,"SmartSwitchPerSite":false,"AutoSwitchByTyping":1,"PauseAutoAfterManual":1,"TypingLogKeys":1,"TypingLogEnabled":0,
        "SwitchKeyStatus":0x7a000206u32,"Teencode":1,"TelexStyle":0,"MainWindowTab":"general","ExcludedApps":crate::policy::EXCLUDED});
    if let Some(v) = object(&defaults) {
        unsafe {
            let _: () = msg_send![&*d, registerDefaults: &*v];
        }
    }
}
pub fn migrate() {
    register_defaults();
    let d = NSUserDefaults::standardUserDefaults();
    let old: Allocated<NSUserDefaults> = unsafe { msg_send![class!(NSUserDefaults), alloc] };
    let old: Option<Retained<NSUserDefaults>> =
        unsafe { msg_send![old, initWithSuiteName: &*NSString::from_str("com.tuyenmai.openkey")] };
    let mut prefs = Defaults;
    if prefs.int("MigratedTelexStyle", 0) == 0 {
        let domain = d.persistentDomainForName(&NSString::from_str("com.devuap.sevi"));
        let chosen =
            domain.is_some_and(|v| v.objectForKey(&NSString::from_str("TelexStyle")).is_some());
        if chosen {
            let _ = prefs.set_int("MigratedTelexStyle", 1);
        } else if let Some(t) = old
            .as_ref()
            .and_then(|o| o.objectForKey(&NSString::from_str("InputType")))
            .and_then(|o| value(&o))
            .and_then(|v| v.as_i64())
        {
            if t == 2 || t == 3 {
                let _ = prefs.set_int("TelexStyle", t as i32 - 1);
            }
            let _ = prefs.set_int("MigratedTelexStyle", 1);
        }
    }
    if prefs.int("MigratedFromOpenKey", 0) != 0 {
        return;
    }
    let _ = prefs.set_int("MigratedFromOpenKey", 1);
    if let Some(old) = old {
        for k in [
            "InputMethod",
            "Spelling",
            "RestoreIfInvalidWord",
            "ModernOrthography",
            "UseMacro",
            "UseMacroInEnglishMode",
            "vAutoCapsMacro",
            "UseSmartSwitchKey",
            "SmartSwitchPerSite",
            "AutoSwitchByTyping",
            "TypingLogKeys",
            "SwitchKeyStatus",
            "macroData",
            "smartSwitchKey",
            "TypingLogStats",
        ] {
            let key = NSString::from_str(k);
            if let Some(v) = old.objectForKey(&key) {
                unsafe {
                    d.setObject_forKey(Some(&v), &key);
                }
            }
        }
    }
    let to_dir = support_dir();
    let from = to_dir.parent().unwrap().join("OpenKey/typing-log.jsonl");
    let to = to_dir.join("typing-log.jsonl");
    if from.is_file() && !to.exists() && std::fs::create_dir_all(&to_dir).is_ok() {
        let _ = std::fs::copy(from, to);
    }
}
