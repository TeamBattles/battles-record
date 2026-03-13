//! Episode numbering persistence for Jellyfin exports.
//!
//! Tracks episode numbers per channel with year-based seasons.
//! Season number = year (2024, 2025, ...).
//! Episodes are numbered sequentially within each year.

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
    /** Current month (kept for reference, not used for season determination). */
    pub current_month: u32,
    /** Current day (kept for reference, not used for season determination). */
    pub current_day: u32,
    /** Next episode number within current season (year). */
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
     * Season number = year (2024, 2025, ...).
     * Episode numbers reset when the year changes.
     */
    pub fn get_next_episode(
        &mut self,
        platform: &str,
        channel_name: &str,
        recording_date: DateTime<Utc>,
    ) -> (u32, u32) {
        let key = format!(
            "{}/{}",
            platform.to_lowercase(),
            channel_name.to_lowercase()
        );
        let year = recording_date.year() as u32;
        let month = recording_date.month();
        let day = recording_date.day();

        let info = self
            .channels
            .entry(key)
            .or_insert_with(|| ChannelEpisodeInfo {
                current_year: year,
                current_month: month,
                current_day: day,
                next_episode: 1,
            });

        // Reset episodes only on year rollover
        if info.current_year != year {
            debug!("Season rollover: year {} -> {}", info.current_year, year);
            info.current_year = year;
            info.current_month = month;
            info.current_day = day;
            info.next_episode = 1;
        } else {
            info.current_month = month;
            info.current_day = day;
        }

        let season = year;
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
        let key = format!(
            "{}/{}",
            platform.to_lowercase(),
            channel_name.to_lowercase()
        );
        let year = recording_date.year() as u32;

        match self.channels.get(&key) {
            Some(info) => {
                if info.current_year != year {
                    (year, 1)
                } else {
                    (year, info.next_episode)
                }
            }
            None => (year, 1),
        }
    }

    /** Count episodes for a season (year). */
    pub fn count_episodes_for_season(
        &self,
        platform: &str,
        channel_name: &str,
        date: DateTime<Utc>,
    ) -> u32 {
        let key = format!(
            "{}/{}",
            platform.to_lowercase(),
            channel_name.to_lowercase()
        );
        let year = date.year() as u32;

        match self.channels.get(&key) {
            Some(info) => {
                if info.current_year == year {
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

        debug!(
            "Saved episode tracker with {} channels",
            self.channels.len()
        );
        Ok(())
    }

    /** Get info for a specific channel (for display/debugging). */
    pub fn get_channel_info(
        &self,
        platform: &str,
        channel_name: &str,
    ) -> Option<&ChannelEpisodeInfo> {
        let key = format!(
            "{}/{}",
            platform.to_lowercase(),
            channel_name.to_lowercase()
        );
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

        let jan_15_morning = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let jan_15_evening = Utc.with_ymd_and_hms(2024, 1, 15, 20, 0, 0).unwrap();

        let (season, episode) = tracker.get_next_episode("twitch", "xqc", jan_15_morning);
        assert_eq!(season, 2024);
        assert_eq!(episode, 1);

        let (season, episode) = tracker.get_next_episode("twitch", "xqc", jan_15_evening);
        assert_eq!(season, 2024);
        assert_eq!(episode, 2);
    }

    #[test]
    fn test_month_change_same_year_continues() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let jan_31 = Utc.with_ymd_and_hms(2024, 1, 31, 12, 0, 0).unwrap();
        let feb_1 = Utc.with_ymd_and_hms(2024, 2, 1, 12, 0, 0).unwrap();

        let (season, episode) = tracker.get_next_episode("twitch", "test", jan_31);
        assert_eq!(season, 2024);
        assert_eq!(episode, 1);

        // Month change within same year does NOT reset
        let (season, episode) = tracker.get_next_episode("twitch", "test", feb_1);
        assert_eq!(season, 2024);
        assert_eq!(episode, 2);
    }

    #[test]
    fn test_year_rollover_resets() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let dec_31_2024 = Utc.with_ymd_and_hms(2024, 12, 31, 12, 0, 0).unwrap();
        let jan_1_2025 = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();

        let (season, episode) = tracker.get_next_episode("twitch", "test", dec_31_2024);
        assert_eq!(season, 2024);
        assert_eq!(episode, 1);

        let (season, episode) = tracker.get_next_episode("twitch", "test", jan_1_2025);
        assert_eq!(season, 2025);
        assert_eq!(episode, 1);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        {
            let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();
            let date = Utc.with_ymd_and_hms(2024, 3, 15, 12, 0, 0).unwrap();
            tracker.get_next_episode("twitch", "streamer", date);
            tracker.get_next_episode("twitch", "streamer", date);
            tracker.save().unwrap();
        }

        {
            let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();
            // Different month, same year — should continue
            let same_year = Utc.with_ymd_and_hms(2024, 6, 20, 18, 0, 0).unwrap();
            let (season, episode) = tracker.get_next_episode("twitch", "streamer", same_year);
            assert_eq!(season, 2024);
            assert_eq!(episode, 3);
        }
    }

    #[test]
    fn test_multiple_channels() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let date = Utc.with_ymd_and_hms(2024, 5, 10, 12, 0, 0).unwrap();

        let (s1, ep1) = tracker.get_next_episode("twitch", "channel_a", date);
        let (s2, ep2) = tracker.get_next_episode("twitch", "channel_b", date);
        let (s3, ep3) = tracker.get_next_episode("youtube", "channel_a", date);

        assert_eq!(s1, 2024);
        assert_eq!(s2, 2024);
        assert_eq!(s3, 2024);
        assert_eq!(ep1, 1);
        assert_eq!(ep2, 1);
        assert_eq!(ep3, 1);
    }

    #[test]
    fn test_count_episodes_for_season() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let jan_15 = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let mar_20 = Utc.with_ymd_and_hms(2024, 3, 20, 12, 0, 0).unwrap();

        assert_eq!(
            tracker.count_episodes_for_season("twitch", "test", jan_15),
            0
        );

        tracker.get_next_episode("twitch", "test", jan_15);
        tracker.get_next_episode("twitch", "test", jan_15);
        tracker.get_next_episode("twitch", "test", mar_20);

        // All 3 counted — same year regardless of month
        assert_eq!(
            tracker.count_episodes_for_season("twitch", "test", jan_15),
            3
        );
        assert_eq!(
            tracker.count_episodes_for_season("twitch", "test", mar_20),
            3
        );
    }

    #[test]
    fn test_peek_next_episode() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        let jan = Utc.with_ymd_and_hms(2024, 1, 10, 12, 0, 0).unwrap();
        let feb = Utc.with_ymd_and_hms(2024, 2, 10, 12, 0, 0).unwrap();

        let (season, ep) = tracker.peek_next_episode("twitch", "test", jan);
        assert_eq!(season, 2024);
        assert_eq!(ep, 1);

        tracker.get_next_episode("twitch", "test", jan);

        // Same year different month — continues
        let (season, ep) = tracker.peek_next_episode("twitch", "test", feb);
        assert_eq!(season, 2024);
        assert_eq!(ep, 2);
    }

    #[test]
    fn test_backward_compat_deserialization() {
        let temp_dir = TempDir::new().unwrap();
        let tracker_path = temp_dir.path().join(".jellyfin_episodes.json");

        // Old month-based tracker data
        let old_data = r#"{
            "twitch/streamer": {
                "current_year": 2024,
                "current_month": 3,
                "current_day": 15,
                "next_episode": 5
            }
        }"#;
        std::fs::write(&tracker_path, old_data).unwrap();

        let mut tracker = EpisodeTracker::new(temp_dir.path().to_path_buf()).unwrap();

        // Same year — should continue from episode 5
        let date = Utc.with_ymd_and_hms(2024, 6, 10, 12, 0, 0).unwrap();
        let (season, episode) = tracker.get_next_episode("twitch", "streamer", date);
        assert_eq!(season, 2024);
        assert_eq!(episode, 5);
    }
}
