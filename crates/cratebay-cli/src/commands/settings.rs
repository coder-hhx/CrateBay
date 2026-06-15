//! Persisted app settings commands.

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::{json, Value};

use cratebay_core::{settings as core_settings, storage};

use super::{print_structured, OutputFormat};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SettingKind {
    Language,
    Theme,
    RegistryMirrors,
    RuntimeHttpProxy,
    RuntimeHttpProxyBridge,
    RuntimeHttpProxyBindHost,
    RuntimeHttpProxyBindPort,
    RuntimeHttpProxyGuestHost,
    IncludePrereleases,
}

#[derive(Copy, Clone, Debug)]
struct SettingDefinition {
    key: &'static str,
    kind: SettingKind,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingPayload {
    key: String,
    value: Value,
    stored_value: String,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsListPayload {
    count: usize,
    items: Vec<SettingPayload>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsMutationPayload {
    key: String,
    value: Value,
    stored_value: String,
    message: String,
}

const SETTING_DEFINITIONS: &[SettingDefinition] = &[
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_LANGUAGE,
        kind: SettingKind::Language,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_THEME,
        kind: SettingKind::Theme,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_REGISTRY_MIRRORS,
        kind: SettingKind::RegistryMirrors,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY,
        kind: SettingKind::RuntimeHttpProxy,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE,
        kind: SettingKind::RuntimeHttpProxyBridge,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST,
        kind: SettingKind::RuntimeHttpProxyBindHost,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT,
        kind: SettingKind::RuntimeHttpProxyBindPort,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST,
        kind: SettingKind::RuntimeHttpProxyGuestHost,
    },
    SettingDefinition {
        key: core_settings::SETTINGS_KEY_INCLUDE_PRERELEASES,
        kind: SettingKind::IncludePrereleases,
    },
];

fn definition_for_key(key: &str) -> Result<SettingDefinition> {
    SETTING_DEFINITIONS
        .iter()
        .copied()
        .find(|definition| definition.key == key)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown setting '{}'. Supported keys: {}",
                key,
                SETTING_DEFINITIONS
                    .iter()
                    .map(|definition| definition.key)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn parse_boolish(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn default_stored_value(definition: SettingDefinition) -> String {
    match definition.kind {
        SettingKind::Language => core_settings::DEFAULT_LANGUAGE.to_string(),
        SettingKind::Theme => core_settings::DEFAULT_THEME.to_string(),
        SettingKind::RegistryMirrors => {
            serde_json::to_string(&core_settings::default_registry_mirrors())
                .expect("default registry mirrors should serialize")
        }
        SettingKind::RuntimeHttpProxy => String::new(),
        SettingKind::RuntimeHttpProxyBridge => {
            core_settings::DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE.to_string()
        }
        SettingKind::RuntimeHttpProxyBindHost => {
            core_settings::DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST.to_string()
        }
        SettingKind::RuntimeHttpProxyBindPort => {
            core_settings::DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT.to_string()
        }
        SettingKind::RuntimeHttpProxyGuestHost => {
            core_settings::DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST.to_string()
        }
        SettingKind::IncludePrereleases => core_settings::DEFAULT_INCLUDE_PRERELEASES.to_string(),
    }
}

fn normalize_setting_value(definition: SettingDefinition, raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    match definition.kind {
        SettingKind::Language => match trimmed {
            "en" | "zh-CN" => Ok(trimmed.to_string()),
            _ => bail!("language must be one of: en, zh-CN"),
        },
        SettingKind::Theme => match trimmed {
            "dark" | "light" | "system" => Ok(trimmed.to_string()),
            _ => bail!("theme must be one of: dark, light, system"),
        },
        SettingKind::RegistryMirrors => {
            let mirrors = core_settings::parse_registry_mirrors_setting(raw);
            Ok(serde_json::to_string(&mirrors)?)
        }
        SettingKind::RuntimeHttpProxy => Ok(trimmed.to_string()),
        SettingKind::RuntimeHttpProxyBridge | SettingKind::IncludePrereleases => {
            let Some(value) = parse_boolish(raw) else {
                bail!(
                    "{} must be a boolean: true, false, 1, 0, yes, no, on, or off",
                    definition.key
                );
            };
            Ok(value.to_string())
        }
        SettingKind::RuntimeHttpProxyBindHost | SettingKind::RuntimeHttpProxyGuestHost => {
            if trimmed.is_empty() {
                bail!("{} cannot be empty", definition.key);
            }
            Ok(trimmed.to_string())
        }
        SettingKind::RuntimeHttpProxyBindPort => {
            let port = trimmed.parse::<u16>().map_err(|_| {
                anyhow::anyhow!("runtimeHttpProxyBindPort must be a port from 1 to 65535")
            })?;
            if port == 0 {
                bail!("runtimeHttpProxyBindPort must be greater than 0");
            }
            Ok(port.to_string())
        }
    }
}

fn value_from_stored(definition: SettingDefinition, stored_value: &str) -> Value {
    match definition.kind {
        SettingKind::RegistryMirrors => {
            json!(core_settings::parse_registry_mirrors_setting(stored_value))
        }
        SettingKind::RuntimeHttpProxyBridge | SettingKind::IncludePrereleases => {
            json!(parse_boolish(stored_value).unwrap_or(false))
        }
        SettingKind::RuntimeHttpProxyBindPort => stored_value
            .trim()
            .parse::<u16>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::from(core_settings::DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT)),
        _ => json!(stored_value),
    }
}

fn setting_payload_from_stored(
    definition: SettingDefinition,
    persisted: Option<String>,
) -> Result<SettingPayload> {
    let stored_value = persisted
        .clone()
        .unwrap_or_else(|| default_stored_value(definition));
    Ok(SettingPayload {
        key: definition.key.to_string(),
        value: value_from_stored(definition, &stored_value),
        stored_value,
        source: if persisted.is_some() {
            "persisted".to_string()
        } else {
            "default".to_string()
        },
    })
}

pub fn list(format: &OutputFormat) -> Result<()> {
    let db_path = storage::default_db_path()?;
    let conn = storage::init(&db_path)?;
    let items = SETTING_DEFINITIONS
        .iter()
        .map(|definition| {
            let persisted = storage::get_setting(&conn, definition.key)?;
            setting_payload_from_stored(*definition, persisted)
        })
        .collect::<Result<Vec<_>>>()?;
    let payload = SettingsListPayload {
        count: items.len(),
        items,
    };

    match format {
        OutputFormat::Table => {
            for item in &payload.items {
                println!("{}={}", item.key, item.stored_value);
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub fn get(key: &str, format: &OutputFormat) -> Result<()> {
    let definition = definition_for_key(key)?;
    let db_path = storage::default_db_path()?;
    let conn = storage::init(&db_path)?;
    let persisted = storage::get_setting(&conn, definition.key)?;
    let payload = setting_payload_from_stored(definition, persisted)?;

    match format {
        OutputFormat::Table => {
            println!("{}={}", payload.key, payload.stored_value);
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub fn set(key: &str, value: &str, format: &OutputFormat) -> Result<()> {
    let definition = definition_for_key(key)?;
    let stored_value = normalize_setting_value(definition, value)?;
    let db_path = storage::default_db_path()?;
    let conn = storage::init(&db_path)?;
    storage::set_setting(&conn, definition.key, &stored_value)?;
    let payload = SettingsMutationPayload {
        key: definition.key.to_string(),
        value: value_from_stored(definition, &stored_value),
        stored_value,
        message: "Setting saved.".to_string(),
    };

    match format {
        OutputFormat::Table => {
            println!("Setting saved.");
            println!("{}={}", payload.key, payload.stored_value);
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub fn reset(key: &str, format: &OutputFormat) -> Result<()> {
    let definition = definition_for_key(key)?;
    let stored_value = default_stored_value(definition);
    let db_path = storage::default_db_path()?;
    let conn = storage::init(&db_path)?;
    storage::set_setting(&conn, definition.key, &stored_value)?;
    let payload = SettingsMutationPayload {
        key: definition.key.to_string(),
        value: value_from_stored(definition, &stored_value),
        stored_value,
        message: "Setting reset.".to_string(),
    };

    match format {
        OutputFormat::Table => {
            println!("Setting reset.");
            println!("{}={}", payload.key, payload.stored_value);
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_keys_match_desktop_settings_store() {
        let gui_settings_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cratebay-gui/src/stores/settingsStore.ts");
        let gui_settings = std::fs::read_to_string(gui_settings_path)
            .expect("GUI settings store should be readable");

        for definition in SETTING_DEFINITIONS {
            assert!(
                gui_settings.contains(&format!("\"{}\"", definition.key)),
                "GUI settings store should include {}",
                definition.key
            );
        }
    }

    #[test]
    fn settings_defaults_match_desktop_settings_constants() {
        let gui_settings_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cratebay-gui/src/types/settings.ts");
        let gui_settings = std::fs::read_to_string(gui_settings_path)
            .expect("GUI settings types should be readable");

        for mirror in core_settings::DEFAULT_REGISTRY_MIRRORS {
            assert!(
                gui_settings.contains(&format!("\"{}\"", mirror)),
                "GUI registry mirror defaults should include {}",
                mirror
            );
        }

        assert!(
            gui_settings.contains("DEFAULT_RUNTIME_HTTP_PROXY = \"\""),
            "GUI runtime proxy default should stay empty"
        );
        assert!(
            gui_settings.contains(&format!(
                "DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE = {}",
                core_settings::DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE
            )),
            "GUI runtime proxy bridge default should match core"
        );
        assert!(
            gui_settings.contains(&format!(
                "DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST = \"{}\"",
                core_settings::DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST
            )),
            "GUI runtime proxy bind host default should match core"
        );
        assert!(
            gui_settings.contains(&format!(
                "DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT = {}",
                core_settings::DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT
            )),
            "GUI runtime proxy bind port default should match core"
        );
        assert!(
            gui_settings.contains(&format!(
                "DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST = \"{}\"",
                core_settings::DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST
            )),
            "GUI runtime proxy guest host default should match core"
        );
    }

    #[test]
    fn settings_normalize_registry_mirrors_for_desktop_storage() {
        let definition = definition_for_key(core_settings::SETTINGS_KEY_REGISTRY_MIRRORS)
            .expect("registry mirrors setting should exist");

        assert_eq!(
            normalize_setting_value(definition, "docker.1ms.run,\n mirror.local").unwrap(),
            r#"["docker.1ms.run","mirror.local"]"#
        );
        assert_eq!(
            normalize_setting_value(definition, r#"["docker.1ms.run"," mirror.local "]"#).unwrap(),
            r#"["docker.1ms.run","mirror.local"]"#
        );
    }

    #[test]
    fn settings_normalize_typed_values_for_desktop_storage() {
        let include_prereleases =
            definition_for_key(core_settings::SETTINGS_KEY_INCLUDE_PRERELEASES)
                .expect("include prereleases setting should exist");
        assert_eq!(
            normalize_setting_value(include_prereleases, "yes").unwrap(),
            "true"
        );
        assert_eq!(
            normalize_setting_value(include_prereleases, "0").unwrap(),
            "false"
        );

        let port = definition_for_key(core_settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT)
            .expect("proxy port setting should exist");
        assert_eq!(normalize_setting_value(port, "3128").unwrap(), "3128");
        assert!(normalize_setting_value(port, "0").is_err());

        let theme = definition_for_key(core_settings::SETTINGS_KEY_THEME)
            .expect("theme setting should exist");
        assert_eq!(normalize_setting_value(theme, "system").unwrap(), "system");
        assert!(normalize_setting_value(theme, "sepia").is_err());
    }
}
