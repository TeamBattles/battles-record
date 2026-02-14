pub mod aliases;

use std::path::{Path, PathBuf};

use aliases::{AliasError, AliasMap};
use tracing::info;

#[derive(thiserror::Error, Debug)]
pub enum MergeError {
    #[error("Source channel not found: {0}")]
    SourceNotFound(String),
    #[error("Cannot merge: active downloads for {0}")]
    ActiveDownloads(String),
    #[error("Alias error: {0}")]
    Alias(#[from] AliasError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Merge downloads from source to target within a platform folder.
/// Moves files from {downloads_dir}/{platform}/{source}/ to {downloads_dir}/{platform}/{target}/
pub async fn merge_downloads(
    downloads_dir: &Path,
    platform: &str,
    source: &str,
    target: &str,
    aliases: &mut AliasMap,
    alias_path: &Path,
) -> Result<u64, MergeError> {
    let source_dir = downloads_dir.join(platform).join(source);
    let target_dir = downloads_dir.join(platform).join(target);

    if !source_dir.exists() {
        return Err(MergeError::SourceNotFound(format!(
            "{}/{}",
            platform, source
        )));
    }

    tokio::fs::create_dir_all(&target_dir).await?;

    let moved = move_directory_contents(&source_dir, &target_dir).await?;

    // Remove empty source dir
    if dir_is_empty(&source_dir).await {
        let _ = tokio::fs::remove_dir(&source_dir).await;
    }

    // Create alias so future downloads for the old name go to the new name
    let key = format!("{}/{}", platform, source);
    aliases.add_download_alias(&key, target)?;
    aliases.update_chains(source, target);
    aliases.save(alias_path)?;

    info!(platform, source, target, files = moved, "Merged downloads");
    Ok(moved)
}

/// Move all files from source_dir to target_dir.
/// Handles cross-filesystem moves with copy-then-delete fallback.
async fn move_directory_contents(source: &Path, target: &Path) -> Result<u64, std::io::Error> {
    let mut count = 0u64;
    let mut entries = tokio::fs::read_dir(source).await?;

    while let Some(entry) = entries.next_entry().await? {
        let source_path = entry.path();
        let file_name = entry.file_name();
        let mut target_path = target.join(&file_name);

        // Handle collision: append numeric suffix
        if target_path.exists() {
            target_path = find_unique_path(&target_path);
        }

        if source_path.is_dir() {
            tokio::fs::create_dir_all(&target_path).await?;
            let sub_count =
                Box::pin(move_directory_contents(&source_path, &target_path)).await?;
            count += sub_count;
            if dir_is_empty(&source_path).await {
                let _ = tokio::fs::remove_dir(&source_path).await;
            }
        } else {
            // Try rename first (same filesystem), fall back to copy+delete
            match tokio::fs::rename(&source_path, &target_path).await {
                Ok(_) => {}
                Err(_) => {
                    tokio::fs::copy(&source_path, &target_path).await?;
                    // Verify size matches before deleting source
                    let src_meta = tokio::fs::metadata(&source_path).await?;
                    let dst_meta = tokio::fs::metadata(&target_path).await?;
                    if src_meta.len() == dst_meta.len() {
                        tokio::fs::remove_file(&source_path).await?;
                    }
                }
            }
            count += 1;
        }
    }

    Ok(count)
}

/// Find a unique path by appending (2), (3), etc.
fn find_unique_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let parent = path.parent().unwrap_or(path);

    for i in 2..1000 {
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, i)
        } else {
            format!("{} ({}).{}", stem, i, ext)
        };
        let candidate = parent.join(new_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{}_conflict", stem))
}

async fn dir_is_empty(path: &Path) -> bool {
    match tokio::fs::read_dir(path).await {
        Ok(mut entries) => entries
            .next_entry()
            .await
            .map_or(true, |e| e.is_none()),
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn find_unique_path_appends_suffix() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("video.mp4");
        std::fs::write(&original, "data").unwrap();

        let unique = find_unique_path(&original);
        assert_eq!(
            unique.file_name().and_then(|n| n.to_str()),
            Some("video (2).mp4")
        );
    }

    #[test]
    fn find_unique_path_increments_when_multiple_collisions() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("video.mp4");
        std::fs::write(&original, "data").unwrap();
        std::fs::write(dir.path().join("video (2).mp4"), "data").unwrap();
        std::fs::write(dir.path().join("video (3).mp4"), "data").unwrap();

        let unique = find_unique_path(&original);
        assert_eq!(
            unique.file_name().and_then(|n| n.to_str()),
            Some("video (4).mp4")
        );
    }

    #[test]
    fn find_unique_path_handles_no_extension() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("readme");
        std::fs::write(&original, "data").unwrap();

        let unique = find_unique_path(&original);
        assert_eq!(
            unique.file_name().and_then(|n| n.to_str()),
            Some("readme (2)")
        );
    }

    #[test]
    fn find_unique_path_returns_original_stem_when_no_collision() {
        let dir = TempDir::new().unwrap();
        // The file doesn't exist, so find_unique_path should still return (2)
        // because the function is only called when the original path already exists
        let original = dir.path().join("video.mp4");
        // Not writing the file - find_unique_path checks existence
        let unique = find_unique_path(&original);
        // Since "video.mp4" doesn't exist, (2) won't exist either, returns (2)
        assert_eq!(
            unique.file_name().and_then(|n| n.to_str()),
            Some("video (2).mp4")
        );
    }

    #[tokio::test]
    async fn move_directory_contents_moves_files() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        tokio::fs::create_dir_all(&src).await.unwrap();
        tokio::fs::create_dir_all(&dst).await.unwrap();

        tokio::fs::write(src.join("a.txt"), "hello").await.unwrap();
        tokio::fs::write(src.join("b.txt"), "world").await.unwrap();

        let count = move_directory_contents(&src, &dst).await.unwrap();
        assert_eq!(count, 2);
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("b.txt").exists());
        // Source files should be gone (moved)
        assert!(!src.join("a.txt").exists());
        assert!(!src.join("b.txt").exists());
    }

    #[tokio::test]
    async fn move_directory_contents_handles_collisions() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        tokio::fs::create_dir_all(&src).await.unwrap();
        tokio::fs::create_dir_all(&dst).await.unwrap();

        // Both dirs have a file with the same name
        tokio::fs::write(src.join("video.mp4"), "new content")
            .await
            .unwrap();
        tokio::fs::write(dst.join("video.mp4"), "old content")
            .await
            .unwrap();

        let count = move_directory_contents(&src, &dst).await.unwrap();
        assert_eq!(count, 1);
        // Original should still be there
        assert!(dst.join("video.mp4").exists());
        // Collision file should have suffix
        assert!(dst.join("video (2).mp4").exists());
    }

    #[tokio::test]
    async fn move_directory_contents_recurses_subdirs() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        let sub = src.join("subdir");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        tokio::fs::create_dir_all(&dst).await.unwrap();

        tokio::fs::write(sub.join("nested.txt"), "nested")
            .await
            .unwrap();

        let count = move_directory_contents(&src, &dst).await.unwrap();
        assert_eq!(count, 1);
        assert!(dst.join("subdir").join("nested.txt").exists());
    }

    #[tokio::test]
    async fn merge_downloads_creates_alias_and_moves_files() {
        let dir = TempDir::new().unwrap();
        let downloads = dir.path().join("downloads");
        let alias_path = dir.path().join("aliases.json");

        // Set up source channel directory with a file
        let src_dir = downloads.join("youtube").join("fivee");
        tokio::fs::create_dir_all(&src_dir).await.unwrap();
        tokio::fs::write(src_dir.join("stream.mp4"), "video data")
            .await
            .unwrap();

        let mut aliases = AliasMap::default();

        let moved = merge_downloads(
            &downloads,
            "youtube",
            "fivee",
            "five",
            &mut aliases,
            &alias_path,
        )
        .await
        .unwrap();

        assert_eq!(moved, 1);
        // Target should have the file
        assert!(downloads.join("youtube").join("five").join("stream.mp4").exists());
        // Source dir should be removed (was empty after move)
        assert!(!src_dir.exists());
        // Alias should be created
        assert_eq!(aliases.resolve_download("youtube", "fivee"), "five");
        // Alias file should be persisted
        let loaded = AliasMap::load(&alias_path);
        assert_eq!(loaded.resolve_download("youtube", "fivee"), "five");
    }

    #[tokio::test]
    async fn merge_downloads_fails_for_missing_source() {
        let dir = TempDir::new().unwrap();
        let downloads = dir.path().join("downloads");
        let alias_path = dir.path().join("aliases.json");
        tokio::fs::create_dir_all(&downloads).await.unwrap();

        let mut aliases = AliasMap::default();

        let result = merge_downloads(
            &downloads,
            "youtube",
            "nonexistent",
            "target",
            &mut aliases,
            &alias_path,
        )
        .await;

        assert!(matches!(result, Err(MergeError::SourceNotFound(_))));
    }

    #[tokio::test]
    async fn dir_is_empty_true_for_empty_dir() {
        let dir = TempDir::new().unwrap();
        assert!(dir_is_empty(dir.path()).await);
    }

    #[tokio::test]
    async fn dir_is_empty_false_for_nonempty_dir() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file.txt"), "content").unwrap();
        assert!(!dir_is_empty(dir.path()).await);
    }

    #[tokio::test]
    async fn dir_is_empty_true_for_nonexistent_path() {
        let dir = TempDir::new().unwrap();
        let nonexistent = dir.path().join("nope");
        assert!(dir_is_empty(&nonexistent).await);
    }
}
