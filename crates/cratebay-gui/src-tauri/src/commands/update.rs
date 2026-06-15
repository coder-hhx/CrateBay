use reqwest::header::{ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Url};
use tauri_plugin_updater::UpdaterExt;

const DEFAULT_UPDATE_REPOSITORY: &str = "nicepkg/CrateBay";
const UPDATE_MANIFEST_ASSET: &str = "latest.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCheckResponse {
    configured: bool,
    available: bool,
    current_version: String,
    version: Option<String>,
    date: Option<String>,
    body: Option<String>,
    channel: AppUpdateChannel,
    release_tag: Option<String>,
    release_name: Option<String>,
    release_url: Option<String>,
    repository: String,
    message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum AppUpdateChannel {
    Stable,
    Prerelease,
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

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn update_repository() -> String {
    std::env::var("CRATEBAY_UPDATE_REPOSITORY")
        .ok()
        .or_else(|| option_env!("CRATEBAY_UPDATE_REPOSITORY").map(str::to_string))
        .map(|value| value.trim().trim_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_UPDATE_REPOSITORY.to_string())
}

fn updater_public_key_override() -> Option<String> {
    std::env::var("CRATEBAY_UPDATER_PUBLIC_KEY")
        .ok()
        .or_else(|| option_env!("CRATEBAY_UPDATER_PUBLIC_KEY").map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn release_channel(release: &GitHubRelease) -> AppUpdateChannel {
    if release.prerelease {
        AppUpdateChannel::Prerelease
    } else {
        AppUpdateChannel::Stable
    }
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

async fn fetch_releases(repository: &str) -> Result<Vec<GitHubRelease>, String> {
    let url = format!("https://api.github.com/repos/{repository}/releases");
    let response = reqwest::Client::new()
        .get(url)
        .header(USER_AGENT, "CrateBay-Updater")
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("failed to request GitHub releases: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub releases request failed with status {}",
            response.status()
        ));
    }

    response
        .json::<Vec<GitHubRelease>>()
        .await
        .map_err(|error| format!("failed to parse GitHub releases: {error}"))
}

async fn select_release(
    repository: &str,
    include_prerelease: bool,
) -> Result<Option<GitHubRelease>, String> {
    let releases = fetch_releases(repository).await?;
    Ok(releases
        .into_iter()
        .filter(|release| include_prerelease || !release.prerelease)
        .find(has_update_manifest))
}

fn build_response(
    app: &AppHandle,
    repository: String,
    release: Option<&GitHubRelease>,
    available: bool,
    message: Option<String>,
    body_override: Option<String>,
) -> AppUpdateCheckResponse {
    AppUpdateCheckResponse {
        configured: true,
        available,
        current_version: current_version(app),
        version: release.map(|item| version_from_tag(&item.tag_name)),
        date: release.and_then(|item| item.published_at.clone()),
        body: body_override.or_else(|| release.and_then(|item| item.body.clone())),
        channel: release
            .map(release_channel)
            .unwrap_or(AppUpdateChannel::Stable),
        release_tag: release.map(|item| item.tag_name.clone()),
        release_name: release.and_then(|item| item.name.clone()),
        release_url: release.and_then(|item| item.html_url.clone()),
        repository,
        message,
    }
}

fn build_updater(
    app: &AppHandle,
    manifest_url: &str,
) -> Result<tauri_plugin_updater::Updater, String> {
    let endpoint =
        Url::parse(manifest_url).map_err(|error| format!("invalid updater URL: {error}"))?;
    let mut builder = app.updater_builder();
    if let Some(public_key) = updater_public_key_override() {
        builder = builder.pubkey(public_key);
    }
    builder
        .endpoints(vec![endpoint])
        .map_err(|error| format!("invalid updater endpoint: {error}"))?
        .build()
        .map_err(|error| format!("failed to initialize updater: {error}"))
}

#[tauri::command]
pub async fn app_update_check(
    app: AppHandle,
    include_prerelease: bool,
) -> Result<AppUpdateCheckResponse, String> {
    let repository = update_repository();
    let release = select_release(&repository, include_prerelease).await?;
    let Some(release) = release else {
        return Ok(build_response(
            &app,
            repository,
            None,
            false,
            Some("No GitHub Release with latest.json was found.".to_string()),
            None,
        ));
    };

    let manifest_url = release_manifest_url(&repository, &release.tag_name);
    let updater = build_updater(&app, &manifest_url)?;
    let update = updater
        .check()
        .await
        .map_err(|error| format!("failed to check updater manifest: {error}"))?;
    let body_override = update.as_ref().and_then(|item| item.body.clone());
    Ok(build_response(
        &app,
        repository,
        Some(&release),
        update.is_some(),
        None,
        body_override,
    ))
}

#[tauri::command]
pub async fn app_update_install(
    app: AppHandle,
    include_prerelease: bool,
) -> Result<AppUpdateCheckResponse, String> {
    let repository = update_repository();
    let release = select_release(&repository, include_prerelease).await?;
    let Some(release) = release else {
        return Ok(build_response(
            &app,
            repository,
            None,
            false,
            Some("No GitHub Release with latest.json was found.".to_string()),
            None,
        ));
    };

    let manifest_url = release_manifest_url(&repository, &release.tag_name);
    let updater = build_updater(&app, &manifest_url)?;
    let update = updater
        .check()
        .await
        .map_err(|error| format!("failed to check updater manifest: {error}"))?
        .ok_or_else(|| "No newer update is available.".to_string())?;
    let body_override = update.body.clone();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| format!("failed to install update: {error}"))?;
    Ok(build_response(
        &app,
        repository,
        Some(&release),
        false,
        Some("Update installed. Restart CrateBay to finish.".to_string()),
        body_override,
    ))
}

#[tauri::command]
pub fn app_restart(app: AppHandle) -> Result<(), String> {
    app.restart();
}
