//! Schedule checking logic with timezone support.

use crate::config::{ScheduleConfig, ScheduleRule};
use chrono::{Datelike, Timelike, Utc};
use chrono_tz::Tz;

/** Converts a chrono Weekday to lowercase string name. */
fn weekday_to_string(weekday: chrono::Weekday) -> String {
    match weekday {
        chrono::Weekday::Mon => "monday",
        chrono::Weekday::Tue => "tuesday",
        chrono::Weekday::Wed => "wednesday",
        chrono::Weekday::Thu => "thursday",
        chrono::Weekday::Fri => "friday",
        chrono::Weekday::Sat => "saturday",
        chrono::Weekday::Sun => "sunday",
    }
    .to_string()
}

/** Schedule checker for determining if current time is within recording schedule. */
pub struct ScheduleChecker;

impl ScheduleChecker {
    /** Create a new ScheduleChecker. */
    pub fn new() -> Self {
        Self
    }

    /**
     * Check if current time is within any schedule rule.
     *
     * Returns true if:
     * - schedule.enabled is false (disabled = always allowed), OR
     * - At least one rule matches the current day/time
     *
     * Returns false if enabled but no rules match.
     */
    pub fn is_within_schedule(&self, config: &ScheduleConfig) -> bool {
        // If schedule is disabled, always allow
        if !config.enabled {
            return true;
        }

        // If enabled but no rules, nothing matches
        if config.rules.is_empty() {
            return false;
        }

        // Parse timezone, default to UTC
        let tz: Tz = config
            .timezone
            .as_ref()
            .and_then(|tz_str| tz_str.parse().ok())
            .unwrap_or(chrono_tz::UTC);

        // Get current time in the specified timezone
        let now = Utc::now().with_timezone(&tz);
        let day = weekday_to_string(now.weekday());
        let time = format!("{:02}:{:02}", now.hour(), now.minute());

        self.check_rules_at(config, &day, &time)
    }

    /**
     * Check rules at a specific day/time (for testing).
     *
     * Day should be lowercase weekday name (monday, tuesday, etc.).
     * Time should be in HH:MM format.
     */
    pub fn check_rules_at(&self, config: &ScheduleConfig, day: &str, time: &str) -> bool {
        // If schedule is disabled, always allow
        if !config.enabled {
            return true;
        }

        // If enabled but no rules, nothing matches
        if config.rules.is_empty() {
            return false;
        }

        // Rules are OR'd - any match returns true
        for rule in &config.rules {
            if self.rule_matches(rule, day, time) {
                return true;
            }
        }

        false
    }

    /**
     * Check if a single rule matches the given day and time.
     *
     * Day is lowercase weekday name (monday, tuesday, etc.).
     * Time is HH:MM format.
     *
     * A rule matches if:
     * - Day is in rule.days (case-insensitive), AND
     * - Time >= start_time (if set), AND
     * - Time <= end_time (if set)
     */
    pub fn rule_matches(&self, rule: &ScheduleRule, day: &str, time: &str) -> bool {
        // Check day match (case-insensitive)
        let day_matches = rule
            .days
            .iter()
            .any(|d| d.to_lowercase() == day.to_lowercase());

        if !day_matches {
            return false;
        }

        // Check start_time constraint
        if let Some(ref start) = rule.start_time {
            if time < start.as_str() {
                return false;
            }
        }

        // Check end_time constraint
        if let Some(ref end) = rule.end_time {
            if time > end.as_str() {
                return false;
            }
        }

        true
    }
}

impl Default for ScheduleChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schedule(enabled: bool, rules: Vec<ScheduleRule>) -> ScheduleConfig {
        ScheduleConfig {
            enabled,
            timezone: None,
            rules,
        }
    }

    fn make_rule(days: Vec<&str>, start: Option<&str>, end: Option<&str>) -> ScheduleRule {
        ScheduleRule {
            days: days.into_iter().map(String::from).collect(),
            start_time: start.map(String::from),
            end_time: end.map(String::from),
        }
    }

    #[test]
    fn test_schedule_disabled_always_allows() {
        let checker = ScheduleChecker::new();
        let config = make_schedule(false, vec![]);

        // Even with no rules, disabled schedule should allow
        assert!(checker.is_within_schedule(&config));
        assert!(checker.check_rules_at(&config, "monday", "12:00"));
        assert!(checker.check_rules_at(&config, "sunday", "23:59"));
    }

    #[test]
    fn test_schedule_no_rules_denies() {
        let checker = ScheduleChecker::new();
        let config = make_schedule(true, vec![]);

        // Enabled with no rules should deny
        assert!(!checker.check_rules_at(&config, "monday", "12:00"));
        assert!(!checker.check_rules_at(&config, "friday", "18:00"));
    }

    #[test]
    fn test_schedule_day_match() {
        let checker = ScheduleChecker::new();
        let rule = make_rule(
            vec![
                "monday",
                "tuesday",
                "wednesday",
                "thursday",
                "friday",
                "saturday",
                "sunday",
            ],
            None,
            None,
        );
        let config = make_schedule(true, vec![rule]);

        // All days should match with no time restriction
        assert!(checker.check_rules_at(&config, "monday", "00:00"));
        assert!(checker.check_rules_at(&config, "wednesday", "12:00"));
        assert!(checker.check_rules_at(&config, "sunday", "23:59"));
    }

    #[test]
    fn test_schedule_time_range() {
        let checker = ScheduleChecker::new();
        // Rule: all days, 09:00 to 17:00
        let rule = make_rule(
            vec![
                "monday",
                "tuesday",
                "wednesday",
                "thursday",
                "friday",
                "saturday",
                "sunday",
            ],
            Some("09:00"),
            Some("17:00"),
        );
        let config = make_schedule(true, vec![rule]);

        // Within range
        assert!(checker.check_rules_at(&config, "monday", "09:00"));
        assert!(checker.check_rules_at(&config, "monday", "12:00"));
        assert!(checker.check_rules_at(&config, "monday", "17:00"));

        // Outside range
        assert!(!checker.check_rules_at(&config, "monday", "08:59"));
        assert!(!checker.check_rules_at(&config, "monday", "17:01"));
    }

    #[test]
    fn test_check_rule_day_mismatch() {
        let checker = ScheduleChecker::new();
        let rule = make_rule(vec!["monday", "wednesday", "friday"], None, None);

        // Matching days
        assert!(checker.rule_matches(&rule, "monday", "12:00"));
        assert!(checker.rule_matches(&rule, "wednesday", "12:00"));
        assert!(checker.rule_matches(&rule, "friday", "12:00"));

        // Non-matching days
        assert!(!checker.rule_matches(&rule, "tuesday", "12:00"));
        assert!(!checker.rule_matches(&rule, "thursday", "12:00"));
        assert!(!checker.rule_matches(&rule, "saturday", "12:00"));
        assert!(!checker.rule_matches(&rule, "sunday", "12:00"));
    }

    #[test]
    fn test_check_rule_time_before_start() {
        let checker = ScheduleChecker::new();
        let rule = make_rule(vec!["monday"], Some("10:00"), Some("18:00"));

        // Before start time
        assert!(!checker.rule_matches(&rule, "monday", "09:59"));
        assert!(!checker.rule_matches(&rule, "monday", "00:00"));

        // At or after start time
        assert!(checker.rule_matches(&rule, "monday", "10:00"));
        assert!(checker.rule_matches(&rule, "monday", "10:01"));
    }

    #[test]
    fn test_multiple_rules_or_logic() {
        let checker = ScheduleChecker::new();

        // Rule 1: Monday 09:00-12:00
        let rule1 = make_rule(vec!["monday"], Some("09:00"), Some("12:00"));

        // Rule 2: Friday 18:00-22:00
        let rule2 = make_rule(vec!["friday"], Some("18:00"), Some("22:00"));

        let config = make_schedule(true, vec![rule1, rule2]);

        // Matches rule 1
        assert!(checker.check_rules_at(&config, "monday", "10:00"));

        // Matches rule 2
        assert!(checker.check_rules_at(&config, "friday", "20:00"));

        // Matches neither
        assert!(!checker.check_rules_at(&config, "monday", "15:00"));
        assert!(!checker.check_rules_at(&config, "tuesday", "10:00"));
        assert!(!checker.check_rules_at(&config, "friday", "12:00"));
    }

    #[test]
    fn test_case_insensitive_days() {
        let checker = ScheduleChecker::new();
        let rule = make_rule(vec!["Monday", "TUESDAY", "WeDnEsDaY"], None, None);

        // Should match case-insensitively
        assert!(checker.rule_matches(&rule, "monday", "12:00"));
        assert!(checker.rule_matches(&rule, "MONDAY", "12:00"));
        assert!(checker.rule_matches(&rule, "tuesday", "12:00"));
        assert!(checker.rule_matches(&rule, "wednesday", "12:00"));
    }

    #[test]
    fn test_start_time_only() {
        let checker = ScheduleChecker::new();
        let rule = make_rule(vec!["monday"], Some("14:00"), None);

        // Before start
        assert!(!checker.rule_matches(&rule, "monday", "13:59"));

        // At and after start (no end time, so allowed)
        assert!(checker.rule_matches(&rule, "monday", "14:00"));
        assert!(checker.rule_matches(&rule, "monday", "23:59"));
    }

    #[test]
    fn test_end_time_only() {
        let checker = ScheduleChecker::new();
        let rule = make_rule(vec!["monday"], None, Some("14:00"));

        // Before and at end (no start time, so allowed from 00:00)
        assert!(checker.rule_matches(&rule, "monday", "00:00"));
        assert!(checker.rule_matches(&rule, "monday", "14:00"));

        // After end
        assert!(!checker.rule_matches(&rule, "monday", "14:01"));
    }
}
