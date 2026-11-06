use crate::Language;
use std::collections::BTreeMap;

/// Compatible with the legacy little-endian count, one-byte key length and language byte.
#[derive(Clone, Default)]
pub struct SmartSwitch {
    entries: BTreeMap<Vec<u8>, u8>,
    cache: Option<(Vec<u8>, u8)>,
}
impl SmartSwitch {
    pub fn load(bytes: &[u8]) -> Self {
        let mut s = Self::default();
        let Some(count) = bytes.get(..2).map(|b| u16::from_le_bytes([b[0], b[1]])) else {
            return s;
        };
        let mut cursor = 2;
        for _ in 0..count {
            let Some(&len) = bytes.get(cursor) else { break };
            cursor += 1;
            let end = cursor + len as usize;
            if end >= bytes.len() {
                break;
            }
            s.entries.insert(bytes[cursor..end].to_vec(), bytes[end]);
            cursor = end + 1
        }
        s
    }
    pub fn save(&self) -> Vec<u8> {
        let mut out = vec![0, 0];
        let mut n = 0u16;
        for (key, value) in &self.entries {
            if key.len() > 255 {
                continue;
            }
            out.push(key.len() as u8);
            out.extend_from_slice(key);
            out.push(*value);
            n = n.wrapping_add(1)
        }
        out[..2].copy_from_slice(&n.to_le_bytes());
        out
    }
    /// A missing context is remembered with the current language and returns None on its first lookup.
    pub fn lookup(&mut self, key: &str, current: Language) -> Option<Language> {
        let key = key.as_bytes();
        if let Some((cached, value)) = &self.cache
            && cached == key
        {
            return Some(if value & 1 != 0 {
                Language::Vietnamese
            } else {
                Language::English
            });
        }
        let value = self.entries.get(key).copied();
        let stored = value.unwrap_or(current as u8);
        if value.is_none() {
            self.entries.insert(key.to_vec(), stored);
        }
        self.cache = Some((key.to_vec(), stored));
        value.map(|v| {
            if v & 1 != 0 {
                Language::Vietnamese
            } else {
                Language::English
            }
        })
    }
    pub fn set(&mut self, key: &str, language: Language) {
        self.entries.insert(key.as_bytes().to_vec(), language as u8);
        self.cache = Some((key.as_bytes().to_vec(), language as u8));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_format_and_truncated_tail() {
        let bytes = [2, 0, 3, b'a', b'p', b'p', 1, 3, b'b'];
        let mut s = SmartSwitch::load(&bytes);
        assert_eq!(
            s.lookup("app", Language::English),
            Some(Language::Vietnamese)
        );
        assert_eq!(s.save(), [1, 0, 3, b'a', b'p', b'p', 1]);
        assert_eq!(s.lookup("new", Language::English), None);
        assert_eq!(
            s.lookup("new", Language::Vietnamese),
            Some(Language::English)
        );
    }
    #[test]
    fn long_keys_not_serialized() {
        let mut s = SmartSwitch::default();
        s.set(&"x".repeat(256), Language::Vietnamese);
        assert_eq!(s.save(), [0, 0])
    }
}
