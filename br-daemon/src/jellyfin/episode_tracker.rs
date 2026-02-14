//! Episode numbering persistence for Jellyfin exports.
//!
//! Tracks episode numbers per channel with season groupings based on month.
//! Season number = month (1-12)
//! Episodes are numbered sequentially within each month (season).

use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tracing::debug;

const TRACKER_FILENAME: &str = ".jellyfin_episodes.json";

/** Information about episode numbering for a channel. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEpisodeInfo {
    /** Current year being tracked. */
    pub current_year: u32,
    /** Current month being tracked. */
    pub current_month: u32,
    /** Current day (kept for reference, not used for season determination). */
    pub current_day: u32,
    /** Next episode number within current season (month). */
    pub next_episode: u32,
}

impl Default for ChannelEpisodeInfo {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            current_year: now.year() as u32,
            current_month: now.month(),
            current_day: now.day(),
            next_episode: 1,
        }
    }
}

/** Tracks episode numbers across all channels. */
#[derive(Debug)]
pub struct EpisodeTracker {
    /** Path to the library directory where tracker is stored. */
    library_dir: PathBuf,
    /** Episode info per channel (key: "platform/channel_name"). */
    channels: HashMap<String, ChannelEpisodeInfo>,
}

impl EpisodeTracker {
    /** Create or load an episode tracker from the library directory. */
    pub fn new(library_dir: PathBuf) -> anyhow::Result<Self> {
        let tracker_path = library_dir.join(TRACKER_FILENAME);
        let channels = if tracker_path.exists() {
            let content = fs::read_to_string(&tracker_path)?;
            serde_json::from_str(&content)?
        } else {
            HashMap::new()
        };

        Ok(Self {
            library_dir,
            channels,
        })
    }

    /**
     * Get the next episode number for a recording.
     *
     * Returns (season_number, episode_number).
     * Season number = month (1-12).
     * Episode numbers reset when the month changes.
     */
    pub fn get_next_episode(
        &mut self,
        platform: &str,
        channel_name: &str,
        recording_date: DateTime<Utc>,
    ) -> (u32, u32) {
        let key = format!("{}/{}", platform.to_lowercase(), channel_name.to_lowercase());
        let year = recording_date.year() as u32;
        let month = recording_date.month();
        let day = recording_date.day();

        let info = self.channels.entry(key).or_insert_with(|| ChannelEpisodeInfo {
            current_year: year,
            current_month: month,
            current_day: day,
            next_episode: 1,
        });

        // Check if we've rolled over to a new month/year (day changes don't reset episodes)
        if info.current_year != year || info.current_month != month {
            debug!(
                "Season rollover: {}/{} -> {}/{}",
                info.current_year, info.current_month, year, month
            );
            info.current_year = year;
            info.current_month = month;
            info.current_day = day;
            info.next_episode = 1;
        } else {
            // Keep day updated for reference even if no rollover
            info.current_day = day;
        }

        // Season number = month (1-12)
        let season = month;
        let episode = info.next_episode;
        info.next_episode += 1;

        (season, episode)
    }

    /** Peek at what the next episode number would be without incrementing. */
    pub fn peek_next_episode(
        &self,
        platform: &str,
        channel_name: &str,
        recording_date: DateTime<Utc>,
    ) -> (u32, u32) {
        let key = format!("{}/{}", platform.to_lowercase(), channel_name.to_lowercase());
        let year = recording_date.year() as u32;
        let month = recording_date.month();

        match self.channels.get(&key) {
            Some(info) => {
                // Check if month changed
                if info.current_year != year || info.current_month != month {
                    (month, 1)
                } else {
                    (month, info.next_episode)
                }
            }
            None => (month, 1),
        }
    }

    /** Count episodes for a specific month (season). */
    pub fn count_episodes_for_month(
        &self,
        platform: &str,
        channel_name: &str,
        date: DateTime<Utc>,
    ) -> u32 {
        let key = format!("{}/{}", platform.to_lowercase(), channel_name.to_lowercase());
        let year = date.year() as u32;
        let month = date.month();

        match self.channels.get(&key) {
            Some(info) => {
                if info.current_year == year && info.current_month == month {
                    // Episodes are 1-indexed, so next_episode - 1 = count
                    info.next_episode.saturating_sub(1)
                } else {
                    0
                }
            }
            None => 0,
        }
    }

    /** Save the tracker state to disk. */
    pub fn save(&self) -> anyhow::Result<()> {
        let tracker_path = self.library_dir.join(TRACKER_FILENAME);
        let tmp_path = self.library_dir.join(".jellyfin_episodes.json.tmp");

        let content = serde_json::to_string_pretty(&self.channels)?;
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;

        // Atomic rename
        fs::rename(&tmp_path, &tracker_path)?;

        debug!("Saved episode tracker with {} channels", self.channels.len());
        Ok(())
    }

    /** Get info for a specific channel (for display/debugging). */
    pub fn get_channel_info(&self, platform: &str, channel_name: &str) -> Option<&ChannelEpisodeInfo> {
        let key = format!("{}/{}", platform.to_lowercase(), channel_name.to_lowercase());
        self.channels.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    #[test]
    fn test_episode_numbering_same_day() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        // Two recordings on the same day (January 15)
        let jan_15_morning = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let jan_15_evening = Utc.with_ymd_and_hms(2024, 1, 15, 20, 0, 0).unwrap();

        // First episode on January 15
        let (season, episode) = tracker.get_next_episode("twitch", "xqc", jan_15_morning);
        assert_eq!(season, 1); // Month = January
        assert_eq!(episode, 1);

        // Second episode same day
        let (season, episode) = tracker.get_next_episode("twitch", "xqc", jan_15_evening);
        assert_eq!(season, 1); // Still January
        assert_eq!(episode, 2);
    }

    #[test]
    fn test_day_rollover_same_month() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let jan_15 = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let jan_16 = Utc.with_ymd_and_hms(2024, 1, 16, 12, 0, 0).unwrap();

        // Episode on January 15
        let (season, episode) = tracker.get_next_episode("twitch", "test", jan_15);
        assert_eq!(season, 1); // Month = January
        assert_eq!(episode, 1);

        // Episode on January 16 (same month, so same season)
        let (season, episode) = tracker.get_next_episode("twitch", "test", jan_16);
        assert_eq!(season, 1); // Still January
        assert_eq!(episode, 2); // Continues from previous
    }

    #[test]
    fn test_month_rollover() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let jan_31 = Utc.with_ymd_and_hms(2024, 1, 31, 12, 0, 0).unwrap();
        let feb_1 = Utc.with_ymd_and_hms(2024, 2, 1, 12, 0, 0).unwrap();

        // Episode on January 31
        let (season, episode) = tracker.get_next_episode("twitch", "test", jan_31);
        assert_eq!(season, 1); // Month = January
        assert_eq!(episode, 1);

        // Episode on February 1 (new month = new season)
        let (season, episode) = tracker.get_next_episode("twitch", "test", feb_1);
        assert_eq!(season, 2); // Month = February
        assert_eq!(episode, 1); // Reset for new month
    }

    #[test]
    fn test_year_rollover() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let dec_31_2024 = Utc.with_ymd_and_hms(2024, 12, 31, 12, 0, 0).unwrap();
        let jan_1_2025 = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();

        // Episode on December 31, 2024
        let (season, episode) = tracker.get_next_episode("twitch", "test", dec_31_2024);
        assert_eq!(season, 12); // Month = December
        assert_eq!(episode, 1);

        // Episode on January 1, 2025 (new year = new season)
        let (season, episode) = tracker.get_next_episode("twitch", "test", jan_1_2025);
        assert_eq!(season, 1); // Month = January
        assert_eq!(episode, 1); // Reset for new year/month
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create tracker and add episodes
        {
            let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();
            let date = Utc.with_ymd_and_hms(2024, 3, 15, 12, 0, 0).unwrap();
            tracker.get_next_episode("twitch", "streamer", date);
            tracker.get_next_episode("twitch", "streamer", date);
            tracker.save().unwrap();
        }

        // Reload and verify state persists
        {
            let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();
            // Same month (different day within March)
            let same_month = Utc.with_ymd_and_hms(2024, 3, 20, 18, 0, 0).unwrap();
            let (season, episode) = tracker.get_next_episode("twitch", "streamer", same_month);
            assert_eq!(season, 3); // Month = March
            assert_eq!(episode, 3); // Should continue from 3
        }
    }

    #[test]
    fn test_multiple_channels() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let date = Utc.with_ymd_and_hms(2024, 5, 10, 12, 0, 0).unwrap();

        // Different channels should have independent numbering
        let (s1, ep1) = tracker.get_next_episode("twitch", "channel_a", date);
        let (s2, ep2) = tracker.get_next_episode("twitch", "channel_b", date);
        let (s3, ep3) = tracker.get_next_episode("youtube", "channel_a", date);

        // All in May (month 5)
        assert_eq!(s1, 5);
        assert_eq!(s2, 5);
        assert_eq!(s3, 5);

        // All episode 1 (independent tracking)
        assert_eq!(ep1, 1);
        assert_eq!(ep2, 1);
        assert_eq!(ep3, 1);
    }

    #[test]
    fn test_count_episodes_for_month() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let jan_15 = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let jan_20 = Utc.with_ymd_and_hms(2024, 1, 20, 12, 0, 0).unwrap();

        // No episodes yet
        assert_eq!(tracker.count_episodes_for_month("twitch", "test", jan_15), 0);

        // Add some episodes on different days in January
        tracker.get_next_episode("twitch", "test", jan_15);
        tracker.get_next_episode("twitch", "test", jan_15);
        tracker.get_next_episode("twitch", "test", jan_20);

        // All 3 episodes should be counted (same month)
        assert_eq!(tracker.count_episodes_for_month("twitch", "test", jan_15), 3);
        assert_eq!(tracker.count_episodes_for_month("twitch", "test", jan_20), 3);
    }
}
