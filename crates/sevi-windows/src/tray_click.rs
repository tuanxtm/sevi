//! Distinguish tray activation without toggling on either half of a double-click.
#[derive(Default)]
pub(crate) struct TrayClick {
    pending: bool,
    ignore_release: bool,
}
impl TrayClick {
    pub fn press(&mut self) {
        self.ignore_release = false;
    }
    pub fn release(&mut self) -> bool {
        if std::mem::take(&mut self.ignore_release) {
            return false;
        }
        self.pending = true;
        true
    }
    pub fn double_click(&mut self) {
        self.pending = false;
        self.ignore_release = true;
    }
    pub fn expire(&mut self) -> bool {
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_click_toggles_once_after_the_double_click_window() {
        let mut click = TrayClick::default();
        assert!(click.release());
        assert!(click.expire());
        assert!(!click.expire());
        assert!(click.release());
        assert!(click.expire());
    }

    #[test]
    fn double_click_does_not_toggle_on_either_release_or_a_stale_timer() {
        let mut click = TrayClick::default();
        assert!(click.release());
        click.double_click();
        assert!(!click.expire());
        assert!(!click.release());
        assert!(!click.expire());
        // A subsequent independent click still works normally.
        assert!(click.release());
        assert!(click.expire());
    }

    #[test]
    fn a_missing_double_click_release_does_not_swallow_the_next_click() {
        let mut click = TrayClick::default();
        assert!(click.release());
        click.double_click();
        click.press();
        assert!(click.release());
        assert!(click.expire());
    }
}
