//! Application update commands.

use anyhow::{Context, Result};
use reqwest::header::{ACCEPT, USER_AGENT};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use cratebay_core::{settings, storage};

use super::{print_structured, OutputFormat};

const DEFAULT_UPDATE_REPOSITORY: &str = "nicepkg/CrateBay";
const UPDATE_MANIFEST_ASSET: &str = "latest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum AppUpdateChannel {
    Stable,
    Prerelease,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateCheckPayload {
    configured: bool,
    available: bool,
    include_prerelease: bool,
    include_prerelease_source: String,
    current_version: String,
    version: Option<String>,
    date: Option<String>,
    body: Option<String>,
    channel: AppUpdateChannel,
    release_tag: Option<String>,
    release_name: Option<String>,
    release_url: Option<String>,
    repository: String,
    manifest_url: Option<String>,
    platform_count: Option<usize>,
    message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    prerelease: bool,
    html_url: Option<String>,
    published_at: Option<String>,
    body: Option<String>,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct UpdateManifest {
    version: Option<String>,
    notes: Option<String>,
    pub_date: Option<String>,
    platforms: Option<Value>,
}

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn release_channel(release: &GitHubRelease) -> AppUpdateChannel {
    if release.prerelease {
        AppUpdateChannel::Prerelease
    } else {
        AppUpdateChannel::Stable
    }
}

fn normalize_repository(repository: Option<&str>) -> String {
    repository
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("CRATEBAY_UPDATE_REPOSITORY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| option_env!("CRATEBAY_UPDATE_REPOSITORY").map(str::to_string))
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_UPDATE_REPOSITORY.to_string())
}

fn version_from_tag(tag_name: &str) -> String {
    tag_name.trim().trim_start_matches('v').to_string()
}

fn release_manifest_url(repository: &str, tag_name: &str) -> String {
    format!(
        "https://github.com/{}/releases/download/{}/{}",
        repository, tag_name, UPDATE_MANIFEST_ASSET,
    )
}

fn has_update_manifest(release: &GitHubRelease) -> bool {
    release
        .assets
        .iter()
        .any(|asset| asset.name == UPDATE_MANIFEST_ASSET)
}

fn parse_boolish(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn include_prerelease_from_setting(raw: Option<String>) -> bool {
    raw.as_deref()
        .and_then(parse_boolish)
        .unwrap_or(settings::DEFAULT_INCLUDE_PRERELEASES)
}

fn load_persisted_include_prerelease() -> Result<bool> {
    let db_path = storage::default_db_path()?;
    let conn = storage::init(&db_path)?;
    Ok(include_prerelease_from_setting(storage::get_setting(
        &conn,
        settings::SETTINGS_KEY_INCLUDE_PRERELEASES,
    )?))
}

fn resolve_include_prerelease(override_value: Option<bool>) -> Result<(bool, String)> {
    if let Some(value) = override_value {
        return Ok((value, "cli".to_string()));
    }

    Ok((load_persisted_include_prerelease()?, "settings".to_string()))
}

fn parse_version(value: &str) -> Option<Version> {
    Version::parse(value.trim().trim_start_matches('v')).ok()
}

fn is_newer_version(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => candidate.trim().trim_start_matches('v') != current.trim().trim_start_matches('v'),
    }
}

fn platform_count(manifest: &UpdateManifest) -> Option<usize> {
    manifest
        .platforms
        .as_ref()
        .and_then(Value::as_object)
        .map(|platforms| platforms.len())
}

async fn fetch_releases(repository: &str) -> Result<Vec<GitHubRelease>> {
    let url = format!("https://api.github.com/repos/{repository}/releases");
    let response = reqwest::Client::new()
        .get(url)
        .header(USER_AGENT, "CrateBay-CLI-Updater")
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .context("failed to request GitHub releases")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "GitHub releases request failed with status {}",
            response.status()
        );
    }

    response
        .json::<Vec<GitHubRelease>>()
        .await
        .context("failed to parse GitHub releases")
}

async fn fetch_manifest(manifest_url: &str) -> Result<UpdateManifest> {
    let response = reqwest::Client::new()
        .get(manifest_url)
        .header(USER_AGENT, "CrateBay-CLI-Updater")
        .send()
        .await
        .context("failed to request updater manifest")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Updater manifest request failed with status {}",
            response.status()
        );
    }

    response
        .json::<UpdateManifest>()
        .await
        .context("failed to parse updater manifest")
}

async fn select_release(
    repository: &str,
    include_prerelease: bool,
) -> Result<Option<GitHubRelease>> {
    let releases = fetch_releases(repository).await?;
    Ok(releases
        .into_iter()
        .filter(|release| include_prerelease || !release.prerelease)
        .find(has_update_manifest))
}

fn build_no_release_payload(
    repository: String,
    include_prerelease: bool,
    include_prerelease_source: String,
) -> AppUpdateCheckPayload {
    AppUpdateCheckPayload {
        configured: true,
        available: false,
        include_prerelease,
        include_prerelease_source,
        current_version: current_version(),
        version: None,
        date: None,
        body: None,
        channel: AppUpdateChannel::Stable,
        release_tag: None,
        release_name: None,
        release_url: None,
        repository,
        manifest_url: None,
        platform_count: None,
        message: Some("No GitHub Release with latest.json was found.".to_string()),
    }
}

fn build_release_payload(
    repository: String,
    include_prerelease: bool,
    include_prerelease_source: String,
    release: &GitHubRelease,
    manifest_url: String,
    manifest: UpdateManifest,
) -> AppUpdateCheckPayload {
    let current = current_version();
    let version = manifest
        .version
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| version_from_tag(&release.tag_name));
    let available = is_newer_version(&version, &current);
    AppUpdateCheckPayload {
        configured: true,
        available,
        include_prerelease,
        include_prerelease_source,
        current_version: current,
        version: Some(version),
        date: manifest
            .pub_date
            .clone()
            .or_else(|| release.published_at.clone()),
        body: manifest.notes.clone().or_else(|| release.body.clone()),
        channel: release_channel(release),
        release_tag: Some(release.tag_name.clone()),
        release_name: release.name.clone(),
        release_url: release.html_url.clone(),
        repository,
        manifest_url: Some(manifest_url),
        platform_count: platform_count(&manifest),
        message: (!available).then(|| "CrateBay is up to date.".to_string()),
    }
}

fn print_update_check_table(payload: &AppUpdateCheckPayload) {
    println!(
        "CrateBay update: {}",
        if payload.available {
            "available"
        } else {
            "current"
        }
    );
    println!("Current version: v{}", payload.current_version);
    if let Some(version) = &payload.version {
        println!("Latest version: v{version}");
    }
    println!("Channel: {:?}", payload.channel);
    println!(
        "Include prereleases: {} ({})",
        if payload.include_prerelease {
            "yes"
        } else {
            "no"
        },
        payload.include_prerelease_source
    );
    println!("Repository: {}", payload.repository);
    if let Some(tag) = &payload.release_tag {
        println!("Release: {tag}");
    }
    if let Some(url) = &payload.release_url {
        println!("URL: {url}");
    }
    if let Some(message) = &payload.message {
        println!("{message}");
    }
}

/// Check GitHub Releases for the same updater manifest used by the desktop UI.
pub async fn check(
    include_prerelease_override: Option<bool>,
    repository: Option<String>,
    format: &OutputFormat,
) -> Result<()> {
    let repository = normalize_repository(repository.as_deref());
    let (include_prerelease, include_prerelease_source) =
        resolve_include_prerelease(include_prerelease_override)?;
    let release = select_release(&repository, include_prerelease).await?;
    let Some(release) = release else {
        let payload =
            build_no_release_payload(repository, include_prerelease, include_prerelease_source);
        return match format {
            OutputFormat::Table => {
                print_update_check_table(&payload);
                Ok(())
            }
            _ => print_structured(&payload, format),
        };
    };

    let manifest_url = release_manifest_url(&repository, &release.tag_name);
    let manifest = fetch_manifest(&manifest_url).await?;
    let payload = build_release_payload(
        repository,
        include_prerelease,
        include_prerelease_source,
        &release,
        manifest_url,
        manifest,
    );
    match format {
        OutputFormat::Table => {
            print_update_check_table(&payload);
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_version_comparison_handles_v_prefix_and_prerelease() {
        assert!(is_newer_version("v0.9.1", "0.9.0"));
        assert!(is_newer_version("1.0.0-beta.1", "0.9.0"));
        assert!(!is_newer_version("v0.9.0", "0.9.0"));
        assert!(!is_newer_version("0.8.9", "0.9.0"));
    }

    #[test]
    fn update_repository_normalization_keeps_owner_repo_shape() {
        assert_eq!(
            normalize_repository(Some(" /nicepkg/CrateBay/ ")),
            "nicepkg/CrateBay"
        );
        assert_eq!(
            normalize_repository(Some("nicepkg/CrateBay")),
            "nicepkg/CrateBay"
        );
    }

    #[test]
    fn update_include_prerelease_setting_matches_desktop_storage_values() {
        assert!(include_prerelease_from_setting(Some("true".to_string())));
        assert!(include_prerelease_from_setting(Some("1".to_string())));
        assert!(!include_prerelease_from_setting(Some("false".to_string())));
        assert!(!include_prerelease_from_setting(Some(
            "not-a-bool".to_string()
        )));
        assert!(!include_prerelease_from_setting(None));
    }

    #[test]
    fn update_release_payload_uses_manifest_metadata() {
        let release = GitHubRelease {
            tag_name: "v9.9.9".to_string(),
            name: Some("CrateBay v9.9.9".to_string()),
            prerelease: false,
            html_url: Some("https://github.com/nicepkg/CrateBay/releases/tag/v9.9.9".to_string()),
            published_at: Some("2026-01-01T00:00:00Z".to_string()),
            body: Some("release body".to_string()),
            assets: vec![GitHubReleaseAsset {
                name: UPDATE_MANIFEST_ASSET.to_string(),
            }],
        };
        let manifest = UpdateManifest {
            version: Some("9.9.9".to_string()),
            notes: Some("manifest notes".to_string()),
            pub_date: Some("2026-01-02T00:00:00Z".to_string()),
            platforms: Some(serde_json::json!({
                "darwin-aarch64": {},
                "darwin-x86_64": {}
            })),
        };

        let payload = build_release_payload(
            "nicepkg/CrateBay".to_string(),
            true,
            "settings".to_string(),
            &release,
            release_manifest_url("nicepkg/CrateBay", &release.tag_name),
            manifest,
        );

        assert!(payload.available);
        assert_eq!(payload.version.as_deref(), Some("9.9.9"));
        assert_eq!(payload.body.as_deref(), Some("manifest notes"));
        assert_eq!(payload.platform_count, Some(2));
        assert!(payload.include_prerelease);
        assert_eq!(payload.include_prerelease_source, "settings");
    }
}
