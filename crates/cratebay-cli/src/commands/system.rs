use anyhow::Result;
use serde::Serialize;

use super::{print_structured, OutputFormat};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemInfoPayload {
    version: String,
    platform: String,
    arch: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineStatusPayload {
    engine: String,
    engine_responsive: bool,
    name: Option<String>,
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility_compatible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    docker: Option<String>,
    api: Option<String>,
    backend_runtime: Option<String>,
    oci_runtime: Option<String>,
    network_stack: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    docker_compatible: Option<bool>,
}

/// Show system information.
pub fn info(format: &OutputFormat) -> Result<()> {
    let payload = SystemInfoPayload {
        version: format!("CrateBay v{}", env!("CARGO_PKG_VERSION")),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    };

    match format {
        OutputFormat::Table => {
            println!("{}", payload.version);
            println!("Platform: {}", payload.platform);
            println!("Arch: {}", payload.arch);
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

/// Show CrateBay Engine connection status without starting the built-in runtime.
pub async fn engine_status(format: &OutputFormat) -> Result<()> {
    let runtime = cratebay_core::runtime::create_runtime_manager();
    let engine = cratebay_core::runtime::query_built_in_ready_engine_status(runtime.as_ref()).ok();

    let Some(engine) = engine else {
        let payload = EngineStatusPayload {
            engine: "notConnected".to_string(),
            engine_responsive: false,
            name: None,
            kind: None,
            compatibility: Some("notConnected".to_string()),
            compatibility_compatible: None,
            docker: Some("notConnected".to_string()),
            api: None,
            backend_runtime: None,
            oci_runtime: None,
            network_stack: None,
            docker_compatible: None,
        };
        return match format {
            OutputFormat::Table => {
                println!("CrateBay Engine: not connected");
                Ok(())
            }
            _ => print_structured(&payload, format),
        };
    };

    let payload = EngineStatusPayload {
        engine: "connected".to_string(),
        engine_responsive: true,
        name: Some(engine.name.clone()),
        kind: Some(engine.kind.clone()),
        compatibility: Some(if engine.docker_compatible {
            "enabled".to_string()
        } else {
            "disabled".to_string()
        }),
        compatibility_compatible: Some(engine.docker_compatible),
        docker: Some(if engine.docker_compatible {
            "compatible".to_string()
        } else {
            "disabled".to_string()
        }),
        api: Some(engine.api.clone()),
        backend_runtime: Some(engine.backend_runtime.clone()),
        oci_runtime: Some(engine.oci_runtime.clone()),
        network_stack: Some(engine.network_stack.clone()),
        docker_compatible: Some(engine.docker_compatible),
    };

    match format {
        OutputFormat::Table => {
            println!("CrateBay Engine: connected");
            println!("Engine: {} ({})", engine.name, engine.kind);
            println!("API: {}", engine.api);
            println!("Backend: {}", engine.backend_runtime);
            println!("OCI runtime: {}", engine.oci_runtime);
            println!("Network: {}", engine.network_stack);
            println!(
                "Compatibility endpoint: {}",
                if engine.docker_compatible {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn engine_status_uses_native_contract_instead_of_compatibility_version() {
        let source = include_str!("system.rs");
        let forbidden = "docker::".to_string() + "version";

        assert!(
            !source.contains(&forbidden),
            "system engine-status should use the native CrateBay Engine contract as its primary signal"
        );
        assert!(source.contains("query_built_in_ready_engine_status"));
        assert!(source.contains("engine_responsive"));
        assert!(source.contains("compatibility_compatible"));
        assert!(source.contains("backend_runtime"));
        assert!(source.contains("docker_compatible"));
    }

    #[test]
    fn engine_status_json_exposes_compatibility_aliases() {
        let payload = super::EngineStatusPayload {
            engine: "connected".to_string(),
            engine_responsive: true,
            name: Some("CrateBay Engine".to_string()),
            kind: Some("cratebay-containerd".to_string()),
            compatibility: Some("enabled".to_string()),
            compatibility_compatible: Some(true),
            docker: Some("compatible".to_string()),
            api: Some("cratebay.engine.v1".to_string()),
            backend_runtime: Some("containerd".to_string()),
            oci_runtime: Some("runc".to_string()),
            network_stack: Some("CNI".to_string()),
            docker_compatible: Some(true),
        };

        let json = serde_json::to_value(&payload).expect("payload should serialize");
        assert_eq!(json["engineResponsive"], true);
        assert_eq!(json["compatibility"], "enabled");
        assert_eq!(json["compatibilityCompatible"], true);
        assert_eq!(json["docker"], "compatible");
        assert_eq!(json["dockerCompatible"], true);
    }
}
