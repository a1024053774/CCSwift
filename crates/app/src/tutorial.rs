//! Tutorial visibility is independent of the persisted completion flag during replay.
#[derive(Default)]
pub(crate) struct Tutorial {
    pub step: Option<usize>,
}
impl Tutorial {
    pub fn new(done: bool) -> Self {
        Self { step: (!done).then_some(0) }
    }
    pub fn replay(&mut self) {
        self.step = Some(0);
    }
    pub fn dismiss(&mut self, done: &mut bool) {
        *done = true;
        self.step = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tutorial_flag_lifecycle() {
        // Missing flag in an older config must show; completion and skip must persist;
        // replay must not clear completion, even if the window is closed mid-tour.
        let cfg: cc_same_core::Config = serde_json::from_str("{}").unwrap();
        assert!(!cfg.tutorial_done);
        let mut done = cfg.tutorial_done;
        let mut tour = Tutorial::new(done);
        assert_eq!(tour.step, Some(0));
        for last_step in [0, 4] {
            done = false;
            tour.step = Some(last_step);
            tour.dismiss(&mut done);
            assert!(done);
            assert_eq!(tour.step, None);
            let saved = cc_same_core::Config { tutorial_done: done, ..Default::default() };
            let loaded: cc_same_core::Config = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
            assert_eq!(Tutorial::new(loaded.tutorial_done).step, None);
            tour.replay();
            assert_eq!(tour.step, Some(0));
            assert!(done);
            assert_eq!(Tutorial::new(done).step, None);
        }
    }
}
