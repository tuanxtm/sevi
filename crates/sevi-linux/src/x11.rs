//! Small X11 adapter. Pure Wayland deliberately retains the existing identity/position limitations.
use std::{
    ffi::{CStr, CString, c_char, c_int, c_uint, c_ulong, c_void},
    ptr,
};
#[repr(C)]
struct ClassHint {
    res_name: *mut c_char,
    res_class: *mut c_char,
}
#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut c_void;
    fn XCloseDisplay(display: *mut c_void) -> c_int;
    fn XGetInputFocus(display: *mut c_void, window: *mut c_ulong, revert: *mut c_int) -> c_int;
    fn XDefaultRootWindow(display: *mut c_void) -> c_ulong;
    fn XGetClassHint(display: *mut c_void, window: c_ulong, hint: *mut ClassHint) -> c_int;
    fn XFree(data: *mut c_void) -> c_int;
    fn XQueryTree(
        display: *mut c_void,
        window: c_ulong,
        root: *mut c_ulong,
        parent: *mut c_ulong,
        children: *mut *mut c_ulong,
        count: *mut c_uint,
    ) -> c_int;
    fn XSetErrorHandler(
        handler: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int>,
    ) -> Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int>;
    #[cfg(feature = "native-tests")]
    fn XQueryPointer(
        d: *mut c_void,
        w: c_ulong,
        root: *mut c_ulong,
        child: *mut c_ulong,
        rx: *mut c_int,
        ry: *mut c_int,
        wx: *mut c_int,
        wy: *mut c_int,
        mask: *mut c_uint,
    ) -> c_int;
    #[cfg(feature = "native-tests")]
    fn XMoveWindow(d: *mut c_void, w: c_ulong, x: c_int, y: c_int) -> c_int;
    #[cfg(feature = "native-tests")]
    fn XInternAtom(d: *mut c_void, name: *const c_char, only: c_int) -> c_ulong;
    #[cfg(feature = "native-tests")]
    fn XSendEvent(
        d: *mut c_void,
        w: c_ulong,
        propagate: c_int,
        mask: std::ffi::c_long,
        event: *mut ClientEvent,
    ) -> c_int;
    #[cfg(feature = "native-tests")]
    fn XFlush(d: *mut c_void) -> c_int;
}
#[repr(C)]
#[cfg(feature = "native-tests")]
struct ClientEvent {
    type_: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut c_void,
    window: c_ulong,
    message_type: c_ulong,
    format: c_int,
    data: [std::ffi::c_long; 5],
    padding: [std::ffi::c_long; 7],
}
#[cfg(feature = "native-tests")]
#[link(name = "gtk-4")]
unsafe extern "C" {
    fn gdk_x11_surface_get_xid(surface: *mut gtk::gdk::ffi::GdkSurface) -> c_ulong;
    fn gdk_x11_display_get_xdisplay(display: *mut gtk::gdk::ffi::GdkDisplay) -> *mut c_void;
}
/// Match the existing panel position and EWMH focus/above requests on X11.
#[cfg(feature = "native-tests")]
pub fn place_picker(w: &impl gtk::prelude::IsA<gtk::Window>) {
    use glib::translate::ToGlibPtr;
    use gtk::prelude::*;
    let Some(surface) = w.as_ref().surface() else {
        return;
    };
    let display = surface.display();
    if !display.type_().name().contains("X11") {
        return;
    }
    unsafe {
        let d = gdk_x11_display_get_xdisplay(display.to_glib_none().0);
        let xw = gdk_x11_surface_get_xid(surface.to_glib_none().0);
        let root = XDefaultRootWindow(d);
        let (mut rr, mut child, mut px, mut py, mut wx, mut wy, mut mask) = (0, 0, 0, 0, 0, 0, 0);
        XQueryPointer(
            d, root, &mut rr, &mut child, &mut px, &mut py, &mut wx, &mut wy, &mut mask,
        );
        let scale = surface.scale_factor();
        let monitors = display.monitors();
        let mut geometry = gtk::gdk::Rectangle::new(0, 0, 1920, 1080);
        for i in 0..monitors.n_items() {
            if let Some(m) = monitors.item(i).and_downcast::<gtk::gdk::Monitor>() {
                let g = m.geometry();
                if i == 0
                    || (px / scale >= g.x()
                        && px / scale < g.x() + g.width()
                        && py / scale >= g.y()
                        && py / scale < g.y() + g.height())
                {
                    geometry = g;
                }
            }
        }
        let x = (geometry.x() + (geometry.width() - 460) / 2) * scale;
        let y = (geometry.y() + (geometry.height() - 444) / 2
            - (geometry.height() as f64 * 0.08) as i32)
            * scale;
        XMoveWindow(d, xw, x, y);
        let send = |name: &std::ffi::CStr, data: [std::ffi::c_long; 5]| {
            let mut event: ClientEvent = std::mem::zeroed();
            event.type_ = 33;
            event.display = d;
            event.window = xw;
            event.message_type = XInternAtom(d, name.as_ptr(), 0);
            event.format = 32;
            event.data = data;
            XSendEvent(d, root, 0, (1 << 20) | (1 << 19), &mut event);
        };
        for name in [
            c"_NET_WM_STATE_ABOVE",
            c"_NET_WM_STATE_SKIP_TASKBAR",
            c"_NET_WM_STATE_SKIP_PAGER",
        ] {
            send(
                c"_NET_WM_STATE",
                [
                    1,
                    XInternAtom(d, name.as_ptr(), 0) as std::ffi::c_long,
                    0,
                    2,
                    0,
                ],
            );
        }
        send(c"_NET_ACTIVE_WINDOW", [2, 0, 0, 0, 0]);
        XFlush(d);
    }
}
unsafe extern "C" fn ignore_error(_: *mut c_void, _: *mut c_void) -> c_int {
    0
}
pub struct Display(*mut c_void);
impl Default for Display {
    fn default() -> Self {
        let name = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
        let name = CString::new(name).unwrap_or_default();
        unsafe {
            XSetErrorHandler(Some(ignore_error));
            Self(XOpenDisplay(name.as_ptr()))
        }
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                XCloseDisplay(self.0);
            }
        }
    }
}
impl Display {
    pub fn front_app(&self) -> String {
        if self.0.is_null() {
            return String::new();
        }
        unsafe {
            let (mut window, mut revert) = (0, 0);
            XGetInputFocus(self.0, &mut window, &mut revert);
            let root = XDefaultRootWindow(self.0);
            for _ in 0..8 {
                if window == 0 || window == 1 || window == root {
                    break;
                }
                let mut hint = ClassHint {
                    res_name: ptr::null_mut(),
                    res_class: ptr::null_mut(),
                };
                if XGetClassHint(self.0, window, &mut hint) != 0 {
                    let name = if hint.res_class.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(hint.res_class)
                            .to_string_lossy()
                            .into_owned()
                    };
                    if !hint.res_class.is_null() {
                        XFree(hint.res_class.cast());
                    }
                    if !hint.res_name.is_null() {
                        XFree(hint.res_name.cast());
                    }
                    if !name.is_empty() {
                        return name;
                    }
                }
                let (mut root, mut parent, mut children, mut count) = (0, 0, ptr::null_mut(), 0);
                if XQueryTree(
                    self.0,
                    window,
                    &mut root,
                    &mut parent,
                    &mut children,
                    &mut count,
                ) == 0
                {
                    break;
                }
                if !children.is_null() {
                    XFree(children.cast());
                }
                window = parent
            }
        }
        String::new()
    }
}
/// Called at process startup before GLib, GStreamer, or any worker threads are initialized.
pub fn configure_environment() {
    if std::env::var_os("DISPLAY").is_none() {
        unsafe { std::env::set_var("DISPLAY", ":0") }
    }
    if std::env::var_os("XAUTHORITY").is_none() {
        let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".into());
        if let Ok(entries) = std::fs::read_dir(runtime) {
            let mut paths: Vec<_> = entries
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(".mutter-Xwaylandauth.")
                })
                .map(|e| e.path())
                .collect();
            paths.sort();
            if let Some(path) = paths.first() {
                unsafe { std::env::set_var("XAUTHORITY", path) }
            }
        }
    }
}
