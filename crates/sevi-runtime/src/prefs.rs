use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Native adapters preserve their existing domain, registry types and migration markers.
pub trait Preferences {
    fn get(&self, key: &str) -> Option<Value>;
    fn set(&mut self, key: &str, value: Value) -> io::Result<()>;
    fn remove(&mut self, key: &str) -> io::Result<()>;
    fn keys(&self) -> Vec<String>;
    fn reload(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn changed_externally(&self) -> bool {
        false
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn clear_stats(&mut self) -> io::Result<()> {
        for key in self.keys() {
            if key.starts_with("TLStat_") {
                self.remove(&key)?;
            }
        }
        self.flush()
    }
    fn int(&self, key: &str, fallback: i32) -> i32 {
        self.get(key)
            .and_then(|v| {
                v.as_i64()
                    .map(|n| n as i32)
                    .or_else(|| v.as_bool().map(i32::from))
            })
            .unwrap_or(fallback)
    }
    fn string(&self, key: &str) -> String {
        self.get(key)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
    fn binary(&self, key: &str) -> Vec<u8> {
        let s = self.string(key);
        decode_binary(&s)
    }
    fn set_int(&mut self, key: &str, n: i32) -> io::Result<()> {
        self.set(key, Value::from(n))
    }
    fn set_binary(&mut self, key: &str, data: &[u8]) -> io::Result<()> {
        self.set(key, Value::from(STANDARD.encode(data)))
    }
}
/// The legacy JSON decoder ignores non-base64 characters, including padding.
pub fn decode_binary(s: &str) -> Vec<u8> {
    let mut out = vec![];
    let (mut val, mut bits) = (0u32, -8);
    for c in s.bytes() {
        let Some(n) = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .iter()
            .position(|b| *b == c)
        else {
            continue;
        };
        val = val.wrapping_shl(6) | (n as u32);
        bits += 6;
        if bits >= 0 {
            out.push((val >> bits) as u8);
            bits -= 8
        }
    }
    out
}
#[derive(Default)]
pub struct MemoryPreferences(pub BTreeMap<String, Value>);
impl Preferences for MemoryPreferences {
    fn get(&self, k: &str) -> Option<Value> {
        self.0.get(k).cloned()
    }
    fn set(&mut self, k: &str, v: Value) -> io::Result<()> {
        self.0.insert(k.into(), v);
        Ok(())
    }
    fn remove(&mut self, k: &str) -> io::Result<()> {
        self.0.remove(k);
        Ok(())
    }
    fn keys(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
}
pub struct JsonPreferences {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    values: BTreeMap<String, Value>,
    stats: BTreeMap<String, Value>,
    stats_dirty: bool,
    own_write: Option<SystemTime>,
}
impl JsonPreferences {
    pub fn linux() -> io::Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let data = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        Self::new(config.join("sevi"), data.join("sevi"))
    }
    pub fn new(config_dir: PathBuf, data_dir: PathBuf) -> io::Result<Self> {
        let mut s = Self {
            config_dir,
            data_dir,
            values: BTreeMap::new(),
            stats: BTreeMap::new(),
            stats_dirty: false,
            own_write: None,
        };
        s.reload()?;
        Ok(s)
    }
    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
    pub fn command_path(&self) -> PathBuf {
        self.config_dir.join("command")
    }
    pub fn send_command(&self, line: &str) -> io::Result<()> {
        private_directory(&self.config_dir)?;
        let temporary = self.command_path().with_extension("tmp");
        fs::write(&temporary, format!("{line}\n"))?;
        fs::rename(temporary, self.command_path())
    }
    fn read(path: &Path, values: &mut BTreeMap<String, Value>) -> io::Result<()> {
        match fs::read(path) {
            Ok(data) => {
                if let Ok(new) = serde_json::from_slice(&data) {
                    *values = new
                }
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                values.clear();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    fn write(&mut self, key: &str, value: Option<Value>) -> io::Result<()> {
        let stats = key.starts_with("TLStat_");
        let path = if stats {
            self.data_dir.join("stats.json")
        } else {
            self.settings_path()
        };
        if !stats {
            Self::read(&path, &mut self.values)?
        }
        let values = if stats {
            &mut self.stats
        } else {
            &mut self.values
        };
        if let Some(value) = value {
            values.insert(key.into(), value);
        } else {
            values.remove(key);
        }
        if stats {
            self.stats_dirty = true;
            return Ok(());
        }
        atomic_json(&path, values)?;
        if !stats {
            self.own_write = fs::metadata(&path)?.modified().ok()
        }
        Ok(())
    }
}
pub fn private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}
pub fn atomic_json(path: &Path, value: &impl serde::Serialize) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing settings directory"))?;
    private_directory(parent)?;
    let tmp = path.with_extension(format!("json.tmp{}", std::process::id()));
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp)?;
    serde_json::to_writer_pretty(&file, value)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, path)
}
impl Preferences for JsonPreferences {
    fn get(&self, key: &str) -> Option<Value> {
        if key.starts_with("TLStat_") {
            self.stats.get(key)
        } else {
            self.values.get(key)
        }
        .cloned()
    }
    fn set(&mut self, key: &str, value: Value) -> io::Result<()> {
        self.write(key, Some(value))
    }
    fn remove(&mut self, key: &str) -> io::Result<()> {
        self.write(key, None)
    }
    fn keys(&self) -> Vec<String> {
        self.values
            .keys()
            .chain(self.stats.keys())
            .cloned()
            .collect()
    }
    fn reload(&mut self) -> io::Result<()> {
        Self::read(&self.settings_path(), &mut self.values)?;
        if !self.stats_dirty {
            Self::read(&self.data_dir.join("stats.json"), &mut self.stats)?;
        }
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.stats_dirty {
            atomic_json(&self.data_dir.join("stats.json"), &self.stats)?;
            self.stats_dirty = false;
        }
        Ok(())
    }
    fn clear_stats(&mut self) -> io::Result<()> {
        self.stats.clear();
        self.stats_dirty = true;
        self.flush()
    }
    fn changed_externally(&self) -> bool {
        fs::metadata(self.settings_path())
            .and_then(|m| m.modified())
            .is_ok_and(|t| Some(t) != self.own_write)
    }
}
