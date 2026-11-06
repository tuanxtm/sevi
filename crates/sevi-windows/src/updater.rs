//! WinHTTP/Sparkle updater, retaining the existing download, prompt and installer flow.
use crate::{
    feed::{self, Release},
    prefs::{self, RegistryPreferences},
    wide,
};
use sevi_runtime::prefs::Preferences;
use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    ptr,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::*,
        Networking::WinHttp::*,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};
pub const WM_UPDATER: u32 = WM_APP + 4;
const UP_TO_DATE: usize = 1;
const FAILED: usize = 2;
const AVAILABLE: usize = 3;
const DOWNLOADED: usize = 4;
const TIMER_UPDATE: usize = 0x7001;
const TIMER_INITIAL: usize = 0x7002;
const VERSION: &str = env!("CARGO_PKG_VERSION");
const ARCH: &str = if cfg!(target_arch = "aarch64") {
    "arm64"
} else {
    "x64"
};
#[derive(Default, Clone)]
struct State {
    owner: isize,
    manual: bool,
    found: Release,
    pending: PathBuf,
    error: String,
}
static STATE: Mutex<State> = Mutex::new(State {
    owner: 0,
    manual: false,
    found: Release {
        version: String::new(),
        url: String::new(),
        signature: String::new(),
        notes: String::new(),
        length: 0,
    },
    pending: PathBuf::new(),
    error: String::new(),
});
static BUSY: AtomicBool = AtomicBool::new(false);
struct Internet(*mut c_void);
impl Internet {
    fn new(handle: *mut c_void) -> Result<Self, String> {
        if handle.is_null() {
            Err("WinHTTP connection failed".into())
        } else {
            Ok(Self(handle))
        }
    }
}
impl Drop for Internet {
    fn drop(&mut self) {
        unsafe {
            let _ = WinHttpCloseHandle(self.0);
        }
    }
}
fn http_get(url: &str, max: usize) -> Result<Vec<u8>, String> {
    unsafe {
        let url = wide(url);
        let mut host = [0u16; 256];
        let mut path = [0u16; 2048];
        let mut components = URL_COMPONENTS {
            dwStructSize: size_of::<URL_COMPONENTS>() as u32,
            lpszHostName: windows::core::PWSTR(host.as_mut_ptr()),
            dwHostNameLength: host.len() as u32,
            lpszUrlPath: windows::core::PWSTR(path.as_mut_ptr()),
            dwUrlPathLength: path.len() as u32,
            ..Default::default()
        };
        WinHttpCrackUrl(&url[..url.len() - 1], 0, &mut components).map_err(|e| e.to_string())?;
        let agent = wide(&format!("Sevi/{VERSION}"));
        let session = Internet::new(WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            None,
            None,
            0,
        ))?;
        let connection = Internet::new(WinHttpConnect(
            session.0,
            PCWSTR(host.as_ptr()),
            components.nPort,
            0,
        ))?;
        let request = Internet::new(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            PCWSTR(path.as_ptr()),
            None,
            None,
            ptr::null(),
            if components.nScheme == WINHTTP_INTERNET_SCHEME_HTTPS {
                WINHTTP_FLAG_SECURE
            } else {
                WINHTTP_OPEN_REQUEST_FLAGS(0)
            },
        ))?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|e| e.to_string())?;
        WinHttpReceiveResponse(request.0, ptr::null_mut()).map_err(|e| e.to_string())?;
        let (mut status, mut len) = (0u32, 4u32);
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            None,
            Some((&mut status as *mut u32).cast()),
            &mut len,
            ptr::null_mut(),
        )
        .map_err(|e| e.to_string())?;
        if status != 200 {
            return Err(format!("HTTP {status}"));
        }
        let mut bytes = vec![];
        loop {
            let mut available = 0;
            WinHttpQueryDataAvailable(request.0, &mut available).map_err(|e| e.to_string())?;
            if available == 0 {
                break;
            }
            let at = bytes.len();
            if available as usize > max.saturating_sub(at) {
                return Err("update is too large".into());
            }
            bytes.resize(at + available as usize, 0);
            let mut got = 0;
            WinHttpReadData(
                request.0,
                bytes[at..].as_mut_ptr().cast(),
                available,
                &mut got,
            )
            .map_err(|e| e.to_string())?;
            bytes.truncate(at + got as usize);
            if got == 0 {
                break;
            }
        }
        Ok(bytes)
    }
}
fn forget_pending() {
    let mut p = RegistryPreferences;
    let _ = p.remove("PendingUpdatePath");
    let _ = p.remove("PendingUpdateVersion");
    let _ = p.remove("PendingUpdateSignature");
    STATE.lock().unwrap().pending.clear();
}
fn download(release: &Release) -> Result<(), String> {
    let bytes = http_get(&release.url, 200 << 20).map_err(|_| "Could not download the update.")?;
    if release.length != 0 && release.length != bytes.len() as u64 {
        return Err("The downloaded file has an unexpected size.".into());
    }
    if !feed::verify(&bytes, &release.signature) {
        return Err("The update signature is invalid.".into());
    }
    let dir = prefs::data_dir()
        .map_err(|_| "Could not save the update.")?
        .join("updates");
    fs::create_dir_all(&dir).map_err(|_| "Could not save the update.")?;
    let path = dir.join(format!("Sevi-{}-{ARCH}-Setup.exe", release.version));
    fs::write(&path, bytes).map_err(|_| "Could not save the update.")?;
    let mut p = RegistryPreferences;
    let _ = p.set(
        "PendingUpdatePath",
        path.to_string_lossy().to_string().into(),
    );
    let _ = p.set("PendingUpdateVersion", release.version.clone().into());
    let _ = p.set("PendingUpdateSignature", release.signature.clone().into());
    STATE.lock().unwrap().pending = path;
    Ok(())
}
fn run_installer(path: &Path, silent: bool) -> bool {
    // Recheck the cached installer at the execution boundary, including after restart.
    let signature = RegistryPreferences.string("PendingUpdateSignature");
    if !fs::metadata(path).is_ok_and(|metadata| metadata.len() <= 200 << 20)
        || !fs::read(path).is_ok_and(|bytes| feed::verify(&bytes, &signature))
    {
        forget_pending();
        return false;
    }
    let path = wide(&path.to_string_lossy());
    let args = wide(&format!(
        "{} /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /RELAUNCH=1",
        if silent { "/VERYSILENT" } else { "/SILENT" }
    ));
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(path.as_ptr()),
            PCWSTR(args.as_ptr()),
            None,
            if silent { SW_HIDE } else { SW_SHOW },
        )
        .0 as isize
            > 32
    }
}
fn notify(event: usize, manual: bool, error: Option<String>) {
    let owner = {
        let mut s = STATE.lock().unwrap();
        s.manual = manual;
        if let Some(error) = error {
            s.error = error
        }
        s.owner
    };
    BUSY.store(false, Ordering::Release);
    unsafe {
        let _ = PostMessageW(
            Some(HWND(owner as *mut _)),
            WM_UPDATER,
            WPARAM(event),
            LPARAM(0),
        );
    }
}
fn check(manual: bool) {
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(move || {
        let url = feed::update_url(&RegistryPreferences);
        if url.trim().is_empty() {
            notify(
                FAILED,
                manual,
                Some("No update source is configured for Sevi.".into()),
            );
            return;
        }
        let Ok(bytes) = http_get(&url, 4 << 20) else {
            notify(
                FAILED,
                manual,
                Some("Could not check for updates. Check your connection and try again.".into()),
            );
            return;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;
        let _ = RegistryPreferences.set_int("LastUpdateCheck", now);
        let release = feed::newest(&String::from_utf8_lossy(&bytes), ARCH);
        let Some(release) = release.filter(|r| feed::compare_versions(&r.version, VERSION).is_gt())
        else {
            notify(UP_TO_DATE, manual, None);
            return;
        };
        STATE.lock().unwrap().found = release.clone();
        if manual {
            notify(AVAILABLE, true, None)
        } else {
            match download(&release) {
                Ok(()) => notify(DOWNLOADED, false, None),
                Err(e) => notify(FAILED, false, Some(e)),
            }
        }
    });
}
fn message(body: &str, _warning: bool) {
    crate::shared_ui::notice(body);
}
pub fn accept_update() {
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let release = STATE.lock().unwrap().found.clone();
    std::thread::spawn(move || match download(&release) {
        Ok(()) => notify(DOWNLOADED, true, None),
        Err(e) => notify(FAILED, true, Some(e)),
    });
}
pub fn on_event(event: usize) {
    let state = STATE.lock().unwrap().clone();
    if !state.manual {
        return;
    }
    match event {
        UP_TO_DATE => message(&format!("Sevi {VERSION} is up to date."), false),
        FAILED => message(&state.error, true),
        AVAILABLE => crate::shared_ui::offer(state.found.version, state.found.notes),
        DOWNLOADED => {
            if run_installer(&state.pending, false) {
                forget_pending();
                unsafe {
                    let _ = PostMessageW(
                        Some(HWND(state.owner as *mut _)),
                        WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            } else {
                message("Could not start the installer.", true)
            }
        }
        _ => (),
    }
}
unsafe extern "system" fn timer(owner: HWND, _: u32, id: usize, _: u32) {
    if id == TIMER_INITIAL {
        unsafe {
            let _ = KillTimer(Some(owner), TIMER_INITIAL);
        }
    }
    if RegistryPreferences.int("AutoUpdate", 1) == 0 {
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    if now - RegistryPreferences.int("LastUpdateCheck", 0) as i64 >= 86400 {
        check(false)
    }
}
pub fn start(owner: HWND) {
    let path = PathBuf::from(RegistryPreferences.string("PendingUpdatePath"));
    {
        let mut state = STATE.lock().unwrap();
        state.owner = owner.0 as isize;
        state.pending = path.clone();
    }
    if !path.as_os_str().is_empty() && !path.is_file() {
        forget_pending()
    }
    unsafe {
        SetTimer(Some(owner), TIMER_UPDATE, 3600000, Some(timer));
        SetTimer(Some(owner), TIMER_INITIAL, 20000, Some(timer));
    }
}
pub fn check_now(owner: HWND) {
    STATE.lock().unwrap().owner = owner.0 as isize;
    check(true)
}
pub fn install_pending(at_launch: bool) -> bool {
    let path = if at_launch {
        PathBuf::from(RegistryPreferences.string("PendingUpdatePath"))
    } else {
        STATE.lock().unwrap().pending.clone()
    };
    if path.as_os_str().is_empty() {
        return false;
    }
    let version = RegistryPreferences.string("PendingUpdateVersion");
    if !path.is_file() || !feed::compare_versions(&version, VERSION).is_gt() {
        if path.is_file() {
            let _ = fs::remove_file(&path);
        }
        forget_pending();
        return false;
    }
    if at_launch {
        let installed = run_installer(&path, true);
        forget_pending();
        installed
    } else {
        let ok = run_installer(&path, true);
        forget_pending();
        ok
    }
}
