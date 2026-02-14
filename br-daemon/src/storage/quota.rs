//! Quota checking for storage limits.

use crate::config::QuotaConfig;

/** Bytes per gigabyte for quota calculations. */
const BYTES_PER_GB: u64 = 1024 * 1024 * 1024;

/** Result of a quota check. */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaCheckResult {
    /** Whether recording is allowed (under limit or no limit set). */
    pub allowed: bool,
    /** Whether usage has reached warning threshold. */
    pub warning: bool,
    /** Whether quota has been exceeded. */
    pub exceeded: bool,
    /** Current usage in bytes. */
    pub usage_bytes: u64,
    /** Limit in bytes (None if unlimited). */
    pub limit_bytes: Option<u64>,
    /** Usage as percentage of limit (0 if unlimited). */
    pub usage_percent: u8,
}

impl QuotaCheckResult {
    /** Create a result for unlimited quota. */
    fn unlimited(usage_bytes: u64) -> Self {
        Self {
            allowed: true,
            warning: false,
            exceeded: false,
            usage_bytes,
            limit_bytes: None,
            usage_percent: 0,
        }
    }

    /** Create a result with a specific limit. */
    fn with_limit(usage_bytes: u64, limit_bytes: u64, warn_at_percent: u8) -> Self {
        let usage_percent = if limit_bytes > 0 {
            ((usage_bytes as f64 / limit_bytes as f64) * 100.0).min(255.0) as u8
        } else {
            0
        };

        let exceeded = usage_bytes >= limit_bytes;
        let warning = usage_percent >= warn_at_percent;

        Self {
            allowed: !exceeded,
            warning,
            exceeded,
            usage_bytes,
            limit_bytes: Some(limit_bytes),
            usage_percent,
        }
    }
}

/** Checker for storage quotas. */
#[derive(Debug, Clone)]
pub struct QuotaChecker {
    config: QuotaConfig,
}

impl QuotaChecker {
    /** Create a new quota checker with the given configuration. */
    pub fn new(config: QuotaConfig) -> Self {
        Self { config }
    }

    /**
     * Check quota for a specific channel.
     *
     * Uses the per-channel limit from config if set, otherwise unlimited.
     * The `_channel` and `_platform` parameters are reserved for future
     * per-channel quota overrides.
     */
    pub fn check_channel_quota(
        &self,
        current_usage_bytes: u64,
        _channel: &str,
        _platform: &str,
    ) -> QuotaCheckResult {
        self.check_channel_quota_with_override(current_usage_bytes, self.config.per_channel_max_gb)
    }

    /**
     * Check quota for a channel with an optional override limit.
     *
     * If `limit_gb` is Some, uses that as the limit.
     * If `limit_gb` is None, the channel has unlimited quota.
     */
    pub fn check_channel_quota_with_override(
        &self,
        current_usage_bytes: u64,
        limit_gb: Option<u64>,
    ) -> QuotaCheckResult {
        match limit_gb {
            Some(gb) => {
                let limit_bytes = gb * BYTES_PER_GB;
                QuotaCheckResult::with_limit(
                    current_usage_bytes,
                    limit_bytes,
                    self.config.warn_at_percent,
                )
            }
            None => QuotaCheckResult::unlimited(current_usage_bytes),
        }
    }

    /**
     * Check global storage quota.
     *
     * Uses the global limit from config if set, otherwise unlimited.
     */
    pub fn check_global_quota(&self, total_usage_bytes: u64) -> QuotaCheckResult {
        match self.config.global_max_gb {
            Some(gb) => {
                let limit_bytes = gb * BYTES_PER_GB;
                QuotaCheckResult::with_limit(
                    total_usage_bytes,
                    limit_bytes,
                    self.config.warn_at_percent,
                )
            }
            None => QuotaCheckResult::unlimited(total_usage_bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gb_to_bytes(gb: u64) -> u64 {
        gb * BYTES_PER_GB
    }

    #[test]
    fn test_quota_check_under_limit() {
        let config = QuotaConfig {
            global_max_gb: Some(100),
            per_channel_max_gb: Some(10),
            warn_at_percent: 80,
        };
        let checker = QuotaChecker::new(config);

        // 5 GB used out of 10 GB limit = 50%
        let result = checker.check_channel_quota(gb_to_bytes(5), "test_channel", "twitch");

        assert!(result.allowed);
        assert!(!result.warning);
        assert!(!result.exceeded);
        assert_eq!(result.usage_bytes, gb_to_bytes(5));
        assert_eq!(result.limit_bytes, Some(gb_to_bytes(10)));
        assert_eq!(result.usage_percent, 50);
    }

    #[test]
    fn test_quota_check_warning() {
        let config = QuotaConfig {
            global_max_gb: Some(100),
            per_channel_max_gb: Some(10),
            warn_at_percent: 80,
        };
        let checker = QuotaChecker::new(config);

        // 8 GB used out of 10 GB limit = 80% (exactly at warning threshold)
        let result = checker.check_channel_quota(gb_to_bytes(8), "test_channel", "twitch");

        assert!(result.allowed);
        assert!(result.warning);
        assert!(!result.exceeded);
        assert_eq!(result.usage_percent, 80);

        // 9 GB used out of 10 GB limit = 90% (above warning threshold)
        let result = checker.check_channel_quota(gb_to_bytes(9), "test_channel", "twitch");

        assert!(result.allowed);
        assert!(result.warning);
        assert!(!result.exceeded);
        assert_eq!(result.usage_percent, 90);
    }

    #[test]
    fn test_quota_check_exceeded() {
        let config = QuotaConfig {
            global_max_gb: Some(100),
            per_channel_max_gb: Some(10),
            warn_at_percent: 80,
        };
        let checker = QuotaChecker::new(config);

        // 10 GB used out of 10 GB limit = 100% (at limit, exceeded)
        let result = checker.check_channel_quota(gb_to_bytes(10), "test_channel", "twitch");

        assert!(!result.allowed);
        assert!(result.warning);
        assert!(result.exceeded);
        assert_eq!(result.usage_percent, 100);

        // 15 GB used out of 10 GB limit = 150% (over limit)
        let result = checker.check_channel_quota(gb_to_bytes(15), "test_channel", "twitch");

        assert!(!result.allowed);
        assert!(result.warning);
        assert!(result.exceeded);
        assert_eq!(result.usage_percent, 150);
    }

    #[test]
    fn test_quota_no_limit() {
        let config = QuotaConfig {
            global_max_gb: None,
            per_channel_max_gb: None,
            warn_at_percent: 80,
        };
        let checker = QuotaChecker::new(config);

        // No limit set, should always be allowed
        let result = checker.check_channel_quota(gb_to_bytes(1000), "test_channel", "twitch");

        assert!(result.allowed);
        assert!(!result.warning);
        assert!(!result.exceeded);
        assert_eq!(result.usage_bytes, gb_to_bytes(1000));
        assert_eq!(result.limit_bytes, None);
        assert_eq!(result.usage_percent, 0);

        // Global quota with no limit
        let result = checker.check_global_quota(gb_to_bytes(5000));

        assert!(result.allowed);
        assert!(!result.warning);
        assert!(!result.exceeded);
        assert_eq!(result.limit_bytes, None);
    }

    #[test]
    fn test_per_channel_override() {
        let config = QuotaConfig {
            global_max_gb: Some(100),
            per_channel_max_gb: Some(10),
            warn_at_percent: 80,
        };
        let checker = QuotaChecker::new(config);

        // Override with a larger limit (20 GB instead of default 10 GB)
        let result = checker.check_channel_quota_with_override(gb_to_bytes(15), Some(20));

        assert!(result.allowed);
        assert!(!result.warning); // 75% < 80%
        assert!(!result.exceeded);
        assert_eq!(result.limit_bytes, Some(gb_to_bytes(20)));
        assert_eq!(result.usage_percent, 75);

        // Override with unlimited
        let result = checker.check_channel_quota_with_override(gb_to_bytes(100), None);

        assert!(result.allowed);
        assert!(!result.warning);
        assert!(!result.exceeded);
        assert_eq!(result.limit_bytes, None);
        assert_eq!(result.usage_percent, 0);

        // Override with smaller limit (5 GB instead of default 10 GB)
        let result = checker.check_channel_quota_with_override(gb_to_bytes(4), Some(5));

        assert!(result.allowed);
        assert!(result.warning); // 80% >= 80%
        assert!(!result.exceeded);
        assert_eq!(result.usage_percent, 80);
    }

    #[test]
    fn test_global_quota() {
        let config = QuotaConfig {
            global_max_gb: Some(100),
            per_channel_max_gb: Some(10),
            warn_at_percent: 80,
        };
        let checker = QuotaChecker::new(config);

        // Under global limit
        let result = checker.check_global_quota(gb_to_bytes(50));
        assert!(result.allowed);
        assert!(!result.warning);
        assert_eq!(result.usage_percent, 50);

        // At warning threshold
        let result = checker.check_global_quota(gb_to_bytes(80));
        assert!(result.allowed);
        assert!(result.warning);
        assert_eq!(result.usage_percent, 80);

        // Exceeded
        let result = checker.check_global_quota(gb_to_bytes(100));
        assert!(!result.allowed);
        assert!(result.exceeded);
    }
}
