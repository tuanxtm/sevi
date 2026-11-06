//! Upstream #32: Mutter must not receive deletes beyond the client's last report.
pub(crate) fn clamp_needed(session: Option<&str>, desktop: Option<&str>) -> bool {
    !session.is_some_and(|s| !s.is_empty() && s != "wayland")
        && !desktop.is_some_and(|s| !s.is_empty() && !s.contains("GNOME"))
}
pub(crate) fn delete_count(requested: usize, cursor: Option<u32>) -> usize {
    requested.min(cursor.unwrap_or(0) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_surrounding_delete_regressions() {
        for (requested, cursor, expected) in [
            (1, Some(3), 1),
            (3, Some(3), 3),
            (2, Some(0), 0),
            (3, Some(1), 1),
            (1, None, 0),
            (0, Some(5), 0),
        ] {
            assert_eq!(delete_count(requested, cursor), expected);
        }
        for (session, desktop, expected) in [
            (Some("wayland"), Some("GNOME"), true),
            (Some("wayland"), Some("ubuntu:GNOME"), true),
            (Some("x11"), Some("ubuntu:GNOME"), false),
            (Some("wayland"), Some("KDE"), false),
            (None, Some("GNOME"), true),
            (Some("wayland"), None, true),
            (None, None, true),
            (None, Some("KDE"), false),
            (Some("x11"), None, false),
            (Some(""), Some(""), true),
        ] {
            assert_eq!(
                clamp_needed(session, desktop),
                expected,
                "{session:?} {desktop:?}"
            );
        }
    }
}
