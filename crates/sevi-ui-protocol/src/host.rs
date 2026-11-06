//! A renderer is a disposable child process. The host never waits for UI I/O.
use super::*;
use std::{
    io::BufReader,
    process::{Child, Command, Stdio},
    sync::{Arc, mpsc},
    thread,
};

struct Client {
    child: Child,
    events: mpsc::Sender<UiEvent>,
    commands: mpsc::Receiver<Incoming>,
    ended: bool,
}
enum Incoming {
    Command(UiCommand),
    Ended,
}
#[derive(Default)]
pub struct UiHost {
    client: Option<Client>,
    // An independent popup keeps an existing settings window and its focus
    // intact. Both use the same small UI executable and private pipe protocol.
    popup: Option<Box<UiHost>>,
    pub capturing: bool,
    pub picker_window: u64,
    pub picker_open: bool,
}
impl UiHost {
    pub fn pid(&self) -> Option<u32> {
        self.client.as_ref().map(|c| c.child.id())
    }
    pub fn connected(&self) -> bool {
        self.client.is_some() || self.popup.as_ref().is_some_and(|p| p.connected())
    }
    pub fn picker_pid(&self) -> Option<u32> {
        self.popup.as_ref().and_then(|p| p.pid())
    }
    pub fn send(&self, event: UiEvent) {
        if let Some(popup) = &self.popup {
            match &event {
                UiEvent::Input(_) => {
                    popup.send(event);
                    return;
                }
                UiEvent::Snapshot(_) | UiEvent::Language(_) | UiEvent::Shutdown => {
                    popup.send(event.clone())
                }
                _ => (),
            }
        }
        if let Some(client) = &self.client {
            let _ = client.events.send(event);
        }
    }
    pub fn open(
        &mut self,
        command: Command,
        view: View,
        snapshot: UiSnapshot,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<()> {
        if view == View::Emoji {
            return self
                .popup
                .get_or_insert_with(|| Box::new(Self::default()))
                .open_single(command, view, snapshot, wake);
        }
        self.open_single(command, view, snapshot, wake)
    }
    fn open_single(
        &mut self,
        mut command: Command,
        view: View,
        snapshot: UiSnapshot,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<()> {
        self.reap();
        if self.client.is_some() {
            self.send(UiEvent::Snapshot(Box::new(snapshot)));
            self.send(UiEvent::Open(view));
            return Ok(());
        }
        let mut child = command
            .arg("--sevi-ui-client")
            .arg(format!("--sevi-ui-view={view:?}"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("missing UI input pipe"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing UI output pipe"))?;
        let (event_tx, event_rx) = mpsc::channel();
        let (command_tx, command_rx) = mpsc::channel();
        let wake = Arc::new(wake);
        thread::Builder::new()
            .name("sevi-ui-write".into())
            .spawn(move || {
                for event in event_rx {
                    if write(&mut input, &event).is_err() {
                        break;
                    }
                }
            })?;
        thread::Builder::new()
            .name("sevi-ui-read".into())
            .spawn(move || {
                let mut reader = BufReader::new(output);
                while let Ok(Some(request)) = read::<UiCommand>(&mut reader) {
                    if command_tx.send(Incoming::Command(request)).is_err() {
                        break;
                    }
                    wake();
                }
                let _ = command_tx.send(Incoming::Ended);
                wake();
            })?;
        self.client = Some(Client {
            child,
            events: event_tx,
            commands: command_rx,
            ended: false,
        });
        self.send(UiEvent::Snapshot(Box::new(snapshot)));
        Ok(())
    }
    pub fn requests(&mut self) -> Vec<UiCommand> {
        let mut requests = vec![];
        if let Some(client) = &mut self.client {
            while let Ok(request) = client.commands.try_recv() {
                match request {
                    Incoming::Command(command) => requests.push(command),
                    Incoming::Ended => client.ended = true,
                }
            }
        }
        if let Some(popup) = &mut self.popup {
            requests.extend(popup.requests());
        }
        for request in &requests {
            match request {
                UiCommand::Capture(on) => self.capturing = *on,
                UiCommand::PickerReady(window) => {
                    self.picker_window = *window;
                    self.picker_open = true;
                }
                UiCommand::PickerClosed => {
                    self.picker_window = 0;
                    self.picker_open = false;
                }
                _ => (),
            }
        }
        self.reap();
        requests
    }
    pub fn reap(&mut self) -> bool {
        let ended = self
            .client
            .as_mut()
            .is_some_and(|c| c.ended && c.child.try_wait().ok().flatten().is_some());
        if ended {
            self.client = None;
            self.capturing = false;
        }
        let popup_ended = self.popup.as_mut().is_some_and(|p| p.reap());
        if self.picker_pid().is_none() {
            self.picker_window = 0;
            self.picker_open = false;
        }
        ended || popup_ended
    }
    pub fn close(&mut self) {
        self.send(UiEvent::Shutdown);
    }
}
impl Drop for UiHost {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn delivers_last_command_before_reaping_and_reopens() {
        let mut host = UiHost::default();
        for _ in 0..3 {
            let mut command = Command::new("sh");
            command.args([
                "-c",
                r#"read handshake; printf '%s\n' '{"SetLanguage":false}' '"Close"'"#,
            ]);
            let (tx, rx) = mpsc::channel();
            host.open(command, View::Settings, UiSnapshot::default(), move || {
                let _ = tx.send(());
            })
            .unwrap();
            let mut received = vec![];
            let start = std::time::Instant::now();
            while host.connected() && start.elapsed() < std::time::Duration::from_secs(3) {
                let _ = rx.recv_timeout(std::time::Duration::from_millis(10));
                received.extend(host.requests());
            }
            assert!(!host.connected());
            assert!(matches!(
                received.as_slice(),
                [UiCommand::SetLanguage(false), UiCommand::Close]
            ));
            assert_eq!(host.pid(), None);
            assert!(!host.capturing);
            assert!(!host.picker_open);
        }
    }
    #[test]
    fn popup_exit_keeps_the_main_window_host_alive() {
        let mut host = UiHost::default();
        let mut main = Command::new("sh");
        main.args(["-c",r#"read handshake; while read event; do case "$event" in *Shutdown*) exit 0;; esac; done"#]);
        host.open(main, View::Settings, UiSnapshot::default(), || {})
            .unwrap();
        let pid = host.pid();
        let mut popup = Command::new("sh");
        popup.args(["-c", r#"read handshake; printf '%s\n' '"PickerClosed"'"#]);
        host.open(popup, View::Emoji, UiSnapshot::default(), || {})
            .unwrap();
        assert!(host.picker_pid().is_some());
        let start = std::time::Instant::now();
        let mut commands = vec![];
        while host.picker_pid().is_some() && start.elapsed() < std::time::Duration::from_secs(3) {
            commands.extend(host.requests());
            std::thread::yield_now();
        }
        assert!(matches!(commands.as_slice(), [UiCommand::PickerClosed]));
        assert_eq!(host.pid(), pid);
        assert!(host.connected());
        host.close();
        let start = std::time::Instant::now();
        while host.connected() && start.elapsed() < std::time::Duration::from_secs(3) {
            host.requests();
            std::thread::yield_now();
        }
        assert!(!host.connected());
    }
}
