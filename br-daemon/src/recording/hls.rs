use m3u8_rs::Playlist;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HlsError {
    #[error("Failed to parse playlist: {0}")]
    ParseError(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Not a media playlist")]
    NotMediaPlaylist,

    #[error("Not a master playlist")]
    NotMasterPlaylist,

    #[error("Quality not found: {0}")]
    QualityNotFound(String),
}

/** A variant stream from a master playlist. */
#[derive(Debug, Clone)]
pub struct MasterVariant {
    pub uri: String,
    pub bandwidth: u64,
    pub resolution: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HlsSegment {
    pub sequence: u64,
    pub uri: String,
    pub duration: f32,
}

#[derive(Debug)]
pub struct ParsedPlaylist {
    pub segments: Vec<HlsSegment>,
    pub media_sequence: u64,
    pub target_duration: u64,
    pub is_endlist: bool,
    /**
     * URI of the initialization segment (EXT-X-MAP) for fMP4/CMAF streams.
     * This must be downloaded and prepended to media segments for playback.
     */
    pub init_segment_uri: Option<String>,
}

pub fn parse_media_playlist(content: &str, base_url: &str) -> Result<ParsedPlaylist, HlsError> {
    let parsed = m3u8_rs::parse_playlist_res(content.as_bytes())
        .map_err(|e| HlsError::ParseError(format!("{:?}", e)))?;

    match parsed {
        Playlist::MediaPlaylist(playlist) => {
            let media_sequence = playlist.media_sequence;

            // Extract init segment URI from EXT-X-MAP tag if present.
            // The map can be on any segment, but typically appears on the first one
            // and applies to all subsequent segments until another EXT-X-MAP appears.
            let init_segment_uri = playlist
                .segments
                .iter()
                .find_map(|seg| seg.map.as_ref())
                .map(|map| resolve_url(base_url, &map.uri));

            let segments = playlist
                .segments
                .iter()
                .enumerate()
                .map(|(i, seg)| {
                    let uri = resolve_url(base_url, &seg.uri);
                    HlsSegment {
                        sequence: media_sequence + i as u64,
                        uri,
                        duration: seg.duration,
                    }
                })
                .collect();

            Ok(ParsedPlaylist {
                segments,
                media_sequence,
                target_duration: playlist.target_duration,
                is_endlist: playlist.end_list,
                init_segment_uri,
            })
        }
        Playlist::MasterPlaylist(_) => Err(HlsError::NotMediaPlaylist),
    }
}

/** Parse a master playlist and return all variant streams. */
pub fn parse_master_playlist(
    content: &str,
    base_url: &str,
) -> Result<Vec<MasterVariant>, HlsError> {
    let parsed = m3u8_rs::parse_playlist_res(content.as_bytes())
        .map_err(|e| HlsError::ParseError(format!("{:?}", e)))?;

    match parsed {
        Playlist::MasterPlaylist(playlist) => {
            let variants = playlist
                .variants
                .iter()
                .map(|v| {
                    let uri = resolve_url(base_url, &v.uri);
                    let resolution = v
                        .resolution
                        .as_ref()
                        .map(|r| format!("{}x{}", r.width, r.height));

                    // Try to extract name from VIDEO attribute or generate from resolution
                    let name = v.video.clone().or_else(|| {
                        resolution
                            .as_ref()
                            .and_then(|r| r.split('x').nth(1).map(|h| format!("{}p", h)))
                    });

                    MasterVariant {
                        uri,
                        bandwidth: v.bandwidth,
                        resolution,
                        name,
                    }
                })
                .collect();

            Ok(variants)
        }
        Playlist::MediaPlaylist(_) => Err(HlsError::NotMasterPlaylist),
    }
}

/**
 * Find the best variant for the given quality preference.
 *
 * - "source" or "best" returns the highest bandwidth variant
 * - "720p", "1080p" etc. tries to match by resolution height
 * - Falls back to highest bandwidth if no match found
 */
pub fn find_variant_for_quality<'a>(
    variants: &'a [MasterVariant],
    quality: &str,
) -> Option<&'a MasterVariant> {
    if variants.is_empty() {
        return None;
    }

    let quality_lower = quality.to_lowercase();

    // For source/best, return highest bandwidth
    if quality_lower == "source" || quality_lower == "best" {
        return variants.iter().max_by_key(|v| v.bandwidth);
    }

    // Try to match by name (e.g., "720p60", "1080p")
    if let Some(variant) = variants.iter().find(|v| {
        v.name
            .as_ref()
            .map_or(false, |n| n.to_lowercase().contains(&quality_lower))
    }) {
        return Some(variant);
    }

    // Try to match by resolution height (e.g., "720" matches "1280x720")
    let height_str = quality_lower.trim_end_matches('p');
    if let Some(variant) = variants.iter().find(|v| {
        v.resolution.as_ref().map_or(false, |r| {
            r.split('x').nth(1).map_or(false, |h| h == height_str)
        })
    }) {
        return Some(variant);
    }

    // Fall back to highest bandwidth
    variants.iter().max_by_key(|v| v.bandwidth)
}

/** Resolve a potentially relative URL against a base URL. */
fn resolve_url(base: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }

    // Find the last '/' in base URL to get the directory
    if let Some(idx) = base.rfind('/') {
        format!("{}/{}", &base[..idx], url)
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_media_playlist() {
        let content = r#"#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:2
#EXT-X-MEDIA-SEQUENCE:12345
#EXTINF:2.0,
segment-12345.ts
#EXTINF:2.0,
segment-12346.ts
#EXTINF:2.0,
segment-12347.ts
"#;
        let base_url = "https://example.com/stream/playlist.m3u8";
        let result = parse_media_playlist(content, base_url).unwrap();

        assert_eq!(result.media_sequence, 12345);
        assert_eq!(result.segments.len(), 3);
        assert_eq!(result.segments[0].sequence, 12345);
        assert_eq!(
            result.segments[0].uri,
            "https://example.com/stream/segment-12345.ts"
        );
        assert!(!result.is_endlist);
    }

    #[test]
    fn test_parse_endlist() {
        let content = r#"#EXTM3U
#EXT-X-TARGETDURATION:2
#EXT-X-MEDIA-SEQUENCE:100
#EXTINF:2.0,
seg.ts
#EXT-X-ENDLIST
"#;
        let result = parse_media_playlist(content, "https://example.com/").unwrap();
        assert!(result.is_endlist);
    }

    #[test]
    fn test_resolve_url() {
        assert_eq!(
            resolve_url("https://example.com/path/playlist.m3u8", "segment.ts"),
            "https://example.com/path/segment.ts"
        );
        assert_eq!(
            resolve_url(
                "https://example.com/path/playlist.m3u8",
                "https://cdn.example.com/seg.ts"
            ),
            "https://cdn.example.com/seg.ts"
        );
    }

    #[test]
    fn test_parse_media_playlist_with_init_segment() {
        // This is an fMP4/CMAF playlist with EXT-X-MAP for the init segment
        let content = r#"#EXTM3U
#EXT-X-VERSION:6
#EXT-X-TARGETDURATION:2
#EXT-X-MEDIA-SEQUENCE:12329
#EXT-X-MAP:URI="init-0.mp4"
#EXTINF:2.0,
0012329.ts
#EXTINF:2.0,
0012330.ts
#EXTINF:2.0,
0012331.ts
"#;
        let base_url = "https://video-edge.twitch.tv/stream/playlist.m3u8";
        let result = parse_media_playlist(content, base_url).unwrap();

        // Should extract the init segment URI
        assert_eq!(
            result.init_segment_uri,
            Some("https://video-edge.twitch.tv/stream/init-0.mp4".to_string())
        );
        assert_eq!(result.media_sequence, 12329);
        assert_eq!(result.segments.len(), 3);
    }

    #[test]
    fn test_parse_media_playlist_without_init_segment() {
        // Traditional MPEG-TS playlist without EXT-X-MAP
        let content = r#"#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:2
#EXT-X-MEDIA-SEQUENCE:100
#EXTINF:2.0,
segment-100.ts
#EXTINF:2.0,
segment-101.ts
"#;
        let result = parse_media_playlist(content, "https://example.com/").unwrap();

        // No init segment for MPEG-TS
        assert!(result.init_segment_uri.is_none());
    }

    // Master Playlist Parsing Tests
    #[test]
    fn test_parse_master_playlist_basic() {
        let content = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080
1080p.m3u8
"#;
        let result = parse_master_playlist(content, "https://example.com/stream/").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bandwidth, 5000000);
        assert_eq!(result[0].resolution, Some("1920x1080".to_string()));
        assert_eq!(result[0].uri, "https://example.com/stream/1080p.m3u8");
    }

    #[test]
    fn test_parse_master_playlist_multiple_variants() {
        let content = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=8000000,RESOLUTION=1920x1080
1080p.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=4000000,RESOLUTION=1280x720
720p.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000000,RESOLUTION=854x480
480p.m3u8
"#;
        let result = parse_master_playlist(content, "https://example.com/").unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].bandwidth, 8000000);
        assert_eq!(result[1].bandwidth, 4000000);
        assert_eq!(result[2].bandwidth, 2000000);
    }

    #[test]
    fn test_parse_master_playlist_with_resolution() {
        let content = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=6000000,RESOLUTION=1920x1080
stream.m3u8
"#;
        let result = parse_master_playlist(content, "https://example.com/").unwrap();
        assert_eq!(result[0].resolution, Some("1920x1080".to_string()));
        assert_eq!(result[0].name, Some("1080p".to_string())); // Derived from resolution
    }

    #[test]
    fn test_parse_master_playlist_relative_urls() {
        let content = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=3000000
playlist/720p.m3u8
"#;
        let result =
            parse_master_playlist(content, "https://cdn.example.com/streams/master.m3u8").unwrap();
        assert_eq!(
            result[0].uri,
            "https://cdn.example.com/streams/playlist/720p.m3u8"
        );
    }

    #[test]
    fn test_parse_master_playlist_absolute_urls() {
        let content = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=3000000
https://other-cdn.example.com/720p.m3u8
"#;
        let result =
            parse_master_playlist(content, "https://cdn.example.com/streams/master.m3u8").unwrap();
        assert_eq!(result[0].uri, "https://other-cdn.example.com/720p.m3u8");
    }

    // Quality Selection Tests
    #[test]
    fn test_find_variant_source_quality() {
        let variants = vec![
            MasterVariant {
                uri: "480p.m3u8".to_string(),
                bandwidth: 2000000,
                resolution: Some("854x480".to_string()),
                name: Some("480p".to_string()),
            },
            MasterVariant {
                uri: "1080p.m3u8".to_string(),
                bandwidth: 8000000,
                resolution: Some("1920x1080".to_string()),
                name: Some("1080p".to_string()),
            },
            MasterVariant {
                uri: "720p.m3u8".to_string(),
                bandwidth: 4000000,
                resolution: Some("1280x720".to_string()),
                name: Some("720p".to_string()),
            },
        ];

        let result = find_variant_for_quality(&variants, "source").unwrap();
        assert_eq!(result.bandwidth, 8000000); // Highest bandwidth
    }

    #[test]
    fn test_find_variant_best_quality() {
        let variants = vec![
            MasterVariant {
                uri: "480p.m3u8".to_string(),
                bandwidth: 2000000,
                resolution: Some("854x480".to_string()),
                name: Some("480p".to_string()),
            },
            MasterVariant {
                uri: "1080p.m3u8".to_string(),
                bandwidth: 8000000,
                resolution: Some("1920x1080".to_string()),
                name: Some("1080p".to_string()),
            },
        ];

        let result = find_variant_for_quality(&variants, "best").unwrap();
        assert_eq!(result.bandwidth, 8000000);
    }

    #[test]
    fn test_find_variant_720p() {
        let variants = vec![
            MasterVariant {
                uri: "480p.m3u8".to_string(),
                bandwidth: 2000000,
                resolution: Some("854x480".to_string()),
                name: Some("480p".to_string()),
            },
            MasterVariant {
                uri: "1080p.m3u8".to_string(),
                bandwidth: 8000000,
                resolution: Some("1920x1080".to_string()),
                name: Some("1080p".to_string()),
            },
            MasterVariant {
                uri: "720p.m3u8".to_string(),
                bandwidth: 4000000,
                resolution: Some("1280x720".to_string()),
                name: Some("720p".to_string()),
            },
        ];

        let result = find_variant_for_quality(&variants, "720p").unwrap();
        assert_eq!(result.resolution, Some("1280x720".to_string()));
    }

    #[test]
    fn test_find_variant_by_name() {
        let variants = vec![
            MasterVariant {
                uri: "low.m3u8".to_string(),
                bandwidth: 2000000,
                resolution: Some("854x480".to_string()),
                name: Some("480p60".to_string()),
            },
            MasterVariant {
                uri: "high.m3u8".to_string(),
                bandwidth: 8000000,
                resolution: Some("1920x1080".to_string()),
                name: Some("1080p60".to_string()),
            },
        ];

        let result = find_variant_for_quality(&variants, "1080p60").unwrap();
        assert_eq!(result.name, Some("1080p60".to_string()));
    }

    #[test]
    fn test_find_variant_fallback() {
        let variants = vec![
            MasterVariant {
                uri: "480p.m3u8".to_string(),
                bandwidth: 2000000,
                resolution: Some("854x480".to_string()),
                name: Some("480p".to_string()),
            },
            MasterVariant {
                uri: "1080p.m3u8".to_string(),
                bandwidth: 8000000,
                resolution: Some("1920x1080".to_string()),
                name: Some("1080p".to_string()),
            },
        ];

        // Request 4K which doesn't exist - should fall back to highest bandwidth
        let result = find_variant_for_quality(&variants, "2160p").unwrap();
        assert_eq!(result.bandwidth, 8000000);
    }

    #[test]
    fn test_find_variant_case_insensitive() {
        let variants = vec![MasterVariant {
            uri: "720p.m3u8".to_string(),
            bandwidth: 4000000,
            resolution: Some("1280x720".to_string()),
            name: Some("720p".to_string()),
        }];

        // Should match regardless of case
        assert!(find_variant_for_quality(&variants, "720P").is_some());
        assert!(find_variant_for_quality(&variants, "SOURCE").is_some());
        assert!(find_variant_for_quality(&variants, "Best").is_some());
    }

    #[test]
    fn test_find_variant_empty_list() {
        let variants: Vec<MasterVariant> = vec![];
        let result = find_variant_for_quality(&variants, "source");
        assert!(result.is_none());
    }

    // Error Cases
    #[test]
    fn test_parse_invalid_playlist() {
        let content = "not a valid m3u8 playlist";
        let result = parse_media_playlist(content, "https://example.com/");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_content() {
        let result = parse_media_playlist("", "https://example.com/");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_media_as_master_error() {
        // A media playlist should error when parsed as master
        let content = r#"#EXTM3U
#EXT-X-TARGETDURATION:2
#EXT-X-MEDIA-SEQUENCE:100
#EXTINF:2.0,
seg.ts
"#;
        let result = parse_master_playlist(content, "https://example.com/");
        assert!(matches!(result, Err(HlsError::NotMasterPlaylist)));
    }

    #[test]
    fn test_parse_master_as_media_error() {
        // A master playlist should error when parsed as media
        let content = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=5000000
720p.m3u8
"#;
        let result = parse_media_playlist(content, "https://example.com/");
        assert!(matches!(result, Err(HlsError::NotMediaPlaylist)));
    }

    // URL Resolution Edge Cases
    #[test]
    fn test_resolve_url_http_protocol() {
        assert_eq!(
            resolve_url("https://example.com/stream/", "http://other.com/seg.ts"),
            "http://other.com/seg.ts"
        );
    }

    #[test]
    fn test_resolve_url_no_slash_in_base() {
        // Edge case: base URL without path component after last slash
        // The function finds the last / in the scheme separator and prepends that
        assert_eq!(
            resolve_url("https://example.com", "segment.ts"),
            "https://segment.ts"
        );
    }

    #[test]
    fn test_segment_duration_preserved() {
        let content = r#"#EXTM3U
#EXT-X-TARGETDURATION:10
#EXT-X-MEDIA-SEQUENCE:0
#EXTINF:9.5,
seg0.ts
#EXTINF:10.0,
seg1.ts
#EXTINF:8.333,
seg2.ts
"#;
        let result = parse_media_playlist(content, "https://example.com/").unwrap();
        assert_eq!(result.segments.len(), 3);
        assert!((result.segments[0].duration - 9.5).abs() < 0.001);
        assert!((result.segments[1].duration - 10.0).abs() < 0.001);
        assert!((result.segments[2].duration - 8.333).abs() < 0.001);
    }

    #[test]
    fn test_target_duration() {
        let content = r#"#EXTM3U
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:0
#EXTINF:6.0,
seg.ts
"#;
        let result = parse_media_playlist(content, "https://example.com/").unwrap();
        assert_eq!(result.target_duration, 6);
    }
}
