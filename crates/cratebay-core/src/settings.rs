//! Shared application settings defaults.

/// Persisted settings keys shared by desktop Settings and CLI automation.
pub const SETTINGS_KEY_LANGUAGE: &str = "language";
pub const SETTINGS_KEY_THEME: &str = "theme";
pub const SETTINGS_KEY_REGISTRY_MIRRORS: &str = "registryMirrors";
pub const SETTINGS_KEY_INCLUDE_PRERELEASES: &str = "includePrereleases";

/// Default Docker Hub mirror hosts used for fresh installs.
pub const DEFAULT_REGISTRY_MIRRORS: &[&str] =
    &["docker.1ms.run", "docker.xuanyuan.me", "dockerhub.icu"];

pub const SETTINGS_KEY_RUNTIME_HTTP_PROXY: &str = "runtimeHttpProxy";
pub const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE: &str = "runtimeHttpProxyBridge";
pub const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST: &str = "runtimeHttpProxyBindHost";
pub const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT: &str = "runtimeHttpProxyBindPort";
pub const SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST: &str = "runtimeHttpProxyGuestHost";

/// Default runtime HTTP proxy bridge values used by desktop Settings.
pub const DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE: bool = false;
pub const DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST: &str = "0.0.0.0";
pub const DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT: u16 = 3128;
pub const DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST: &str = "192.168.64.1";

pub const DEFAULT_LANGUAGE: &str = "en";
pub const DEFAULT_THEME: &str = "dark";
pub const DEFAULT_INCLUDE_PRERELEASES: bool = false;

pub fn default_registry_mirrors() -> Vec<String> {
    DEFAULT_REGISTRY_MIRRORS
        .iter()
        .map(ToString::to_string)
        .collect()
}

pub fn parse_registry_mirrors_setting(value: &str) -> Vec<String> {
    let from_json = serde_json::from_str::<Vec<String>>(value).ok();
    let items = from_json.unwrap_or_else(|| {
        value
            .split([',', '\n'])
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    });

    items
        .into_iter()
        .map(|mirror| mirror.trim().to_string())
        .filter(|mirror| !mirror.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_registry_mirrors_setting, DEFAULT_REGISTRY_MIRRORS};

    #[test]
    fn default_registry_mirrors_are_normalized_hosts() {
        assert!(!DEFAULT_REGISTRY_MIRRORS.is_empty());
        for mirror in DEFAULT_REGISTRY_MIRRORS {
            assert!(!mirror.trim().is_empty());
            assert_eq!(*mirror, mirror.trim());
            assert!(!mirror.starts_with("http://"));
            assert!(!mirror.starts_with("https://"));
            assert!(!mirror.ends_with('/'));
        }
    }

    #[test]
    fn registry_mirror_settings_parse_desktop_storage_formats() {
        assert_eq!(
            parse_registry_mirrors_setting(r#"["docker.1ms.run"," https://mirror.local/ "]"#),
            vec!["docker.1ms.run", "https://mirror.local/"]
        );
        assert_eq!(
            parse_registry_mirrors_setting("docker.1ms.run,\nmirror.local\n "),
            vec!["docker.1ms.run", "mirror.local"]
        );
        assert!(parse_registry_mirrors_setting("[]").is_empty());
    }
}
