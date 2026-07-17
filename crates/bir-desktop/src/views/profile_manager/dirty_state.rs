#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ComplianceDirtyState {
    pub profile: bool,
    pub forms_set: bool,
}

impl ComplianceDirtyState {
    pub fn from_comparison<P, F>(
        clean_profile: &P,
        current_profile: &P,
        clean_forms_set: &F,
        current_forms_set: &F,
    ) -> Self
    where
        P: PartialEq,
        F: PartialEq,
    {
        Self {
            profile: clean_profile != current_profile,
            forms_set: clean_forms_set != current_forms_set,
        }
    }

    pub fn any(self) -> bool {
        self.profile || self.forms_set
    }

    pub fn title(self) -> &'static str {
        match (self.profile, self.forms_set) {
            (true, true) => "Unsaved profile and Forms Set changes",
            (true, false) => "Unsaved profile changes",
            (false, true) => "Unsaved Forms Set changes",
            (false, false) => "No unsaved changes",
        }
    }

    pub fn navigation_message(self) -> &'static str {
        match (self.profile, self.forms_set) {
            (true, true) => {
                "Save or discard the pending profile and Forms Set changes before switching profiles or opening a filing form."
            }
            (true, false) => {
                "Save or discard the pending profile changes before switching profiles or opening a filing form."
            }
            (false, true) => {
                "Save or discard the pending Forms Set changes before switching profiles or opening a filing form."
            }
            (false, false) => "There are no unsaved profile or Forms Set changes.",
        }
    }

    pub fn discarded_message(self) -> &'static str {
        match (self.profile, self.forms_set) {
            (true, true) => "Unsaved profile and Forms Set changes were discarded.",
            (true, false) => "Unsaved profile changes were discarded.",
            (false, true) => "Unsaved Forms Set changes were discarded.",
            (false, false) => "There were no unsaved changes to discard.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ComplianceDirtyState;

    #[test]
    fn exact_baselines_are_clean() {
        let state =
            ComplianceDirtyState::from_comparison(&"profile", &"profile", &vec![1], &vec![1]);

        assert_eq!(state, ComplianceDirtyState::default());
        assert!(!state.any());
    }

    #[test]
    fn profile_and_forms_set_changes_are_classified_independently() {
        let profile_only =
            ComplianceDirtyState::from_comparison(&"saved", &"edited", &vec![1], &vec![1]);
        assert_eq!(
            profile_only,
            ComplianceDirtyState {
                profile: true,
                forms_set: false,
            }
        );
        assert_eq!(profile_only.title(), "Unsaved profile changes");
        assert_eq!(
            profile_only.discarded_message(),
            "Unsaved profile changes were discarded."
        );

        let forms_only =
            ComplianceDirtyState::from_comparison(&"saved", &"saved", &vec![1], &vec![1, 2]);
        assert_eq!(
            forms_only,
            ComplianceDirtyState {
                profile: false,
                forms_set: true,
            }
        );
        assert_eq!(forms_only.title(), "Unsaved Forms Set changes");
        assert_eq!(
            forms_only.discarded_message(),
            "Unsaved Forms Set changes were discarded."
        );
    }

    #[test]
    fn combined_changes_use_combined_copy() {
        let state = ComplianceDirtyState::from_comparison(&"saved", &"edited", &vec![1], &vec![2]);

        assert!(state.any());
        assert_eq!(state.title(), "Unsaved profile and Forms Set changes");
        assert!(state.navigation_message().contains("profile and Forms Set"));
    }
}
