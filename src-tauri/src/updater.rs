use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_name: String,
    pub release_notes: String,
    pub download_url: Option<String>,
    pub file_size: Option<u64>,
    pub release_page_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub status: String, // "downloading" | "installing" | "completed" | "error"
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

/// Helper to compare semver-like version strings
pub fn is_newer_version(remote: &str, current: &str) -> bool {
    let clean_remote = remote.trim().trim_start_matches('v');
    let clean_current = current.trim().trim_start_matches('v');

    let remote_parts: Vec<u64> = clean_remote
        .split('.')
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    let current_parts: Vec<u64> = clean_current
        .split('.')
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();

    for i in 0..std::cmp::max(remote_parts.len(), current_parts.len()) {
        let r = remote_parts.get(i).copied().unwrap_or(0);
        let c = current_parts.get(i).copied().unwrap_or(0);
        if r > c {
            return true;
        }
        if r < c {
            return false;
        }
    }

    false
}

/// Query GitHub Releases API for latest SyncDrop version
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let client = reqwest::Client::builder()
        .user_agent("SyncDrop-Desktop-App")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = "https://api.github.com/repos/trishulreddy3/syncdrop/releases/latest";
    let resp = match client
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(format!(
                "Unable to connect to update server: {}. Check your internet connection.",
                e
            ));
        }
    };

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateInfo {
            available: false,
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            release_name: "No published release yet".to_string(),
            release_notes: String::new(),
            download_url: None,
            file_size: None,
            release_page_url: "https://github.com/trishulreddy3/syncdrop/releases".to_string(),
        });
    }

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned status: {}", resp.status()));
    }

    let release: GithubRelease = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse release metadata: {}", e))?;

    let remote_version = release.tag_name.trim().trim_start_matches('v').to_string();
    let has_update = is_newer_version(&remote_version, current_version);

    // Look for Windows installer asset
    let exe_asset = release.assets.iter().find(|a| {
        let lower = a.name.to_lowercase();
        lower.ends_with(".exe")
    });

    Ok(UpdateInfo {
        available: has_update,
        current_version: current_version.to_string(),
        latest_version: remote_version,
        release_name: release.name.unwrap_or_else(|| release.tag_name.clone()),
        release_notes: release.body.unwrap_or_default(),
        download_url: exe_asset.map(|a| a.browser_download_url.clone()),
        file_size: exe_asset.map(|a| a.size),
        release_page_url: release.html_url,
    })
}

/// Download latest setup exe and launch it
pub async fn download_and_install_update(
    app: AppHandle,
    download_url: String,
) -> Result<PathBuf, String> {
    let client = reqwest::Client::builder()
        .user_agent("SyncDrop-Desktop-App")
        .build()
        .map_err(|e| format!("Failed to initialize download client: {}", e))?;

    let resp = match client.get(&download_url).send().await {
        Ok(r) => r,
        Err(e) => {
            let err_msg = format!("Download failed: {}", e);
            let _ = app.emit(
                "syncdrop://update-progress",
                UpdateProgress {
                    bytes_downloaded: 0,
                    total_bytes: 0,
                    percentage: 0.0,
                    status: "error".to_string(),
                    error_message: Some(err_msg.clone()),
                },
            );
            return Err(err_msg);
        }
    };

    if !resp.status().is_success() {
        let err_msg = format!("Server returned error {}: {}", resp.status(), download_url);
        let _ = app.emit(
            "syncdrop://update-progress",
            UpdateProgress {
                bytes_downloaded: 0,
                total_bytes: 0,
                percentage: 0.0,
                status: "error".to_string(),
                error_message: Some(err_msg.clone()),
            },
        );
        return Err(err_msg);
    }

    let total_bytes = resp.content_length().unwrap_or(0);
    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("SyncDrop_Update_Setup.exe");

    let mut file = tokio::fs::File::create(&installer_path)
        .await
        .map_err(|e| format!("Failed to create temporary installer file: {}", e))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_result) = futures_util::StreamExt::next(&mut stream).await {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Error during download stream: {}", e);
                let _ = app.emit(
                    "syncdrop://update-progress",
                    UpdateProgress {
                        bytes_downloaded: downloaded,
                        total_bytes,
                        percentage: 0.0,
                        status: "error".to_string(),
                        error_message: Some(err_msg.clone()),
                    },
                );
                return Err(err_msg);
            }
        };

        if let Err(e) = file.write_all(&chunk).await {
            let err_msg = format!("Failed to write to installer file: {}", e);
            return Err(err_msg);
        }

        downloaded += chunk.len() as u64;

        if last_emit.elapsed() > std::time::Duration::from_millis(150) || (total_bytes > 0 && downloaded >= total_bytes) {
            let pct = if total_bytes > 0 {
                ((downloaded as f64 / total_bytes as f64) * 100.0).min(100.0)
            } else {
                0.0
            };
            let _ = app.emit(
                "syncdrop://update-progress",
                UpdateProgress {
                    bytes_downloaded: downloaded,
                    total_bytes,
                    percentage: pct,
                    status: "downloading".to_string(),
                    error_message: None,
                },
            );
            last_emit = std::time::Instant::now();
        }
    }

    if let Err(e) = file.flush().await {
        return Err(format!("Failed to finalize installer file: {}", e));
    }
    drop(file);

    let _ = app.emit(
        "syncdrop://update-progress",
        UpdateProgress {
            bytes_downloaded: downloaded,
            total_bytes: downloaded,
            percentage: 100.0,
            status: "installing".to_string(),
            error_message: None,
        },
    );

    // Launch the downloaded NSIS installer
    let mut cmd = std::process::Command::new(&installer_path);
    if let Err(e) = cmd.spawn() {
        let err_msg = format!("Downloaded update but failed to launch installer: {}", e);
        let _ = app.emit(
            "syncdrop://update-progress",
            UpdateProgress {
                bytes_downloaded: downloaded,
                total_bytes: downloaded,
                percentage: 100.0,
                status: "error".to_string(),
                error_message: Some(err_msg.clone()),
            },
        );
        return Err(err_msg);
    }

    let _ = app.emit(
        "syncdrop://update-progress",
        UpdateProgress {
            bytes_downloaded: downloaded,
            total_bytes: downloaded,
            percentage: 100.0,
            status: "completed".to_string(),
            error_message: None,
        },
    );

    Ok(installer_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
        assert!(is_newer_version("0.1.0.1", "0.1.0"));
    }
}

