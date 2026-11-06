//! Nonactivating AppKit panel. The document draws only the visible emoji rows.
use super::{
    app, host,
    prefs::Defaults,
    ui::{self, Obj},
};
use objc2::{
    MainThreadOnly, class, define_class, msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
};
use objc2_app_kit::{NSPanel, NSView};
use objc2_foundation::{
    MainThreadMarker, NSOperatingSystemVersion, NSPoint, NSRect, NSSize, NSString,
};
use sevi_runtime::{
    emoji::{EmojiIndex, convert_telex, frequent, record_usage},
    prefs::Preferences,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
define_class!(
    #[unsafe(super=NSPanel)]
    #[thread_kind=MainThreadOnly]
    struct EmojiPanel;
    impl EmojiPanel {
        #[unsafe(method(canBecomeKeyWindow))] fn key_window(&self)->bool{true}
        #[unsafe(method(canBecomeMainWindow))] fn main_window(&self)->bool{false}
        #[unsafe(method(resignKeyWindow))] fn resign(&self){unsafe{let _:()=msg_send![super(self),resignKeyWindow];}app::with(|a|a.picker.close());}
        #[unsafe(method(keyDown:))] fn key_down(&self,event:&AnyObject){app::with(|a|a.picker.key(event));}
        #[unsafe(method(action:))] fn action(&self,sender:&AnyObject){let tag:isize=unsafe{msg_send![sender,tag]};app::with(|a|{if (500..=505).contains(&tag){a.picker.set_tone((tag-500)as usize);}});}
        #[unsafe(method(tone:))] fn tone(&self,sender:&AnyObject){let index=ui::selected(sender);app::with(|a|a.picker.set_tone(index));}
    }
);
define_class!(
    #[unsafe(super=NSView)]
    #[thread_kind=MainThreadOnly]
    struct EmojiGrid;
    impl EmojiGrid {
        #[unsafe(method(isFlipped))] fn flipped(&self)->bool{true}
        #[unsafe(method(drawRect:))] fn draw(&self,_:NSRect){app::guarded(||draw_grid(self));}
        #[unsafe(method(mouseDown:))] fn click(&self,event:&AnyObject){app::with(|a|{if let Some(i)=a.picker.hit(self,event){a.picker.choose(i);}});}
        #[unsafe(method(mouseMoved:))] fn moved(&self,event:&AnyObject){app::with(|a|{if let Some(i)=a.picker.hit(self,event)&& i!=a.picker.selected{a.picker.selected=i;a.picker.refresh_selection(false);}});}
    }
);
#[derive(Clone)]
struct Row {
    title: String,
    start: usize,
    count: usize,
    y: f64,
}
#[derive(Default)]
struct GridContent {
    rows: Vec<Row>,
    emoji: Vec<String>,
}
#[derive(Clone)]
struct GridState {
    content: Rc<GridContent>,
    selected: usize,
    vietnamese: bool,
}
thread_local! {static GRID:RefCell<Option<GridState>>=const{RefCell::new(None)};}
fn draw_grid(view: &AnyObject) {
    let state = GRID.with(|v| v.borrow().clone());
    if let Some(state) = state {
        state.draw(view);
    }
}
pub struct Picker {
    index: EmojiIndex,
    all: Vec<usize>,
    visible: Vec<usize>,
    rows: Vec<Row>,
    panel: Option<Retained<EmojiPanel>>,
    grid: Option<Retained<EmojiGrid>>,
    search_label: Option<Obj>,
    name_label: Option<Obj>,
    raw: String,
    selected: usize,
    tone: usize,
    all_selected: bool,
    vietnamese: bool,
    draw_content: Rc<GridContent>,
}
impl Picker {
    pub fn new() -> Self {
        let max = unsafe {
            let p: Obj = msg_send![class!(NSProcessInfo), processInfo];
            let v: NSOperatingSystemVersion = msg_send![&*p, operatingSystemVersion];
            match (v.majorVersion, v.minorVersion) {
                (major, minor) if major > 26 || major == 26 && minor >= 4 => 17.,
                (major, minor) if major > 15 || major == 15 && minor >= 4 => 16.,
                (major, minor) if major > 14 || major == 14 && minor >= 4 => 15.1,
                (major, minor) if major > 13 || major == 13 && minor >= 3 => 15.,
                _ => 14.,
            }
        };
        let index = EmojiIndex::default();
        let all = index
            .all
            .iter()
            .enumerate()
            .filter(|(_, e)| e.version <= max)
            .map(|(i, _)| i)
            .collect();
        Self {
            index,
            all,
            visible: vec![],
            rows: vec![],
            panel: None,
            grid: None,
            search_label: None,
            name_label: None,
            raw: String::new(),
            selected: 0,
            tone: Defaults.int("EmojiSkinTone", 0).clamp(0, 5) as usize,
            all_selected: false,
            vietnamese: true,
            draw_content: Rc::default(),
        }
    }
    pub fn visible(&self) -> bool {
        self.panel.as_ref().is_some_and(|p| ui::visible(p))
    }
    pub fn toggle(&mut self) {
        if self.visible() {
            self.close()
        } else {
            self.open()
        }
    }
    pub fn open(&mut self) {
        self.raw.clear();
        self.all_selected = false;
        self.vietnamese = host::with(|h| h.session.language() == sevi_engine::Language::Vietnamese)
            .unwrap_or(true);
        self.build();
        self.search();
        host::with(|h| h.session.emoji_open = true);
        if let Some(p) = &self.panel {
            ui::center(p, true);
            unsafe {
                let _: () = msg_send![&**p,makeKeyAndOrderFront:std::ptr::null::<AnyObject>()];
                let _: () = msg_send![&**p, invalidateShadow];
            }
        }
    }
    pub fn close(&mut self) {
        host::with(|h| h.session.emoji_open = false);
        if let Some(p) = &self.panel {
            ui::close(p)
        }
    }
    pub fn set_raw(&mut self, s: &str) {
        self.raw = s.into();
        self.all_selected = false;
        self.search();
    }
    pub fn set_tone(&mut self, t: usize) {
        self.tone = t.min(5);
        let _ = Defaults.set_int("EmojiSkinTone", self.tone as i32);
        self.update_content();
        self.refresh_selection(false);
    }
    pub fn choose(&mut self, position: usize) {
        let Some(&i) = self.visible.get(position) else {
            return;
        };
        let emoji = &self.index.all[i];
        let text = emoji.with_skin(self.tone).to_owned();
        let mut scores: BTreeMap<String, f64> = Defaults
            .get("EmojiUsage")
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        record_usage(&mut scores, &emoji.text);
        let _ = Defaults.set("EmojiUsage", serde_json::to_value(scores).unwrap());
        self.close();
        let when = dispatch2::DispatchTime::try_from(std::time::Duration::from_millis(30))
            .expect("bounded delay");
        let _ = dispatch2::DispatchQueue::main().after(when, move || {
            app::guarded(|| {
                host::with(|h| h.insert_text(&text));
            })
        });
    }
    fn build(&mut self) {
        if self.panel.is_some() {
            return;
        }
        unsafe {
            let a = EmojiPanel::alloc(MainThreadMarker::new().unwrap());
            let panel: Retained<EmojiPanel> = msg_send![a,initWithContentRect:ui::rect(0.,0.,420.,404.),styleMask:(1usize<<7)|(1usize<<15),backing:2usize,defer:false];
            let _: () = msg_send![&*panel,setReleasedWhenClosed:false];
            let _: () = msg_send![&*panel,setFloatingPanel:true];
            let _: () = msg_send![&*panel,setLevel:101isize];
            let _: () = msg_send![&*panel,setCollectionBehavior:1usize|8usize|256usize];
            let _: () = msg_send![&*panel,setHidesOnDeactivate:false];
            let _: () = msg_send![&*panel,setMovableByWindowBackground:true];
            let _: () = msg_send![&*panel,setHasShadow:true];
            let _: () = msg_send![&*panel,setAcceptsMouseMovedEvents:true];
            let root = ui::view(420., 404.);
            let effect = ui::object("NSVisualEffectView");
            let _: () = msg_send![&*effect,setMaterial:6isize];
            let _: () = msg_send![&*effect,setState:1isize];
            let _: () = msg_send![&*effect,setBlendingMode:0isize];
            let _: () = msg_send![&*effect,setWantsLayer:true];
            let layer: Obj = msg_send![&*effect, layer];
            let _: () = msg_send![&*layer,setCornerRadius:14.0f64];
            let _: () = msg_send![&*layer,setMasksToBounds:true];
            ui::place(&effect, &root, 0., 0., 420., 404.);
            let clear: Obj = msg_send![class!(NSColor), clearColor];
            let _: () = msg_send![&*panel,setBackgroundColor:&*clear];
            let _: () = msg_send![&*panel,setOpaque:false];
            self.search_label = Some(ui::label(&effect, "", 18., 16., 376., 26., 17.));
            let grid: Retained<EmojiGrid> = msg_send![EmojiGrid::alloc(MainThreadMarker::new().unwrap()),initWithFrame:ui::rect(0.,0.,396.,100.)];
            let area: Allocated<AnyObject> = msg_send![class!(NSTrackingArea), alloc];
            let area: Obj = msg_send![area,initWithRect:ui::rect(0.,0.,0.,0.),options:2usize|32usize|512usize,owner:&*grid,userInfo:std::ptr::null::<AnyObject>()];
            let _: () = msg_send![&*grid,addTrackingArea:&*area];
            ui::scroll(&effect, &grid, ui::rect(12., 58., 396., 294.));
            self.grid = Some(grid);
            self.name_label = Some(ui::label(&effect, "", 14., 369., 248., 24., 12.));
            let icon: Obj = msg_send![&*app::application(), applicationIconImage];
            let view = ui::object("NSImageView");
            let _: () = msg_send![&*view,setImage:&*icon];
            ui::place(&view, &effect, 270., 369., 16., 16.);
            ui::label(&effect, "Sevi", 290., 369., 60., 24., 13.);
            let tones = ui::popup(
                &effect,
                &["✋", "✋🏻", "✋🏼", "✋🏽", "✋🏾", "✋🏿"],
                self.tone,
                500,
                &panel,
                ui::rect(358., 362., 49., 30.),
            );
            // A menu action carries the selected tone, rather than the popup's fixed tag.
            let _: () = msg_send![&*tones,setAction:objc2::sel!(tone:)];
            ui::content(&panel, &root);
            self.panel = Some(panel);
        }
    }
    pub fn search(&mut self) {
        self.vietnamese = host::with(|h| h.session.language() == sevi_engine::Language::Vietnamese)
            .unwrap_or(true);
        self.visible.clear();
        self.rows.clear();
        self.selected = 0;
        let mut y = 0.;
        if !self.raw.trim().is_empty() {
            self.visible = self.index.search(&self.raw, 400, Some(&self.all));
            self.add_rows("", self.visible.len(), &mut y);
        } else {
            let scores: BTreeMap<String, f64> = Defaults
                .get("EmojiUsage")
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            for text in frequent(&scores, 27) {
                if let Some(&i) = self.all.iter().find(|&&i| self.index.all[i].text == text) {
                    self.visible.push(i);
                }
            }
            self.add_rows(
                if self.vietnamese {
                    "Dùng thường xuyên"
                } else {
                    "Frequently Used"
                },
                self.visible.len(),
                &mut y,
            );
            for group in 0..10 {
                let list: Vec<_> = self
                    .all
                    .iter()
                    .copied()
                    .filter(|&i| self.index.all[i].group == group)
                    .collect();
                if list.is_empty() {
                    continue;
                }
                let start = self.visible.len();
                self.visible.extend(list);
                let names = if self.vietnamese {
                    &GROUP_VI
                } else {
                    &GROUP_EN
                };
                self.add_rows_from(names[group], start, self.visible.len() - start, &mut y);
            }
        }
        self.update_content();
        if let Some(g) = &self.grid {
            unsafe {
                let _: () = msg_send![&**g,setFrameSize:NSSize::new(396.,y.max(294.))];
                let _: () = msg_send![&**g,scrollPoint:NSPoint::new(0.,0.)];
            }
        }
        self.refresh_selection(false);
    }
    fn add_rows(&mut self, title: &str, count: usize, y: &mut f64) {
        self.add_rows_from(title, 0, count, y)
    }
    fn add_rows_from(&mut self, title: &str, start: usize, count: usize, y: &mut f64) {
        if !title.is_empty() {
            self.rows.push(Row {
                title: title.into(),
                start,
                count: 0,
                y: *y,
            });
            *y += 26.;
        }
        for i in (0..count).step_by(9) {
            self.rows.push(Row {
                title: String::new(),
                start: start + i,
                count: (count - i).min(9),
                y: *y,
            });
            *y += 44.;
        }
    }
    fn display(&self) -> String {
        if self.vietnamese {
            convert_telex(&self.raw)
        } else {
            self.raw.clone()
        }
    }
    pub(super) fn offscreen(&mut self) -> Obj {
        self.raw.clear();
        self.all_selected = false;
        self.vietnamese = true;
        self.build();
        self.search();
        unsafe {
            let v: Obj = msg_send![&**self.panel.as_ref().unwrap(), contentView];
            v
        }
    }
    pub(super) fn debug_summary(&self) -> String {
        let current = self.visible.get(self.selected).map(|&i| &self.index.all[i]);
        let first = self
            .visible
            .iter()
            .take(12)
            .map(|&i| self.index.all[i].text.as_str())
            .collect::<String>();
        format!(
            "query '{}' shows '{}' selected {} = {} {} | first: {}",
            self.raw,
            self.display(),
            self.selected,
            current.map(|e| e.text.as_str()).unwrap_or("-"),
            current.map(|e| e.name_en.as_str()).unwrap_or(""),
            first
        )
    }
    fn refresh_selection(&self, scroll: bool) {
        GRID.with(|v| {
            *v.borrow_mut() = Some(GridState {
                content: self.draw_content.clone(),
                selected: self.selected,
                vietnamese: self.vietnamese,
            })
        });
        if let Some(label) = &self.search_label {
            let query = self.display();
            ui::set_text(
                label,
                &format!(
                    "⌕  {}    {}",
                    if query.is_empty() {
                        if self.vietnamese {
                            "Tìm emoji: ngựa, horse…"
                        } else {
                            "Search emoji: horse, ngựa…"
                        }
                    } else {
                        &query
                    },
                    if self.vietnamese { "VI" } else { "EN" }
                ),
            );
        }
        if let Some(label) = &self.name_label {
            let text = self
                .visible
                .get(self.selected)
                .map(|&i| {
                    let e = &self.index.all[i];
                    format!(
                        "{}  {}",
                        e.with_skin(self.tone),
                        if self.vietnamese {
                            &e.name_vi
                        } else {
                            &e.name_en
                        }
                    )
                })
                .unwrap_or_default();
            ui::set_text(label, &text);
        }
        if let Some(grid) = &self.grid {
            unsafe {
                let _: () = msg_send![&**grid,setNeedsDisplay:true];
                if scroll
                    && let Some(row) = self
                        .rows
                        .iter()
                        .find(|r| self.selected >= r.start && self.selected < r.start + r.count)
                {
                    let _: bool = msg_send![&**grid,scrollRectToVisible:ui::rect(0.,if self.selected==0{0.}else{row.y},396.,44.)];
                }
            }
        }
    }
    fn move_by(&mut self, d: isize, row: bool) {
        self.all_selected = false;
        if self.visible.is_empty() {
            return;
        }
        if row {
            if let Some(index) = self
                .rows
                .iter()
                .position(|r| self.selected >= r.start && self.selected < r.start + r.count)
            {
                let col = self.selected - self.rows[index].start;
                let mut target = index as isize + d;
                while target >= 0 && (target as usize) < self.rows.len() {
                    let r = &self.rows[target as usize];
                    if r.count > 0 {
                        self.selected = r.start + col.min(r.count - 1);
                        break;
                    }
                    target += d;
                }
            }
        } else {
            self.selected =
                (self.selected as isize + d).clamp(0, self.visible.len() as isize - 1) as usize;
        }
        self.refresh_selection(true);
    }
    pub fn key(&mut self, event: &AnyObject) {
        unsafe {
            let code: u16 = msg_send![event, keyCode];
            let flags: u64 = msg_send![event, modifierFlags];
            let cmd = flags & (1 << 20) != 0;
            let alt = flags & (1 << 19) != 0;
            match code {
                53 => {
                    if self.raw.is_empty() {
                        self.close()
                    } else {
                        self.set_raw("")
                    }
                }
                36 | 76 => self.choose(self.selected),
                123 => self.move_by(-1, false),
                124 => self.move_by(1, false),
                125 => self.move_by(1, true),
                126 => self.move_by(-1, true),
                51 => {
                    if cmd || self.all_selected {
                        self.set_raw("")
                    } else if alt {
                        while self.raw.ends_with(' ') {
                            self.raw.pop();
                        }
                        while !self.raw.is_empty() && !self.raw.ends_with(' ') {
                            self.raw.pop();
                        }
                        self.search();
                    } else {
                        crate::policy::delete_grapheme(&mut self.raw);
                        self.search();
                    }
                }
                _ => {
                    let chars = super::prefs::text(msg_send![event, charactersIgnoringModifiers]);
                    if cmd {
                        match chars.as_str() {
                            "a" => self.all_selected = !self.raw.is_empty(),
                            "w" => self.close(),
                            "v" => {
                                let p: Obj = msg_send![class!(NSPasteboard), generalPasteboard];
                                let text = super::prefs::text(
                                    msg_send![&*p,stringForType:&*NSString::from_str("public.utf8-plain-text")],
                                );
                                self.type_text(&text)
                            }
                            "c" | "x" if self.all_selected && !self.raw.is_empty() => {
                                let p: Obj = msg_send![class!(NSPasteboard), generalPasteboard];
                                let _: isize = msg_send![&*p, clearContents];
                                let _: bool = msg_send![&*p,setString:&*NSString::from_str(&self.display()),forType:&*NSString::from_str("public.utf8-plain-text")];
                                if chars == "x" {
                                    self.set_raw("")
                                }
                            }
                            _ => (),
                        }
                    } else {
                        let chars = super::prefs::text(msg_send![event, characters]);
                        if !chars.chars().any(char::is_control) {
                            self.type_text(&chars)
                        }
                    }
                }
            }
        }
    }
    fn type_text(&mut self, s: &str) {
        if self.all_selected {
            self.raw.clear()
        }
        self.all_selected = false;
        self.raw.push_str(s);
        self.search();
    }
    fn hit(&mut self, grid: &AnyObject, event: &AnyObject) -> Option<usize> {
        unsafe {
            let p: NSPoint = msg_send![event, locationInWindow];
            let p: NSPoint = msg_send![grid,convertPoint:p,fromView:std::ptr::null::<AnyObject>()];
            let col = (p.x / 44.).floor() as usize;
            if col >= 9 {
                return None;
            }
            self.rows
                .iter()
                .find(|r| r.count > 0 && p.y >= r.y && p.y < r.y + 44. && col < r.count)
                .map(|r| r.start + col)
        }
    }
    fn update_content(&mut self) {
        self.draw_content = Rc::new(GridContent {
            rows: self.rows.clone(),
            emoji: self
                .visible
                .iter()
                .map(|&i| self.index.all[i].with_skin(self.tone).to_owned())
                .collect(),
        });
    }
}
impl GridState {
    fn draw(&self, grid: &AnyObject) {
        unsafe {
            let visible: NSRect = msg_send![grid, visibleRect];
            let min = visible.origin.y;
            let max = min + visible.size.height;
            for row in self
                .content
                .rows
                .iter()
                .filter(|r| r.y + 44. >= min && r.y <= max)
            {
                if row.count == 0 {
                    draw_text(&row.title, ui::rect(2., row.y + 3., 390., 21.), 11., true);
                    continue;
                }
                for col in 0..row.count {
                    let i = row.start + col;
                    let e = &self.content.emoji[i];
                    let x = col as f64 * 44.;
                    if i == self.selected {
                        let color: Obj = msg_send![class!(NSColor), controlAccentColor];
                        let color: Obj = msg_send![&*color,colorWithAlphaComponent:0.22f64];
                        let _: () = msg_send![&*color, setFill];
                        let path: Obj = msg_send![class!(NSBezierPath),bezierPathWithRoundedRect:ui::rect(x+1.,row.y+1.,42.,42.),xRadius:8.0f64,yRadius:8.0f64];
                        let _: () = msg_send![&*path, fill];
                    }
                    draw_text(e, ui::rect(x, row.y + 4., 44., 36.), 28., false);
                }
            }
            if self.content.emoji.is_empty() {
                draw_text(
                    if self.vietnamese {
                        "Không tìm thấy emoji nào"
                    } else {
                        "No emoji found"
                    },
                    ui::rect(25., 110., 350., 30.),
                    13.,
                    true,
                );
            }
        }
    }
}
fn draw_text(s: &str, rect: NSRect, size: f64, left: bool) {
    unsafe {
        let font: Obj = msg_send![class!(NSFont),systemFontOfSize:size];
        let color: Obj = msg_send![class!(NSColor), labelColor];
        let attrs: Obj = msg_send![class!(NSMutableDictionary), dictionary];
        for (k, v) in [("NSFont", &*font), ("NSColor", &*color)] {
            let _: () = msg_send![&*attrs,setObject:v,forKey:&*NSString::from_str(k)];
        }
        let text = NSString::from_str(s);
        if left {
            let _: () = msg_send![&*text,drawInRect:rect,withAttributes:&*attrs];
        } else {
            let size: NSSize = msg_send![&*text,sizeWithAttributes:&*attrs];
            let _: () = msg_send![&*text,drawAtPoint:NSPoint::new(rect.origin.x+(rect.size.width-size.width)/2.,rect.origin.y),withAttributes:&*attrs];
        }
    }
}
const GROUP_VI: [&str; 10] = [
    "Mặt cười & cảm xúc",
    "Người & cơ thể",
    "",
    "Động vật & thiên nhiên",
    "Đồ ăn & thức uống",
    "Du lịch & địa điểm",
    "Hoạt động",
    "Đồ vật",
    "Biểu tượng",
    "Cờ",
];
const GROUP_EN: [&str; 10] = [
    "Smileys & Emotion",
    "People & Body",
    "",
    "Animals & Nature",
    "Food & Drink",
    "Travel & Places",
    "Activities",
    "Objects",
    "Symbols",
    "Flags",
];
