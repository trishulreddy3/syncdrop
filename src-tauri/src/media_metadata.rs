//! Media Metadata & Asynchronous Checksum Module
//! Specifications: PROJECT_SPEC.md Section 13.2 & Missing Movie Workflow
//! - Asynchronous SHA-256 calculation on background worker threads (never blocks UI)
//! - Media file inspection (filename, size, container format)
//! - Detailed verification against host room movie (Verified, Mismatch, Not Loaded, Compatibility Approved)

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use tokio::task;

use crate::sync_room::{normalize_filename, MovieFileInfo};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MovieVerificationResult {
    pub status: String, // "VERIFIED" | "MISMATCH" | "NOT_LOADED" | "COMPATIBILITY_APPROVED"
    pub is_compatible: bool,
    pub title: String,
    pub details: String,
    pub hash_matched: bool,
    pub duration_matched: bool,
    pub size_matched: bool,
}

/// Inspects local movie file metadata and starts SHA-256 calculation asynchronously
pub async fn inspect_local_movie(file_path: String) -> Result<MovieFileInfo, String> {
    let mut resolved_path = PathBuf::from(&file_path);
    if !resolved_path.exists() {
        return Err(format!("Movie file does not exist: {}", file_path));
    }

    // If a folder was selected, find the primary video file inside
    if resolved_path.is_dir() {
        let video_exts = ["mp4", "mkv", "avi", "mov", "webm", "m4v", "ts", "flv", "wmv", "m2ts"];
        let mut best_candidate: Option<(PathBuf, u64)> = None;

        if let Ok(entries) = std::fs::read_dir(&resolved_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if video_exts.contains(&ext.as_str()) {
                        let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        if best_candidate.as_ref().map(|(_, l)| len > *l).unwrap_or(true) {
                            best_candidate = Some((p, len));
                        }
                    }
                }
            }
        }

        if let Some((best_path, _)) = best_candidate {
            resolved_path = best_path;
        } else {
            return Err("No compatible video files found inside the selected folder".to_string());
        }
    }

    let final_path_str = resolved_path.to_string_lossy().to_string();
    let filename = resolved_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("movie.mp4")
        .to_string();

    let file_size = std::fs::metadata(&resolved_path)
        .map(|m| m.len())
        .map_err(|e| format!("Could not read file metadata: {}", e))?;

    let container_format = resolved_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("MP4")
        .to_uppercase();

    let movie_id = format!("movie-{}", &uuid::Uuid::new_v4().to_string()[..8]);

    // Calculate SHA-256 asynchronously on a blocking worker thread pool so UI thread never freezes
    let path_buf = resolved_path.clone();
    let sha256_hash = task::spawn_blocking(move || -> Result<String, String> {
        let file = File::open(&path_buf).map_err(|e| format!("Cannot open file for hashing: {}", e))?;
        let mut reader = BufReader::with_capacity(1024 * 1024, file); // 1 MB buffer
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 1024 * 64]; // 64 KB chunk

        loop {
            let bytes_read = reader
                .read(&mut buffer)
                .map_err(|e| format!("Read error while hashing: {}", e))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash_bytes = hasher.finalize();
        Ok(format!("{:x}", hash_bytes))
    })
    .await
    .map_err(|e| format!("Background hashing task panicked: {}", e))??;

    Ok(MovieFileInfo {
        movie_id,
        filename,
        file_path: Some(final_path_str),
        file_size,
        duration: 0.0,
        container_format,
        video_codec: None,
        audio_codec: None,
        resolution: None,
        sha256: Some(sha256_hash),
    })
}

/// Evaluates movie compatibility according to Section 13.2 of PROJECT_SPEC.md
pub fn evaluate_movie_verification(
    host_file: Option<&MovieFileInfo>,
    local_file: Option<&MovieFileInfo>,
    compatibility_approved: bool,
) -> MovieVerificationResult {
    let local = match local_file {
        Some(l) => l,
        None => {
            return MovieVerificationResult {
                status: "NOT_LOADED".to_string(),
                is_compatible: false,
                title: "No Movie Loaded".to_string(),
                details: "Please select your local copy of the movie or receive it via Wi-Fi Send.".to_string(),
                hash_matched: false,
                duration_matched: false,
                size_matched: false,
            };
        }
    };

    let host = match host_file {
        Some(h) => h,
        None => {
            return MovieVerificationResult {
                status: "VERIFIED".to_string(),
                is_compatible: true,
                title: "Local Movie Ready".to_string(),
                details: format!("Loaded: {} ({:.1} MB)", local.filename, (local.file_size as f64) / (1024.0 * 1024.0)),
                hash_matched: true,
                duration_matched: true,
                size_matched: true,
            };
        }
    };

    // 1. Check SHA-256 if both have hashes
    if let (Some(ref host_hash), Some(ref local_hash)) = (&host.sha256, &local.sha256) {
        if host_hash.eq_ignore_ascii_case(local_hash) {
            return MovieVerificationResult {
                status: "VERIFIED".to_string(),
                is_compatible: true,
                title: "Verified (Exact SHA-256 Match)".to_string(),
                details: "Byte-for-byte SHA-256 file identity verified. Playback timelines are identical.".to_string(),
                hash_matched: true,
                duration_matched: true,
                size_matched: true,
            };
        } else {
            // Hash mismatch
            if compatibility_approved {
                return MovieVerificationResult {
                    status: "COMPATIBILITY_APPROVED".to_string(),
                    is_compatible: true,
                    title: "Compatibility Mode Approved".to_string(),
                    details: "Different file release/encoding approved. Note: playback synchronization may experience minor drift.".to_string(),
                    hash_matched: false,
                    duration_matched: (host.duration - local.duration).abs() <= 2.0,
                    size_matched: (host.file_size as i64 - local.file_size as i64).abs() <= 50 * 1024 * 1024,
                };
            } else {
                return MovieVerificationResult {
                    status: "MISMATCH".to_string(),
                    is_compatible: false,
                    title: "SHA-256 Hash Mismatch".to_string(),
                    details: "The selected file is not identical to the host's movie. Timelines, audio sync or scene lengths may differ.".to_string(),
                    hash_matched: false,
                    duration_matched: (host.duration - local.duration).abs() <= 2.0,
                    size_matched: (host.file_size as i64 - local.file_size as i64).abs() <= 50 * 1024 * 1024,
                };
            }
        }
    }

    // 2. Metadata Comparison (Filename, Duration, Size)
    let names_match = normalize_filename(&host.filename) == normalize_filename(&local.filename);

    let duration_diff = (host.duration - local.duration).abs();
    let duration_match = if host.duration > 0.0 && local.duration > 0.0 {
        duration_diff <= 2.0 // Tolerates container header index padding up to 2 seconds
    } else {
        true
    };

    let size_diff = (host.file_size as i64 - local.file_size as i64).abs();
    let max_allowed = ((host.file_size as f64) * 0.05) as i64;
    let size_match = size_diff <= max_allowed.max(10 * 1024 * 1024);

    if names_match && duration_match && size_match {
        MovieVerificationResult {
            status: "VERIFIED".to_string(),
            is_compatible: true,
            title: "Verified (Metadata Match)".to_string(),
            details: "Filename, duration, and file size match host movie specifications.".to_string(),
            hash_matched: false,
            duration_matched: true,
            size_matched: true,
        }
    } else if compatibility_approved {
        MovieVerificationResult {
            status: "COMPATIBILITY_APPROVED".to_string(),
            is_compatible: true,
            title: "Compatibility Mode Approved".to_string(),
            details: "Encoding/release differences approved. Caution: timeline drifts may occur.".to_string(),
            hash_matched: false,
            duration_matched: duration_match,
            size_matched: size_match,
        }
    } else {
        let reason = if !names_match {
            format!("Host is watching '{}', local is '{}'.", host.filename, local.filename)
        } else if !duration_match {
            format!("Duration mismatch: Host is {:.1}s, local is {:.1}s.", host.duration, local.duration)
        } else {
            format!("Size mismatch: Host is {} MB, local is {} MB.", host.file_size / (1024 * 1024), local.file_size / (1024 * 1024))
        };

        MovieVerificationResult {
            status: "MISMATCH".to_string(),
            is_compatible: false,
            title: "Movie Mismatch Warning".to_string(),
            details: format!("{} Incompatible files cannot silently enter synchronized playback.", reason),
            hash_matched: false,
            duration_matched: duration_match,
            size_matched: size_match,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_not_loaded() {
        let host = MovieFileInfo {
            movie_id: "m1".to_string(),
            filename: "Endgame.mkv".to_string(),
            file_path: None,
            file_size: 1_000_000,
            duration: 10800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: Some("abcdef1234567890".to_string()),
        };

        let result = evaluate_movie_verification(Some(&host), None, false);
        assert_eq!(result.status, "NOT_LOADED");
        assert!(!result.is_compatible);
    }

    #[test]
    fn test_verification_exact_sha256_match() {
        let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string();
        let host = MovieFileInfo {
            movie_id: "m1".to_string(),
            filename: "Avengers.mkv".to_string(),
            file_path: None,
            file_size: 4_000_000_000,
            duration: 10800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: Some(hash.clone()),
        };
        let local = MovieFileInfo {
            movie_id: "m2".to_string(),
            filename: "Avengers.mkv".to_string(),
            file_path: None,
            file_size: 4_000_000_000,
            duration: 10800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: Some(hash),
        };

        let result = evaluate_movie_verification(Some(&host), Some(&local), false);
        assert_eq!(result.status, "VERIFIED");
        assert!(result.is_compatible);
        assert!(result.hash_matched);
    }

    #[test]
    fn test_verification_hash_mismatch_and_compatibility_mode() {
        let host = MovieFileInfo {
            movie_id: "m1".to_string(),
            filename: "Avengers.mkv".to_string(),
            file_path: None,
            file_size: 4_000_000_000,
            duration: 10800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: Some("aaaa1111".to_string()),
        };
        let local = MovieFileInfo {
            movie_id: "m2".to_string(),
            filename: "Avengers.mkv".to_string(),
            file_path: None,
            file_size: 3_800_000_000,
            duration: 10800.0,
            container_format: "MKV".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: Some("bbbb2222".to_string()),
        };

        // Without approval: MISMATCH
        let res_denied = evaluate_movie_verification(Some(&host), Some(&local), false);
        assert_eq!(res_denied.status, "MISMATCH");
        assert!(!res_denied.is_compatible);

        // With explicit approval: COMPATIBILITY_APPROVED
        let res_approved = evaluate_movie_verification(Some(&host), Some(&local), true);
        assert_eq!(res_approved.status, "COMPATIBILITY_APPROVED");
        assert!(res_approved.is_compatible);
    }

    #[test]
    fn test_verification_duration_mismatch() {
        let host = MovieFileInfo {
            movie_id: "m1".to_string(),
            filename: "Movie.mp4".to_string(),
            file_path: None,
            file_size: 1_000_000_000,
            duration: 7200.0,
            container_format: "MP4".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: None,
        };
        let local = MovieFileInfo {
            movie_id: "m2".to_string(),
            filename: "Movie.mp4".to_string(),
            file_path: None,
            file_size: 1_000_000_000,
            duration: 5400.0, // Differ by 30 mins!
            container_format: "MP4".to_string(),
            video_codec: None,
            audio_codec: None,
            resolution: None,
            sha256: None,
        };

        let result = evaluate_movie_verification(Some(&host), Some(&local), false);
        assert_eq!(result.status, "MISMATCH");
        assert!(!result.is_compatible);
        assert!(!result.duration_matched);
    }
}
