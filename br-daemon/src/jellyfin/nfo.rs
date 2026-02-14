//! NFO XML generation for Jellyfin metadata.
//!
//! Generates tvshow.nfo, season.nfo, and episode.nfo files that Jellyfin
//! can parse to display proper metadata for stream recordings.

use chrono::{DateTime, Utc};
use std::path::Path;

/** Generate tvshow.nfo for a channel (series-level metadata). */
pub fn generate_tvshow_nfo(
    display_name: &str,
    platform: &str,
    description: Option<&str>,
    first_aired: DateTime<Utc>,
) -> String {
    let default_desc = format!("{} streamer on {}", display_name, platform);
    let desc = description.unwrap_or(&default_desc);
    let premiered = first_aired.format("%Y-%m-%d").to_string();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<tvshow>
    <title>{}</title>
    <plot>{}</plot>
    <premiered>{}</premiered>
    <studio>{}</studio>
    <genre>Live Stream</genre>
    <genre>Gaming</genre>
</tvshow>
"#,
        escape_xml(display_name),
        escape_xml(desc),
        premiered,
        escape_xml(&capitalize(platform))
    )
}

/**
 * Generate season.nfo for a season (month-based grouping).
 *
 * Season number = month (1-12).
 */
pub fn generate_season_nfo(season_number: u32, year: u32, month: u32) -> String {
    let month_name = month_name(month);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<season>
    <seasonnumber>{}</seasonnumber>
    <title>{} {}</title>
</season>
"#,
        season_number, month_name, year
    )
}

/** Episode metadata for NFO generation. */
pub struct EpisodeMetadata<'a> {
    pub title: &'a str,
    pub show_title: &'a str,
    pub season: u32,
    pub episode: u32,
    pub aired: DateTime<Utc>,
    pub game: Option<&'a str>,
    pub duration_minutes: u64,
    pub recording_id: &'a str,
}

/** Generate episode.nfo for a single recording. */
pub fn generate_episode_nfo(metadata: &EpisodeMetadata) -> String {
    let aired_str = metadata.aired.format("%Y-%m-%d").to_string();
    let plot = match metadata.game {
        Some(game) => format!("Playing: {}", game),
        None => "Live stream recording".to_string(),
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<episodedetails>
    <title>{}</title>
    <showtitle>{}</showtitle>
    <season>{}</season>
    <episode>{}</episode>
    <aired>{}</aired>
    <plot>{}</plot>
    <runtime>{}</runtime>
    <uniqueid type="br-daemon">{}</uniqueid>
</episodedetails>
"#,
        escape_xml(metadata.title),
        escape_xml(metadata.show_title),
        metadata.season,
        metadata.episode,
        aired_str,
        escape_xml(&plot),
        metadata.duration_minutes,
        escape_xml(metadata.recording_id)
    )
}

/** Write NFO content to a file. */
pub async fn write_nfo(path: &Path, content: &str) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(content.as_bytes()).await?;
    file.sync_all().await?;
    Ok(())
}

/** Escape XML special characters. */
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/** Capitalize the first letter of a string. */
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/** Get month name from month number (1-12). */
fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tvshow_nfo() {
        let nfo = generate_tvshow_nfo(
            "xQc",
            "twitch",
            Some("Canadian streamer"),
            Utc::now(),
        );
        assert!(nfo.contains("<title>xQc</title>"));
        assert!(nfo.contains("<plot>Canadian streamer</plot>"));
        assert!(nfo.contains("<studio>Twitch</studio>"));
    }

    #[test]
    fn test_season_nfo() {
        let nfo = generate_season_nfo(1, 2024, 1); // Season 1 = January
        assert!(nfo.contains("<seasonnumber>1</seasonnumber>"));
        assert!(nfo.contains("<title>January 2024</title>"));
    }

    #[test]
    fn test_episode_nfo() {
        let metadata = EpisodeMetadata {
            title: "Just Chatting Marathon",
            show_title: "xQc",
            season: 1,
            episode: 5,
            aired: Utc::now(),
            game: Some("Just Chatting"),
            duration_minutes: 180,
            recording_id: "abc-123",
        };
        let nfo = generate_episode_nfo(&metadata);
        assert!(nfo.contains("<title>Just Chatting Marathon</title>"));
        assert!(nfo.contains("<episode>5</episode>"));
        assert!(nfo.contains("<runtime>180</runtime>"));
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
    }
}
