//! A serial worker keeps file access out of the event tap and orders reads after writes.
use serde_json::Value;
use std::{io::Write, path::PathBuf, sync::mpsc};
enum Request {
    Append(String),
    Clear,
    Read(mpsc::Sender<Vec<Value>>),
    Sync(mpsc::Sender<()>),
}
pub struct LogIo(mpsc::Sender<Request>);
impl LogIo {
    pub fn new(directory: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let path = directory.join("typing-log.jsonl");
            for request in receiver {
                match request {
                    Request::Append(text) => {
                        if sevi_runtime::prefs::private_directory(&directory).is_err() {
                            continue;
                        }
                        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 20 * 1024 * 1024) {
                            let _ = std::fs::rename(&path, directory.join("typing-log.1.jsonl"));
                        }
                        let mut options = std::fs::OpenOptions::new();
                        options.create(true).append(true);
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::OpenOptionsExt;
                            options.mode(0o600);
                        }
                        if let Ok(mut file) = options.open(&path) {
                            let _ = file.write_all(text.as_bytes());
                        }
                    }
                    Request::Clear => {
                        let _ = sevi_runtime::typing_log::archive_logs(&directory);
                    }
                    Request::Read(reply) => {
                        let events =
                            sevi_runtime::typing_log::read_log_tail(&path, 512 * 1024, 300)
                                .unwrap_or_default();
                        let _ = reply.send(events);
                    }
                    Request::Sync(reply) => {
                        let _ = reply.send(());
                    }
                }
            }
        });
        Self(sender)
    }
    pub fn append(&self, text: String) {
        let _ = self.0.send(Request::Append(text));
    }
    pub fn clear(&self) {
        let _ = self.0.send(Request::Clear);
    }
    pub fn read(&self) -> mpsc::Receiver<Vec<Value>> {
        let (sender, receiver) = mpsc::channel();
        let _ = self.0.send(Request::Read(sender));
        receiver
    }
    pub fn sync(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.0.send(Request::Sync(sender)).is_ok() {
            let _ = receiver.recv();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_reads_and_clear_remain_ordered() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/log-worker-tests/{}-{stamp}",
            std::process::id()
        ));
        let io = LogIo::new(directory.clone());
        io.append("{\"e\":\"old\"}\n".into());
        let before = io.read();
        io.clear();
        io.append("{\"e\":\"new\"}\n".into());
        let after = io.read();
        io.sync();
        assert_eq!(before.recv().unwrap()[0]["e"], "old");
        assert_eq!(after.recv().unwrap()[0]["e"], "new");
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 2);
    }
}
