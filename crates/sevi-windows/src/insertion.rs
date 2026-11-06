//! Checked delivery of the host's adjacent synthetic down/up pairs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Outcome {
    #[default]
    NotNeeded,
    Complete,
    Blocked,
    Partial {
        cleanup_complete: bool,
    },
}
impl Outcome {
    pub(crate) fn failed(self) -> bool {
        matches!(self, Self::Blocked | Self::Partial { .. })
    }
    pub(crate) fn consumed(self, requested: bool) -> bool {
        match self {
            Self::Blocked => false,
            Self::Partial { .. } => true,
            Self::NotNeeded | Self::Complete => requested,
        }
    }
}

/// Each pair must be a down followed by its matching up; no physical modifiers.
/// A short serial insertion can leave only the last accepted down unmatched.
/// Release it once, without replaying any text or destructive key-down events.
pub(crate) fn send_pairs<T>(
    batch: &[T],
    mut send: impl FnMut(&[T]) -> u32,
    release: impl FnOnce(&T) -> T,
) -> Outcome {
    if batch.is_empty() {
        return Outcome::NotNeeded;
    }
    debug_assert!(batch.len().is_multiple_of(2));
    let inserted = send(batch) as usize;
    if inserted == batch.len() {
        Outcome::Complete
    } else if inserted == 0 {
        Outcome::Blocked
    } else {
        let cleanup_complete = inserted.is_multiple_of(2)
            || send(&[release(&batch[inserted.min(batch.len()) - 1])]) == 1;
        Outcome::Partial { cleanup_complete }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Key {
        Backspace,
        Unicode(u16),
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Event {
        key: Key,
        up: bool,
    }
    fn replacement(text: &str) -> Vec<Event> {
        std::iter::once(Key::Backspace)
            .chain(text.encode_utf16().map(Key::Unicode))
            .flat_map(|key| [Event { key, up: false }, Event { key, up: true }])
            .collect()
    }
    #[derive(Default)]
    struct Editor {
        text: Vec<u16>,
        held: Vec<Key>,
        calls: Vec<Vec<Event>>,
    }
    impl Editor {
        fn send(&mut self, events: &[Event], limit: usize) -> u32 {
            self.calls.push(events.to_vec());
            let n = limit.min(events.len());
            for e in &events[..n] {
                if e.up {
                    self.held.retain(|key| *key != e.key);
                } else {
                    self.held.push(e.key);
                    match e.key {
                        Key::Backspace => {
                            self.text.pop();
                        }
                        Key::Unicode(c) => self.text.push(c),
                    }
                }
            }
            n as u32
        }
    }
    fn deliver(editor: &mut Editor, batch: &[Event], accepted: usize, cleanup: usize) -> Outcome {
        send_pairs(
            batch,
            |events| {
                let limit = if editor.calls.is_empty() {
                    accepted
                } else {
                    cleanup
                };
                editor.send(events, limit)
            },
            |e| Event { up: true, ..*e },
        )
    }
    #[test]
    fn no_output_keeps_hotkey_consumption_without_calling_sender() {
        let mut editor = Editor::default();
        let outcome = deliver(&mut editor, &[], 0, 0);
        assert_eq!(outcome, Outcome::NotNeeded);
        assert!(!outcome.failed());
        assert!(outcome.consumed(true));
        assert!(!outcome.consumed(false));
        assert!(editor.calls.is_empty());
    }
    #[test]
    fn successful_replacement_is_sent_once_and_swallows_original() {
        let mut editor = Editor {
            text: vec![b'a' as u16],
            ..Editor::default()
        };
        let batch = replacement("â");
        let outcome = deliver(&mut editor, &batch, batch.len(), 1);
        assert_eq!(outcome, Outcome::Complete);
        assert!(!outcome.failed());
        assert!(outcome.consumed(true));
        assert_eq!(editor.text, "â".encode_utf16().collect::<Vec<_>>());
        assert!(editor.held.is_empty());
        assert_eq!(editor.calls, [batch]);
    }
    #[test]
    fn zero_delivery_forwards_original_without_replaying_replacement() {
        let mut editor = Editor {
            text: vec![b'a' as u16],
            ..Editor::default()
        };
        let outcome = deliver(&mut editor, &replacement("â"), 0, 1);
        assert_eq!(outcome, Outcome::Blocked);
        assert!(outcome.failed());
        assert!(!outcome.consumed(true));
        // The hook passes the physical second 'a' through unchanged.
        editor.text.push(b'a' as u16);
        assert_eq!(editor.text, [b'a' as u16, b'a' as u16]);
        assert_eq!(editor.calls.len(), 1);
    }
    #[test]
    fn every_partial_prefix_stops_edits_and_only_releases_unmatched_down() {
        for text in ["â", "😀"] {
            let batch = replacement(text);
            for accepted in 1..batch.len() {
                let mut editor = Editor {
                    text: vec![b'a' as u16],
                    ..Editor::default()
                };
                let mut expected = Editor {
                    text: editor.text.clone(),
                    ..Editor::default()
                };
                expected.send(&batch, accepted);
                let outcome = deliver(&mut editor, &batch, accepted, 1);
                assert_eq!(
                    outcome,
                    Outcome::Partial {
                        cleanup_complete: true
                    }
                );
                assert!(outcome.failed());
                assert!(outcome.consumed(true));
                assert!(outcome.consumed(false));
                assert_eq!(editor.text, expected.text, "prefix {accepted} of {text}");
                assert!(editor.held.is_empty());
                assert_eq!(editor.calls.len(), 1 + accepted % 2);
                for cleanup in editor.calls.iter().skip(1) {
                    assert_eq!(cleanup.len(), 1);
                    assert!(cleanup[0].up);
                    assert_eq!(cleanup[0].key, batch[accepted - 1].key);
                }
            }
        }
    }
    #[test]
    fn blocked_key_up_cleanup_is_not_retried() {
        let mut editor = Editor {
            text: vec![b'a' as u16],
            ..Editor::default()
        };
        let outcome = deliver(&mut editor, &replacement("â"), 1, 0);
        assert_eq!(
            outcome,
            Outcome::Partial {
                cleanup_complete: false
            }
        );
        assert!(outcome.consumed(true));
        assert!(editor.text.is_empty());
        assert_eq!(editor.calls.len(), 2);
        assert_eq!(editor.held, [Key::Backspace]);
    }
}
