#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleChoice {
    Town10Waves,
    Town20Waves,
    ResumeLocalRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TitleScreenState {
    selected: usize,
}

impl Default for TitleScreenState {
    fn default() -> Self {
        Self { selected: 0 }
    }
}

impl TitleScreenState {
    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn navigate(&mut self, delta: isize) -> bool {
        let previous = self.selected;
        self.selected = (self.selected as isize + delta).rem_euclid(3) as usize;
        previous != self.selected
    }

    pub fn confirm(&self) -> TitleChoice {
        match self.selected {
            1 => TitleChoice::Town20Waves,
            2 => TitleChoice::ResumeLocalRun,
            _ => TitleChoice::Town10Waves,
        }
    }
}

pub fn halloween_2026_active(unix_seconds: u64) -> bool {
    const START_UTC: u64 = 1_792_713_600;
    const END_UTC: u64 = 1_794_182_400;
    START_UTC < unix_seconds && unix_seconds < END_UTC
}

pub fn current_halloween_2026_active() -> bool {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .is_ok_and(|now| halloween_2026_active(now.as_secs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_selection_navigates_wraps_and_reports_changes() {
        let mut title = TitleScreenState::default();
        assert_eq!(title.selected(), 0);
        assert!(title.navigate(1));
        assert_eq!(title.selected(), 1);
        assert!(title.navigate(1));
        assert_eq!(title.selected(), 2);
        assert!(title.navigate(1));
        assert_eq!(title.selected(), 0);
        assert!(title.navigate(-1));
        assert_eq!(title.selected(), 2);
    }

    #[test]
    fn title_confirmation_maps_only_to_supported_local_actions() {
        let mut title = TitleScreenState::default();
        assert_eq!(title.confirm(), TitleChoice::Town10Waves);
        title.navigate(1);
        assert_eq!(title.confirm(), TitleChoice::Town20Waves);
        title.navigate(1);
        assert_eq!(title.confirm(), TitleChoice::ResumeLocalRun);
    }

    #[test]
    fn halloween_logo_and_banner_follow_the_pinned_utc_event_window() {
        assert!(!halloween_2026_active(1_792_713_600));
        assert!(halloween_2026_active(1_792_713_601));
        assert!(halloween_2026_active(1_794_000_000));
        assert!(!halloween_2026_active(1_794_182_400));
    }
}

use std::time::{SystemTime, UNIX_EPOCH};
