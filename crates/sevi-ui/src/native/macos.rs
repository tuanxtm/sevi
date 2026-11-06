//! Keep the picker in a real nonactivating NSPanel. Its widgets are the same egui
//! widgets as on other platforms; AppKit supplies only focus, input and a surface.
use objc2::{
    MainThreadOnly, class, define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
};
use objc2_app_kit::{NSPanel, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
use sevi_ui_protocol::PickerInput;
use std::{
    cell::RefCell,
    ptr::NonNull,
    sync::{Arc, Mutex, mpsc},
    time::Instant,
};
type Obj = Retained<AnyObject>;
struct Input {
    keys: mpsc::Sender<PickerInput>,
    events: Mutex<Vec<egui::Event>>,
    wake: egui::Context,
}
thread_local! {static INPUT:RefCell<Option<Arc<Input>>>=const{RefCell::new(None)};static PANEL:RefCell<Option<Panel>>=const{RefCell::new(None)};}
fn input(f: impl FnOnce(&Input)) {
    INPUT.with(|i| {
        if let Some(i) = i.borrow().as_ref() {
            f(i);
            i.wake.request_repaint();
        }
    });
}
fn key(event: &AnyObject) {
    unsafe {
        let code: u16 = msg_send![event, keyCode];
        let flags: u64 = msg_send![event, modifierFlags];
        let command = flags & (1 << 20) != 0;
        let option = flags & (1 << 19) != 0;
        let text: Option<Retained<NSString>> = msg_send![event, characters];
        let text = text.map(|s| s.to_string()).unwrap_or_default();
        let e = match code {
            53 => PickerInput::Escape,
            36 | 76 => PickerInput::Enter,
            51 => PickerInput::Backspace {
                word: option,
                all: command,
            },
            123 => PickerInput::Move { dx: -1, dy: 0 },
            124 => PickerInput::Move { dx: 1, dy: 0 },
            125 => PickerInput::Move { dx: 0, dy: 1 },
            126 => PickerInput::Move { dx: 0, dy: -1 },
            0 if command => PickerInput::SelectAll,
            9 if command => PickerInput::Paste,
            _ if !command && !text.is_empty() && !text.chars().any(char::is_control) => {
                PickerInput::Text(text)
            }
            _ => return,
        };
        input(|i| {
            let _ = i.keys.send(e);
        });
    }
}
fn pointer(view: &AnyObject, event: &AnyObject, pressed: Option<bool>) {
    unsafe {
        let location: NSPoint = msg_send![event, locationInWindow];
        let point: NSPoint =
            msg_send![view,convertPoint:location,fromView:std::ptr::null::<AnyObject>()];
        input(|i| {
            let mut events = i.events.lock().unwrap();
            let pos = egui::pos2(point.x as f32, point.y as f32);
            events.push(egui::Event::PointerMoved(pos));
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                });
            }
        });
    }
}
define_class!(
    #[unsafe(super=NSPanel)] #[thread_kind=MainThreadOnly] struct PickerPanel;
    impl PickerPanel {
        #[unsafe(method(canBecomeKeyWindow))] fn key_window(&self)->bool{true}
        #[unsafe(method(canBecomeMainWindow))] fn main_window(&self)->bool{false}
        #[unsafe(method(keyDown:))] fn key_down(&self,e:&AnyObject){key(e);}
        #[unsafe(method(resignKeyWindow))] fn resign(&self){unsafe{let _:()=msg_send![super(self),resignKeyWindow];}input(|i|{let _=i.keys.send(PickerInput::Escape);let _=i.keys.send(PickerInput::Escape);});}
    }
);
define_class!(
    #[unsafe(super=NSView)] #[thread_kind=MainThreadOnly] struct PickerView;
    impl PickerView {
        #[unsafe(method(isFlipped))] fn flipped(&self)->bool{true}
        #[unsafe(method(acceptsFirstResponder))] fn accepts(&self)->bool{true}
        #[unsafe(method(keyDown:))] fn key_down(&self,e:&AnyObject){key(e);}
        #[unsafe(method(mouseDown:))] fn down(&self,e:&AnyObject){pointer(self,e,Some(true));}
        #[unsafe(method(mouseUp:))] fn up(&self,e:&AnyObject){pointer(self,e,Some(false));}
        #[unsafe(method(mouseMoved:))] fn moved(&self,e:&AnyObject){pointer(self,e,None);}
        #[unsafe(method(mouseDragged:))] fn dragged(&self,e:&AnyObject){pointer(self,e,None);}
        #[unsafe(method(scrollWheel:))] fn scroll(&self,e:&AnyObject){unsafe{let x:f64=msg_send![e,scrollingDeltaX];let y:f64=msg_send![e,scrollingDeltaY];input(|i|i.events.lock().unwrap().push(egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(x as f32,y as f32),modifiers:egui::Modifiers::default(),phase:egui::TouchPhase::Move,source:egui::MouseWheelSource::Unknown}));}}
    }
);
struct Panel {
    // Drop the GPU surface before releasing its NSView/NSPanel.
    surface: wgpu::Surface<'static>,
    renderer: egui_wgpu::Renderer,
    config: wgpu::SurfaceConfiguration,
    panel: Retained<PickerPanel>,
    _view: Retained<PickerView>,
    ctx: egui::Context,
    started: Instant,
}
impl Drop for Panel {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![&*self.panel,orderOut:std::ptr::null::<AnyObject>()];
        }
    }
}
pub(super) fn configure(
    _frame: &eframe::Frame,
    picker: bool,
    ctx: &egui::Context,
    events: mpsc::Sender<PickerInput>,
) -> u64 {
    close_picker();
    INPUT.with(|i| {
        *i.borrow_mut() = picker.then(|| {
            Arc::new(Input {
                keys: events,
                events: Mutex::new(vec![]),
                wake: ctx.clone(),
            })
        })
    });
    if !picker {
        unsafe {
            let app: Obj = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![&*app,activateIgnoringOtherApps:true];
        }
    }
    0
}
pub(super) fn close_picker() {
    INPUT.with(|input| {
        input.borrow_mut().take();
    });
    PANEL.with(|panel| {
        panel.borrow_mut().take();
    });
}
fn create(frame: &eframe::Frame) -> Option<Panel> {
    unsafe {
        let state = frame.wgpu_render_state()?;
        let marker = MainThreadMarker::new()?;
        let rect = NSRect::new(NSPoint::new(0., 0.), NSSize::new(430., 430.));
        let panel: Retained<PickerPanel> = msg_send![PickerPanel::alloc(marker),initWithContentRect:rect,styleMask:(1usize<<7)|(1usize<<15),backing:2usize,defer:false];
        let _: () = msg_send![&*panel,setReleasedWhenClosed:false];
        let _: () = msg_send![&*panel,setFloatingPanel:true];
        let _: () = msg_send![&*panel,setLevel:101isize];
        let _: () = msg_send![&*panel,setHidesOnDeactivate:false];
        let _: () = msg_send![&*panel,setCollectionBehavior:1usize|8usize|256usize];
        let _: () = msg_send![&*panel,setAcceptsMouseMovedEvents:true];
        let view: Retained<PickerView> = msg_send![PickerView::alloc(marker),initWithFrame:rect];
        let _: () = msg_send![&*panel,setContentView:&*view];
        let _: bool = msg_send![&*panel,makeFirstResponder:&*view];
        let area: Allocated<AnyObject> = msg_send![class!(NSTrackingArea), alloc];
        let area: Obj = msg_send![area,initWithRect:rect,options:2usize|32usize|512usize,owner:&*view,userInfo:std::ptr::null::<AnyObject>()];
        let _: () = msg_send![&*view,addTrackingArea:&*area];
        let screen: Option<Obj> = msg_send![class!(NSScreen), mainScreen];
        if let Some(screen) = screen {
            let work: NSRect = msg_send![&*screen, visibleFrame];
            let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
            let x = (mouse.x - 215.).clamp(
                work.origin.x,
                (work.origin.x + work.size.width - 430.).max(work.origin.x),
            );
            let y = (mouse.y - 430.).clamp(
                work.origin.y,
                (work.origin.y + work.size.height - 430.).max(work.origin.y),
            );
            let _: () = msg_send![&*panel,setFrameOrigin:NSPoint::new(x,y)];
        }
        let _: () = msg_send![&*panel,makeKeyAndOrderFront:std::ptr::null::<AnyObject>()];
        let handle = raw_window_handle::AppKitWindowHandle::new(NonNull::from(&*view).cast());
        let surface = state
            .instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_window_handle::AppKitDisplayHandle::new().into()),
                raw_window_handle: handle.into(),
            })
            .ok()?;
        let scale: f64 = msg_send![&*panel, backingScaleFactor];
        let side = (430. * scale).round() as u32;
        let config = surface.get_default_config(&state.adapter, side, side)?;
        surface.configure(&state.device, &config);
        let renderer = egui_wgpu::Renderer::new(&state.device, config.format, Default::default());
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        Some(Panel {
            surface,
            renderer,
            config,
            panel,
            _view: view,
            ctx,
            started: Instant::now(),
        })
    }
}
pub(crate) fn paint(
    frame: &eframe::Frame,
    root: &egui::Context,
    mut draw: impl FnMut(&mut egui::Ui),
) {
    PANEL.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = create(frame);
        }
        let Some(p) = slot.as_mut() else {
            return;
        };
        let Some(state) = frame.wgpu_render_state() else {
            return;
        };
        p.ctx.set_theme(root.theme());
        let scale: f64 = unsafe { msg_send![&*p.panel, backingScaleFactor] };
        let scale = scale as f32;
        let side = (430. * scale).round() as u32;
        if p.config.width != side {
            p.config.width = side;
            p.config.height = side;
            p.surface.configure(&state.device, &p.config);
        }
        let events = INPUT.with(|i| {
            i.borrow()
                .as_ref()
                .map(|i| std::mem::take(&mut *i.events.lock().unwrap()))
                .unwrap_or_default()
        });
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(430., 430.),
            )),
            time: Some(p.started.elapsed().as_secs_f64()),
            events,
            focused: true,
            ..Default::default()
        };
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(scale);
        let mut output = p.ctx.run_ui(raw, |ui| draw(ui));
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                p.renderer
                    .update_texture(&state.device, &state.queue, *id, delta);
            }
        }
        let jobs = p.ctx.tessellate(output.shapes, output.pixels_per_point);
        let descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [p.config.width, p.config.height],
            pixels_per_point: output.pixels_per_point,
        };
        let mut encoder = state.device.create_command_encoder(&Default::default());
        let buffers = p.renderer.update_buffers(
            &state.device,
            &state.queue,
            &mut encoder,
            &jobs,
            &descriptor,
        );
        if let wgpu::CurrentSurfaceTexture::Success(texture)
        | wgpu::CurrentSurfaceTexture::Suboptimal(texture) = p.surface.get_current_texture()
        {
            let view = texture.texture.create_view(&Default::default());
            {
                let mut pass = encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Sevi emoji panel"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    })
                    .forget_lifetime();
                p.renderer.render(&mut pass, &jobs, &descriptor);
            }
            state
                .queue
                .submit(buffers.into_iter().chain(std::iter::once(encoder.finish())));
            state.queue.present(texture);
        } else {
            p.surface.configure(&state.device, &p.config);
            root.request_repaint();
        }
        for id in &output.textures_delta.free {
            p.renderer.free_texture(id);
        }
        output.textures_delta.clear();
        let delay = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|o| o.repaint_delay)
            .unwrap_or(std::time::Duration::MAX);
        if delay < std::time::Duration::MAX {
            root.request_repaint_after(delay);
        }
    });
}
pub(super) fn clipboard() -> Option<String> {
    unsafe {
        let board: Obj = msg_send![class!(NSPasteboard), generalPasteboard];
        let text: Option<Retained<NSString>> =
            msg_send![&*board,stringForType:&*NSString::from_str("public.utf8-plain-text")];
        text.map(|s| s.to_string())
    }
}
pub(super) fn filter(index: &mut sevi_runtime::emoji::EmojiIndex) {
    unsafe {
        let process: Obj = msg_send![class!(NSProcessInfo), processInfo];
        let version: objc2_foundation::NSOperatingSystemVersion =
            msg_send![&*process, operatingSystemVersion];
        let max = match (version.majorVersion, version.minorVersion) {
            (major, minor) if major > 26 || major == 26 && minor >= 4 => 17.,
            (major, minor) if major > 15 || major == 15 && minor >= 4 => 16.,
            (major, minor) if major > 14 || major == 14 && minor >= 4 => 15.1,
            (major, minor) if major > 13 || major == 13 && minor >= 3 => 15.,
            _ => 14.,
        };
        index.all.retain(|emoji| emoji.version <= max);
    }
}
pub(super) fn glyph(text: &str, pixels: u32) -> Option<egui::ColorImage> {
    unsafe {
        let a: Allocated<AnyObject> = msg_send![class!(NSBitmapImageRep), alloc];
        let rep: Option<Obj> = msg_send![a,initWithBitmapDataPlanes:std::ptr::null_mut::<*mut u8>(),pixelsWide:pixels as isize,pixelsHigh:pixels as isize,bitsPerSample:8isize,samplesPerPixel:4isize,hasAlpha:true,isPlanar:false,colorSpaceName:&*NSString::from_str("NSDeviceRGBColorSpace"),bitmapFormat:0usize,bytesPerRow:(pixels*4)as isize,bitsPerPixel:32isize];
        let rep = rep?;
        let pixels_ptr: *mut u8 = msg_send![&*rep, bitmapData];
        if pixels_ptr.is_null() {
            return None;
        }
        std::ptr::write_bytes(pixels_ptr, 0, (pixels * pixels * 4) as usize);
        let context: Obj =
            msg_send![class!(NSGraphicsContext),graphicsContextWithBitmapImageRep:&*rep];
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let _: () = msg_send![class!(NSGraphicsContext),setCurrentContext:&*context];
        let font: Option<Obj> = msg_send![class!(NSFont),fontWithName:&*NSString::from_str("Apple Color Emoji"),size:pixels as f64*0.78];
        let attrs: Obj = msg_send![class!(NSMutableDictionary), dictionary];
        if let Some(font) = font {
            let _: () = msg_send![&*attrs,setObject:&*font,forKey:&*NSString::from_str("NSFont")];
        }
        let text = NSString::from_str(text);
        let size: NSSize = msg_send![&*text,sizeWithAttributes:&*attrs];
        let _: () = msg_send![&*text,drawAtPoint:NSPoint::new((pixels as f64-size.width)/2.,(pixels as f64-size.height)/2.),withAttributes:&*attrs];
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        let data: *const u8 = msg_send![&*rep, bitmapData];
        if data.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(data, (pixels * pixels * 4) as usize);
        Some(egui::ColorImage::from_rgba_premultiplied(
            [pixels as usize, pixels as usize],
            bytes,
        ))
    }
}
