//! Transitional C interface for the existing native hosts. Every context has one owner thread.
use crate::{keys::*, smart_switch::SmartSwitch, *};
use std::{
    ffi::{CStr, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Config {
    pub language: i32,
    pub spelling: i32,
    pub modern: i32,
    pub restore: i32,
    pub auto_switch: i32,
    pub teencode: i32,
    pub style: i32,
    pub previous_restored: i32,
    pub request: i32,
    pub last_side: i32,
}
impl Config {
    fn engine(self) -> EngineConfig {
        EngineConfig {
            language: if self.language == 0 {
                Language::English
            } else {
                Language::Vietnamese
            },
            spelling: self.spelling != 0,
            modern_orthography: self.modern != 0,
            restore_if_invalid: self.restore != 0,
            auto_switch: self.auto_switch != 0,
            teencode: self.teencode != 0,
            telex_style: match self.style {
                0 => TelexStyle::Telex,
                1 => TelexStyle::Simple1,
                _ => TelexStyle::Simple2,
            },
        }
    }
}
#[repr(C)]
#[derive(Default)]
pub struct Hook {
    pub code: u8,
    pub backspaces: u8,
    pub count: u8,
    pub ext: u8,
    pub chars: [u32; MAX_BUFF],
}
#[repr(C)]
#[derive(Default)]
pub struct Output {
    pub hook: Hook,
    pub request: i32,
    pub last_side: i32,
    pub language: i32,
    pub spelling: i32,
    pub undo_count: i32,
    pub undo: [u32; MAX_BUFF],
    pub restored_count: i32,
    pub restored: [u32; MAX_BUFF],
}
pub struct Context {
    engine: Engine,
    smart: SmartSwitch,
    output: Output,
}
/// # Safety
/// Context must be valid. The returned snapshot must be freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_snapshot(context: *const Context) -> *mut Engine {
    guarded(ptr::null_mut(), || {
        unsafe { context.as_ref() }
            .map(|c| Box::into_raw(Box::new(c.engine.clone())))
            .unwrap_or(ptr::null_mut())
    })
}
/// # Safety
/// Snapshot must come from vk_snapshot. A restore consumes it; null context only frees it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_restore(context: *mut Context, snapshot: *mut Engine) {
    if snapshot.is_null() {
        return;
    }
    guarded((), || {
        let snapshot = unsafe { Box::from_raw(snapshot) };
        if let Some(c) = unsafe { context.as_mut() } {
            c.engine = *snapshot
        }
    })
}
/// # Safety
/// Context and optional NUL-terminated UTF-8 path must be valid. kind: 0 pairs, 1 triples, 2 capitals.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_load_counts(
    context: *mut Context,
    kind: i32,
    path: *const c_char,
    weight: f32,
    min: i32,
) {
    guarded((), || {
        let Some(c) = (unsafe { context.as_mut() }) else {
            return;
        };
        let path = if path.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(path) }
                .to_str()
                .ok()
                .map(std::path::Path::new)
        };
        match kind {
            0 => c.engine.load_bigrams(path, weight, min),
            1 => c.engine.load_trigrams(path, weight, min),
            2 => c.engine.load_capitals(path, min),
            _ => (),
        }
    })
}
impl Context {
    fn output(&mut self) -> *const Output {
        let e = &self.engine;
        self.output = Output {
            hook: Hook {
                code: e.hook.code as u8,
                backspaces: e.hook.backspaces as u8,
                count: e.hook.new_chars as u8,
                ext: e.hook.ext_code,
                chars: e.hook.chars,
            },
            request: e.switch_request.map(|v| v as i32).unwrap_or(-1),
            last_side: e.last_side,
            language: e.config.language as i32,
            spelling: e.config.spelling as i32,
            undo_count: e.undo_data.len() as i32,
            restored_count: e.restored_keys.len() as i32,
            ..Default::default()
        };
        let n = e.undo_data.len().min(MAX_BUFF);
        self.output.undo[..n].copy_from_slice(&e.undo_data[..n]);
        let n = e.restored_keys.len().min(MAX_BUFF);
        self.output.restored[..n].copy_from_slice(&e.restored_keys[..n]);
        &self.output
    }
}
fn guarded<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}
/// # Safety
/// `config` must point to an initialized Config for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_create(config: *const Config) -> *mut Context {
    guarded(ptr::null_mut(), || {
        let Some(config) = (unsafe { config.as_ref() }) else {
            return ptr::null_mut();
        };
        Box::into_raw(Box::new(Context {
            engine: Engine::new(config.engine()),
            smart: SmartSwitch::default(),
            output: Output::default(),
        }))
    })
}
/// # Safety
/// The live context must have been returned by vk_create, with no outstanding users.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_destroy(context: *mut Context) {
    if !context.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(context) })));
    }
}
/// # Safety
/// Both pointers must be valid, and context must be exclusively owned for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_configure(context: *mut Context, config: *const Config) {
    guarded((), || {
        if let (Some(c), Some(config)) = (unsafe { context.as_mut() }, unsafe { config.as_ref() }) {
            c.engine.config = config.engine();
            c.engine.previous_restored = config.previous_restored != 0;
            c.engine.switch_request = match config.request {
                0 => Some(Language::English),
                1 => Some(Language::Vietnamese),
                _ => None,
            };
            c.engine.last_side = config.last_side;
        }
    })
}
/// # Safety
/// Context must be live and exclusively borrowed; returned storage lasts until the next call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_output(context: *mut Context) -> *const Output {
    guarded(ptr::null(), || {
        unsafe { context.as_mut() }
            .map(Context::output)
            .unwrap_or(ptr::null())
    })
}
/// # Safety
/// Context must be live and exclusively borrowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_handle(
    context: *mut Context,
    event: i32,
    state: i32,
    key: u32,
    caps: u8,
    control: bool,
) {
    guarded((), || {
        if let Some(c) = unsafe { context.as_mut() } {
            c.engine.handle_event(
                if event == 0 {
                    Event::Keyboard
                } else {
                    Event::Mouse
                },
                match state {
                    0 => EventState::KeyDown,
                    1 => EventState::KeyUp,
                    2 => EventState::MouseDown,
                    _ => EventState::MouseUp,
                },
                key,
                caps,
                control,
            );
        }
    })
}
/// # Safety
/// Context must be live and exclusively borrowed. Operation: 0 new word, 1 forget, 2 spelling baseline, 3 init.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_reset(context: *mut Context, operation: i32) {
    guarded((), || {
        if let Some(c) = unsafe { context.as_mut() } {
            match operation {
                0 => c.engine.new_session(),
                1 => c.engine.forget(),
                2 => c.engine.set_check_spelling(),
                3 => {
                    c.engine.new_session();
                    c.engine.set_check_spelling();
                }
                _ => (),
            }
        }
    })
}
unsafe fn keys<'a>(ptr: *const u32, len: i32) -> Option<&'a [u32]> {
    if !(0..=MAX_BUFF as i32).contains(&len) {
        return None;
    }
    if len == 0 {
        return Some(&[]);
    }
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(ptr, len as usize) })
}
/// # Safety
/// Arrays must have the indicated lengths. The optional previous array holds up to two valid slices.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_check_word(
    context: *mut Context,
    raw: *const u32,
    count: i32,
    previous: *const *const u32,
    counts: *const i32,
    n: i32,
) -> bool {
    guarded(false, || {
        let (Some(c), Some(raw)) = (unsafe { context.as_mut() }, unsafe { keys(raw, count) })
        else {
            return false;
        };
        if !(0..=2).contains(&n) {
            return false;
        }
        let mut prev = vec![];
        if n > 0 {
            if previous.is_null() || counts.is_null() {
                return false;
            }
            for i in 0..n as usize {
                let Some(k) = (unsafe { keys(*previous.add(i), *counts.add(i)) }) else {
                    return false;
                };
                prev.push(k.to_vec())
            }
        }
        c.engine.check_english_word(raw, &prev)
    })
}
/// # Safety
/// Context and keys must be live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_fix_double(context: *mut Context, raw: *const u32, count: i32) -> bool {
    guarded(false, || {
        match (unsafe { context.as_mut() }, unsafe { keys(raw, count) }) {
            (Some(c), Some(k)) => c.engine.fix_doubled_key(k),
            _ => false,
        }
    })
}
/// # Safety
/// Context and keys must be live for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_word_side(context: *mut Context, raw: *const u32, count: i32) -> i32 {
    guarded(-1, || {
        match (unsafe { context.as_ref() }, unsafe { keys(raw, count) }) {
            (Some(c), Some(k)) => c.engine.word_side(k),
            _ => -1,
        }
    })
}
/// # Safety
/// `word` must be a valid NUL-terminated UTF-8 string. Output, when nonnull, has capacity MAX_BUFF.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_word_score(
    context: *const Context,
    vi: i32,
    word: *const c_char,
    out: *mut u16,
    len: *mut i32,
) -> i32 {
    guarded(-1000, || {
        if !len.is_null() {
            unsafe { *len = 0 }
        }
        let Some(c) = (unsafe { context.as_ref() }) else {
            return -1000;
        };
        if word.is_null() {
            return -1000;
        }
        let Ok(word) = (unsafe { CStr::from_ptr(word) }).to_str() else {
            return -1000;
        };
        let (score, text) = c.engine.word_score(vi != 0, word);
        if !out.is_null() && !len.is_null() {
            let n = text.len().min(MAX_BUFF);
            unsafe {
                ptr::copy_nonoverlapping(text.as_ptr(), out, n);
                *len = n as i32
            }
        }
        score
    })
}
/// # Safety
/// Context and NUL-terminated word must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_is_spelling(context: *const Context, word: *const c_char) -> bool {
    guarded(false, || {
        let Some(c) = (unsafe { context.as_ref() }) else {
            return false;
        };
        if word.is_null() {
            return false;
        }
        let Ok(word) = (unsafe { CStr::from_ptr(word) }).to_str() else {
            return false;
        };
        c.engine.is_vietnamese_spelling(word)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vk_character(value: u32) -> u32 {
    guarded(value, || crate::character_code(value))
}
#[unsafe(no_mangle)]
pub extern "C" fn vk_key_character(value: u32) -> u32 {
    crate::data::key_character(value)
}
/// # Safety
/// Prefix is NUL-terminated; out points to 27 doubles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_next_mass(vi: i32, prefix: *const c_char, out: *mut f64) {
    if prefix.is_null() || out.is_null() {
        return;
    }
    guarded((), || {
        if let Ok(prefix) = (unsafe { CStr::from_ptr(prefix) }).to_str() {
            let mass = crate::data::next_key_mass(vi != 0, prefix);
            unsafe { ptr::copy_nonoverlapping(mass.as_ptr(), out, 27) }
        }
    })
}
/// # Safety
/// Prefix is NUL-terminated; output arrays have capacity max (32 bytes per word).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_completions(
    vi: i32,
    prefix: *const c_char,
    out: *mut [c_char; 32],
    scores: *mut i32,
    max: i32,
) -> i32 {
    if prefix.is_null() || out.is_null() || scores.is_null() || max <= 0 {
        return 0;
    }
    guarded(0, || {
        let Ok(prefix) = (unsafe { CStr::from_ptr(prefix) }).to_str() else {
            return 0;
        };
        let words = crate::data::completions(vi != 0, prefix, max as usize);
        for (i, (word, score)) in words.iter().enumerate() {
            unsafe {
                ptr::write_bytes(out.add(i), 0, 1);
                ptr::copy_nonoverlapping(
                    word.as_ptr().cast::<c_char>(),
                    out.add(i).cast::<c_char>(),
                    word.len(),
                );
                *scores.add(i) = *score
            }
        }
        words.len() as i32
    })
}
/// # Safety
/// Context and data must be valid for the indicated length; null data is accepted only for length zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_smart_load(context: *mut Context, data: *const u8, len: usize) {
    if len > 0 && data.is_null() {
        return;
    }
    guarded((), || {
        if let Some(c) = unsafe { context.as_mut() } {
            c.smart = SmartSwitch::load(if len == 0 {
                &[]
            } else {
                unsafe { slice::from_raw_parts(data, len) }
            })
        }
    })
}
/// # Safety
/// Context is valid; nonnull out has capacity bytes. Returns the required byte count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_smart_save(
    context: *const Context,
    out: *mut u8,
    capacity: usize,
) -> usize {
    guarded(0, || {
        let Some(c) = (unsafe { context.as_ref() }) else {
            return 0;
        };
        let data = c.smart.save();
        if !out.is_null() && capacity >= data.len() {
            unsafe { ptr::copy_nonoverlapping(data.as_ptr(), out, data.len()) }
        }
        data.len()
    })
}
/// # Safety
/// Context and a NUL-terminated UTF-8 key are valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vk_smart_get(
    context: *mut Context,
    key: *const c_char,
    language: i32,
    set: bool,
) -> i32 {
    guarded(-1, || {
        let Some(c) = (unsafe { context.as_mut() }) else {
            return -1;
        };
        if key.is_null() {
            return -1;
        }
        let Ok(key) = (unsafe { CStr::from_ptr(key) }).to_str() else {
            return -1;
        };
        let language = if language == 0 {
            Language::English
        } else {
            Language::Vietnamese
        };
        if set {
            c.smart.set(key, language);
            language as i32
        } else {
            c.smart
                .lookup(key, language)
                .map(|v| v as i32)
                .unwrap_or(-1)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_layout_matches_native_header() {
        assert_eq!(size_of::<Config>(), 40);
        assert_eq!(size_of::<Hook>(), 132);
        assert_eq!(size_of::<Output>(), 412);
        assert_eq!(std::mem::offset_of!(Output, undo), 152);
        assert_eq!(std::mem::offset_of!(Output, restored), 284);
    }
    #[test]
    fn independent_native_contexts_and_hook_lifetime() {
        let config = Config {
            language: 1,
            spelling: 1,
            modern: 1,
            restore: 1,
            auto_switch: 1,
            teencode: 1,
            style: 0,
            previous_restored: 0,
            request: -1,
            last_side: -1,
        };
        // These pointers are allocated by this module and exclusively owned until destroy.
        unsafe {
            let a = vk_create(&config);
            let b = vk_create(&config);
            assert!(!a.is_null() && !b.is_null());
            vk_handle(a, 0, 0, KEY_A, 0, false);
            vk_handle(a, 0, 0, KEY_S, 0, false);
            let result = &*vk_output(a);
            assert_eq!(result.hook.code, HookCode::Process as u8);
            assert_eq!(result.hook.backspaces, 1);
            assert_eq!(rendered_char(result.hook.chars[0]), 'á' as u32);
            let expected = result.hook.chars[0];
            assert_eq!((*vk_output(b)).hook.code, 0);
            let snapshot = vk_snapshot(a);
            vk_reset(a, 0);
            vk_restore(a, snapshot);
            assert_eq!((*vk_output(a)).hook.chars[0], expected);
            vk_destroy(a);
            vk_destroy(b);
        }
    }
}
