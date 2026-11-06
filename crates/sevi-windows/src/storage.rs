//! Keep registry and log I/O off the low-level keyboard hook's thread.
use serde_json::Value;
use sevi_runtime::prefs::Preferences;
use std::{collections::BTreeMap, io, sync::mpsc, thread};

type Snapshot = BTreeMap<String, Value>;
#[derive(Clone)]
enum Write {
    Value(Value),
    Binary(Vec<u8>),
    Remove,
}
enum Command {
    Write(String, Write),
    Task(Box<dyn FnOnce() + Send>),
    Flush(mpsc::Sender<io::Result<()>>),
    Reload(mpsc::Sender<io::Result<Snapshot>>),
    Stop,
}
fn disconnected() -> io::Error {
    io::Error::other("settings worker disconnected")
}
fn snapshot(prefs: &dyn Preferences) -> Snapshot {
    prefs
        .keys()
        .into_iter()
        .filter_map(|key| prefs.get(&key).map(|value| (key, value)))
        .collect()
}
fn write(prefs: &mut dyn Preferences, key: &str, value: &Write) -> io::Result<()> {
    match value {
        Write::Value(value) => prefs.set(key, value.clone()),
        Write::Binary(value) => prefs.set_binary(key, value),
        Write::Remove => prefs.remove(key),
    }
}
fn flush(prefs: &mut dyn Preferences, failed: &mut BTreeMap<String, Write>) -> io::Result<()> {
    let mut error = None;
    failed.retain(|key, value| match write(prefs, key, value) {
        Ok(()) => false,
        Err(e) => {
            error = Some(e);
            true
        }
    });
    prefs.flush()?;
    error.map_or(Ok(()), Err)
}

#[derive(Clone)]
pub(crate) struct Tasks(mpsc::Sender<Command>);
impl Tasks {
    pub(crate) fn run(&self, task: impl FnOnce() + Send + 'static) -> io::Result<()> {
        self.0
            .send(Command::Task(Box::new(task)))
            .map_err(|_| disconnected())
    }
}

pub(crate) struct BackgroundPreferences {
    values: Snapshot,
    sender: mpsc::Sender<Command>,
    worker: Option<thread::JoinHandle<()>>,
}
impl BackgroundPreferences {
    pub(crate) fn new(mut prefs: impl Preferences + Send + 'static) -> io::Result<Self> {
        let values = snapshot(&prefs);
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("sevi-storage".into())
            .spawn(move || {
                let mut failed = BTreeMap::new();
                while let Ok(command) = receiver.recv() {
                    match command {
                        Command::Write(key, value) => {
                            if write(&mut prefs, &key, &value).is_err() {
                                failed.insert(key, value);
                            } else {
                                failed.remove(&key);
                            }
                        }
                        Command::Task(task) => task(),
                        Command::Flush(reply) => {
                            let _ = reply.send(flush(&mut prefs, &mut failed));
                        }
                        Command::Reload(reply) => {
                            let result = flush(&mut prefs, &mut failed)
                                .and_then(|()| prefs.reload())
                                .map(|()| snapshot(&prefs));
                            let _ = reply.send(result);
                        }
                        Command::Stop => {
                            let _ = flush(&mut prefs, &mut failed);
                            break;
                        }
                    }
                }
            })?;
        Ok(Self {
            values,
            sender,
            worker: Some(worker),
        })
    }
    pub(crate) fn tasks(&self) -> Tasks {
        Tasks(self.sender.clone())
    }
    fn queue(&self, key: &str, value: Write) -> io::Result<()> {
        self.sender
            .send(Command::Write(key.into(), value))
            .map_err(|_| disconnected())
    }
}
impl Preferences for BackgroundPreferences {
    fn get(&self, key: &str) -> Option<Value> {
        self.values.get(key).cloned()
    }
    fn set(&mut self, key: &str, value: Value) -> io::Result<()> {
        self.queue(key, Write::Value(value.clone()))?;
        self.values.insert(key.into(), value);
        Ok(())
    }
    fn remove(&mut self, key: &str) -> io::Result<()> {
        self.queue(key, Write::Remove)?;
        self.values.remove(key);
        Ok(())
    }
    fn keys(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }
    fn binary(&self, key: &str) -> Vec<u8> {
        // RegistryPreferences represents REG_BINARY as a JSON byte array.
        self.values
            .get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
                    .collect()
            })
            .unwrap_or_default()
    }
    fn set_binary(&mut self, key: &str, bytes: &[u8]) -> io::Result<()> {
        self.queue(key, Write::Binary(bytes.into()))?;
        self.values
            .insert(key.into(), bytes.iter().copied().map(Value::from).collect());
        Ok(())
    }
    fn reload(&mut self) -> io::Result<()> {
        let (reply, response) = mpsc::channel();
        self.sender
            .send(Command::Reload(reply))
            .map_err(|_| disconnected())?;
        self.values = response.recv().map_err(|_| disconnected())??;
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        let (reply, response) = mpsc::channel();
        self.sender
            .send(Command::Flush(reply))
            .map_err(|_| disconnected())?;
        response.recv().map_err(|_| disconnected())?
    }
}
impl Drop for BackgroundPreferences {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sevi_engine::{Language, data::key_character, keys::KEY_RETURN};
    use sevi_runtime::{DesktopSession, Effect, EventResult, Modifiers, Platform};
    use std::{
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };

    #[derive(Clone, Default)]
    struct Registry {
        values: Arc<Mutex<Snapshot>>,
        fail: Arc<AtomicBool>,
    }
    impl Preferences for Registry {
        fn get(&self, key: &str) -> Option<Value> {
            self.values.lock().unwrap().get(key).cloned()
        }
        fn set(&mut self, key: &str, value: Value) -> io::Result<()> {
            if self.fail.load(Ordering::SeqCst) {
                return Err(io::Error::other("storage unavailable"));
            }
            self.values.lock().unwrap().insert(key.into(), value);
            Ok(())
        }
        fn remove(&mut self, key: &str) -> io::Result<()> {
            self.values.lock().unwrap().remove(key);
            Ok(())
        }
        fn keys(&self) -> Vec<String> {
            self.values.lock().unwrap().keys().cloned().collect()
        }
        fn set_binary(&mut self, key: &str, bytes: &[u8]) -> io::Result<()> {
            self.set(key, bytes.iter().copied().map(Value::from).collect())
        }
    }
    fn type_word(session: &mut DesktopSession, text: &str) -> Vec<EventResult> {
        text.chars()
            .enumerate()
            .map(|(i, c)| {
                let key = if c == '\n' {
                    KEY_RETURN
                } else {
                    (0..128).find(|k| key_character(*k) == c as u32).unwrap()
                };
                session.key_down(key, false, Modifiers::default(), 10.0 + i as f64 * 0.03)
            })
            .collect()
    }
    #[test]
    fn blocked_storage_does_not_block_typing_or_change_replacement() {
        for (input, logging) in [
            ("reference ", false),
            ("reference ", true),
            ("reference\n", false),
            ("reference\n", true),
        ] {
            let mut original = Registry::default();
            original
                .set_int("TypingLogEnabled", i32::from(logging))
                .unwrap();
            let mut direct = DesktopSession::new(Platform::Windows, Box::new(original));
            let expected = type_word(&mut direct, input);
            assert_eq!(direct.language(), Language::English);
            let mut registry = Registry::default();
            registry
                .set_int("TypingLogEnabled", i32::from(logging))
                .unwrap();
            let saved = registry.clone();
            let background = BackgroundPreferences::new(registry).unwrap();
            let tasks = background.tasks();
            let mut session = DesktopSession::new(Platform::Windows, Box::new(background));
            let (ready, started) = mpsc::channel();
            let (release, blocked) = mpsc::channel();
            let timed_out = Arc::new(AtomicBool::new(false));
            let timeout = timed_out.clone();
            tasks
                .run(move || {
                    ready.send(()).unwrap();
                    timeout.store(
                        blocked.recv_timeout(Duration::from_secs(3)).is_err(),
                        Ordering::SeqCst,
                    );
                })
                .unwrap();
            started.recv_timeout(Duration::from_secs(3)).unwrap();
            let now = Instant::now();
            let actual = type_word(&mut session, input);
            let elapsed = now.elapsed();
            release.send(()).unwrap();
            session.preferences.flush().unwrap();
            assert!(
                !timed_out.load(Ordering::SeqCst),
                "typing waited for blocked storage"
            );
            assert_eq!(actual, expected);
            assert_eq!(session.language(), Language::English);
            assert_eq!(saved.int("InputMethod", -1), 0);
            assert!(!session.preferences.binary("smartSwitchKey").is_empty());
            assert!(!session.log.enabled);
            assert!(session.log.pending.is_empty());
            assert!(saved.keys().iter().all(|key| !key.starts_with("TLStat_")));
            println!(
                "{input:?}: {elapsed:?} for 10 keys while storage was blocked; logging={logging}"
            );
            for (i, result) in actual.iter().enumerate() {
                if result
                    .effects
                    .iter()
                    .any(|e| matches!(e, Effect::Backspaces(_)))
                {
                    println!("  key {i}: {:?}", result.effects);
                }
            }
        }
    }
    #[test]
    fn registry_types_order_reload_and_shutdown_are_preserved() {
        let mut registry = Registry::default();
        registry.set_binary("smartSwitchKey", &[0, 255, 1]).unwrap();
        let saved = registry.clone();
        {
            let mut p = BackgroundPreferences::new(registry).unwrap();
            assert_eq!(p.binary("smartSwitchKey"), [0, 255, 1]);
            p.set_binary("smartSwitchKey", &[2, 0, 7]).unwrap();
            p.set_int("InputMethod", 1).unwrap();
            p.set_int("InputMethod", 0).unwrap();
            p.set_int("removed", 1).unwrap();
            p.remove("removed").unwrap();
            p.flush().unwrap();
            let expected = p.values.clone();
            p.reload().unwrap();
            assert_eq!(p.values, expected);
            assert_eq!(p.binary("smartSwitchKey"), [2, 0, 7]);
            let mut external = saved.clone();
            external.set_int("AutoSwitchByTyping", 0).unwrap();
            p.reload().unwrap();
            assert_eq!(p.int("AutoSwitchByTyping", 1), 0);
            p.set_int("shutdown", 42).unwrap();
        }
        assert_eq!(saved.int("shutdown", 0), 42);
        assert_eq!(saved.int("InputMethod", -1), 0);
        assert!(saved.get("removed").is_none());
    }
    #[test]
    fn failed_writes_retry_and_latest_value_wins() {
        let registry = Registry::default();
        let saved = registry.clone();
        let mut p = BackgroundPreferences::new(registry).unwrap();
        saved.fail.store(true, Ordering::SeqCst);
        p.set_int("InputMethod", 1).unwrap();
        p.set_int("retained", 5).unwrap();
        assert!(p.flush().is_err());
        assert_eq!(p.int("InputMethod", -1), 1);
        assert!(p.reload().is_err());
        assert_eq!(p.int("InputMethod", -1), 1);
        saved.fail.store(false, Ordering::SeqCst);
        p.set_int("InputMethod", 0).unwrap();
        p.flush().unwrap();
        assert_eq!(saved.int("InputMethod", -1), 0);
        assert_eq!(saved.int("retained", 0), 5);
    }
}
