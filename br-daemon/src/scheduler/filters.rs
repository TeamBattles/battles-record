//! Filter matching for stream metadata to determine if a stream should be recorded.

use crate::config::FiltersConfig;
use serde::Serialize;

/** Stream metadata needed for filtering decisions. */
#[derive(Debug, Clone)]
pub struct StreamMetadata {
    pub title: String,
    pub game: Option<String>,
    pub viewer_count: Option<u32>,
}

/** Result of checking if a stream should be recorded. */
#[derive(Debug, Clone)]
pub struct RecordingDecision {
    pub should_record: bool,
    pub reason: DecisionReason,
}

/** The reason for the recording decision. */
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionReason {
    Allowed,
    TitleNotMatched { required: Vec<String> },
    TitleExcluded { pattern: String },
    GameNotMatched { required: Vec<String> },
    GameExcluded { pattern: String },
    ViewersBelowMinimum { current: u32, minimum: u32 },
}

/**
 * Matcher for stream metadata filters.
 *
 * Checks stream metadata against configured filters to determine
 * if a stream should be recorded.
 */
pub struct FilterMatcher;

impl FilterMatcher {
    /** Create a new FilterMatcher. */
    pub fn new() -> Self {
        Self
    }

    /**
     * Check if stream metadata passes all filters.
     *
     * Returns a RecordingDecision indicating whether the stream should be recorded
     * and the reason for the decision.
     *
     * Filter check order:
     * 1. title_contains - title must contain at least one pattern (if any specified)
     * 2. title_excludes - title must not contain any pattern
     * 3. game_contains - game must contain at least one pattern (if any specified)
     * 4. game_excludes - game must not contain any pattern
     * 5. min_viewers - viewer count must be >= minimum (if specified)
     *
     * All checks are case-insensitive.
     */
    pub fn matches(&self, filters: &FiltersConfig, metadata: &StreamMetadata) -> RecordingDecision {
        // Check title_contains (if non-empty, title must contain at least one)
        if !filters.title_contains.is_empty() {
            let title_lower = metadata.title.to_lowercase();
            let matches = filters
                .title_contains
                .iter()
                .any(|pattern| title_lower.contains(&pattern.to_lowercase()));

            if !matches {
                return RecordingDecision {
                    should_record: false,
                    reason: DecisionReason::TitleNotMatched {
                        required: filters.title_contains.clone(),
                    },
                };
            }
        }

        // Check title_excludes (title must not contain any pattern)
        let title_lower = metadata.title.to_lowercase();
        for pattern in &filters.title_excludes {
            if title_lower.contains(&pattern.to_lowercase()) {
                return RecordingDecision {
                    should_record: false,
                    reason: DecisionReason::TitleExcluded {
                        pattern: pattern.clone(),
                    },
                };
            }
        }

        // Check game_contains (if non-empty, game must contain at least one)
        if !filters.game_contains.is_empty() {
            match &metadata.game {
                Some(game) => {
                    let game_lower = game.to_lowercase();
                    let matches = filters
                        .game_contains
                        .iter()
                        .any(|pattern| game_lower.contains(&pattern.to_lowercase()));

                    if !matches {
                        return RecordingDecision {
                            should_record: false,
                            reason: DecisionReason::GameNotMatched {
                                required: filters.game_contains.clone(),
                            },
                        };
                    }
                }
                None => {
                    // No game info but filter is set - fail
                    return RecordingDecision {
                        should_record: false,
                        reason: DecisionReason::GameNotMatched {
                            required: filters.game_contains.clone(),
                        },
                    };
                }
            }
        }

        // Check game_excludes (game must not contain any pattern)
        if let Some(game) = &metadata.game {
            let game_lower = game.to_lowercase();
            for pattern in &filters.game_excludes {
                if game_lower.contains(&pattern.to_lowercase()) {
                    return RecordingDecision {
                        should_record: false,
                        reason: DecisionReason::GameExcluded {
                            pattern: pattern.clone(),
                        },
                    };
                }
            }
        }

        // Check min_viewers
        if let Some(min_viewers) = filters.min_viewers {
            let current = metadata.viewer_count.unwrap_or(0);
            if current < min_viewers {
                return RecordingDecision {
                    should_record: false,
                    reason: DecisionReason::ViewersBelowMinimum {
                        current,
                        minimum: min_viewers,
                    },
                };
            }
        }

        // All filters passed
        RecordingDecision {
            should_record: true,
            reason: DecisionReason::Allowed,
        }
    }
}

impl Default for FilterMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_filters() -> FiltersConfig {
        FiltersConfig::default()
    }

    fn sample_metadata() -> StreamMetadata {
        StreamMetadata {
            title: "Playing Minecraft with friends".to_string(),
            game: Some("Minecraft".to_string()),
            viewer_count: Some(1000),
        }
    }

    #[test]
    fn test_filter_empty_filters_allow_all() {
        let matcher = FilterMatcher::new();
        let filters = empty_filters();
        let metadata = sample_metadata();

        let decision = matcher.matches(&filters, &metadata);

        assert!(decision.should_record);
        assert!(matches!(decision.reason, DecisionReason::Allowed));
    }

    #[test]
    fn test_filter_title_contains_match() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.title_contains = vec!["minecraft".to_string(), "fortnite".to_string()];
        let metadata = sample_metadata();

        let decision = matcher.matches(&filters, &metadata);

        assert!(decision.should_record);
        assert!(matches!(decision.reason, DecisionReason::Allowed));
    }

    #[test]
    fn test_filter_title_contains_no_match() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.title_contains = vec!["fortnite".to_string(), "valorant".to_string()];
        let metadata = sample_metadata();

        let decision = matcher.matches(&filters, &metadata);

        assert!(!decision.should_record);
        match decision.reason {
            DecisionReason::TitleNotMatched { required } => {
                assert_eq!(required, vec!["fortnite".to_string(), "valorant".to_string()]);
            }
            _ => panic!("Expected TitleNotMatched reason"),
        }
    }

    #[test]
    fn test_filter_title_excludes() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.title_excludes = vec!["friends".to_string()];
        let metadata = sample_metadata();

        let decision = matcher.matches(&filters, &metadata);

        assert!(!decision.should_record);
        match decision.reason {
            DecisionReason::TitleExcluded { pattern } => {
                assert_eq!(pattern, "friends");
            }
            _ => panic!("Expected TitleExcluded reason"),
        }
    }

    #[test]
    fn test_filter_game_contains_match() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.game_contains = vec!["mine".to_string()];
        let metadata = sample_metadata();

        let decision = matcher.matches(&filters, &metadata);

        assert!(decision.should_record);
        assert!(matches!(decision.reason, DecisionReason::Allowed));
    }

    #[test]
    fn test_filter_game_contains_no_match() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.game_contains = vec!["fortnite".to_string()];
        let metadata = sample_metadata();

        let decision = matcher.matches(&filters, &metadata);

        assert!(!decision.should_record);
        match decision.reason {
            DecisionReason::GameNotMatched { required } => {
                assert_eq!(required, vec!["fortnite".to_string()]);
            }
            _ => panic!("Expected GameNotMatched reason"),
        }
    }

    #[test]
    fn test_filter_game_contains_no_game_info() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.game_contains = vec!["minecraft".to_string()];
        let metadata = StreamMetadata {
            title: "Playing something".to_string(),
            game: None,
            viewer_count: Some(100),
        };

        let decision = matcher.matches(&filters, &metadata);

        assert!(!decision.should_record);
        assert!(matches!(decision.reason, DecisionReason::GameNotMatched { .. }));
    }

    #[test]
    fn test_filter_game_excludes() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.game_excludes = vec!["minecraft".to_string()];
        let metadata = sample_metadata();

        let decision = matcher.matches(&filters, &metadata);

        assert!(!decision.should_record);
        match decision.reason {
            DecisionReason::GameExcluded { pattern } => {
                assert_eq!(pattern, "minecraft");
            }
            _ => panic!("Expected GameExcluded reason"),
        }
    }

    #[test]
    fn test_filter_min_viewers_pass() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.min_viewers = Some(500);
        let metadata = sample_metadata(); // has 1000 viewers

        let decision = matcher.matches(&filters, &metadata);

        assert!(decision.should_record);
        assert!(matches!(decision.reason, DecisionReason::Allowed));
    }

    #[test]
    fn test_filter_min_viewers_fail() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.min_viewers = Some(2000);
        let metadata = sample_metadata(); // has 1000 viewers

        let decision = matcher.matches(&filters, &metadata);

        assert!(!decision.should_record);
        match decision.reason {
            DecisionReason::ViewersBelowMinimum { current, minimum } => {
                assert_eq!(current, 1000);
                assert_eq!(minimum, 2000);
            }
            _ => panic!("Expected ViewersBelowMinimum reason"),
        }
    }

    #[test]
    fn test_filter_min_viewers_no_viewer_count() {
        let matcher = FilterMatcher::new();
        let mut filters = empty_filters();
        filters.min_viewers = Some(100);
        let metadata = StreamMetadata {
            title: "Test".to_string(),
            game: None,
            viewer_count: None, // treated as 0
        };

        let decision = matcher.matches(&filters, &metadata);

        assert!(!decision.should_record);
        match decision.reason {
            DecisionReason::ViewersBelowMinimum { current, minimum } => {
                assert_eq!(current, 0);
                assert_eq!(minimum, 100);
            }
            _ => panic!("Expected ViewersBelowMinimum reason"),
        }
    }

    #[test]
    fn test_filter_case_insensitive() {
        let matcher = FilterMatcher::new();

        // Test title_contains case insensitivity
        let mut filters = empty_filters();
        filters.title_contains = vec!["MINECRAFT".to_string()];
        let metadata = StreamMetadata {
            title: "playing minecraft today".to_string(),
            game: Some("Minecraft".to_string()),
            viewer_count: Some(100),
        };
        let decision = matcher.matches(&filters, &metadata);
        assert!(decision.should_record, "title_contains should be case-insensitive");

        // Test title_excludes case insensitivity
        let mut filters = empty_filters();
        filters.title_excludes = vec!["MINECRAFT".to_string()];
        let decision = matcher.matches(&filters, &metadata);
        assert!(!decision.should_record, "title_excludes should be case-insensitive");

        // Test game_contains case insensitivity
        let mut filters = empty_filters();
        filters.game_contains = vec!["MINECRAFT".to_string()];
        let decision = matcher.matches(&filters, &metadata);
        assert!(decision.should_record, "game_contains should be case-insensitive");

        // Test game_excludes case insensitivity
        let mut filters = empty_filters();
        filters.game_excludes = vec!["MINECRAFT".to_string()];
        let decision = matcher.matches(&filters, &metadata);
        assert!(!decision.should_record, "game_excludes should be case-insensitive");
    }

    #[test]
    fn test_filter_check_order() {
        let matcher = FilterMatcher::new();

        // Set up filters that would fail at multiple stages
        let mut filters = empty_filters();
        filters.title_contains = vec!["nonexistent".to_string()]; // Would fail first
        filters.title_excludes = vec!["playing".to_string()]; // Would fail second
        filters.game_contains = vec!["nonexistent".to_string()]; // Would fail third
        filters.min_viewers = Some(10000); // Would fail last

        let metadata = sample_metadata();
        let decision = matcher.matches(&filters, &metadata);

        // Should fail at title_contains first
        assert!(!decision.should_record);
        assert!(
            matches!(decision.reason, DecisionReason::TitleNotMatched { .. }),
            "Should fail at title_contains check first"
        );
    }
}
