#[cfg(any(target_os = "linux", test))]
mod engine_contract {
    use serde_json::{json, Value};

    pub(crate) struct EnginePayloadInput {
        pub socket: String,
        pub containerd_socket: String,
        pub namespace: String,
        pub version: String,
    }

    pub(crate) fn payload(input: EnginePayloadInput) -> Value {
        json!({
            "name": "CrateBay Engine",
            "kind": "cratebay-containerd",
            "state": "ready",
            "version": input.version,
            "backend": {
                "runtime": "containerd",
                "ociRuntime": "runc",
                "namespace": input.namespace,
                "containerdSocket": input.containerd_socket,
            },
            "network": {
                "stack": "CNI",
                "driver": "cratebay-cni",
            },
            "adapter": {
                "socket": input.socket,
                "api": "cratebay.engine.v1",
            },
            "compatibility": {
                "dockerCompatible": true,
                "dockerApiVersion": "1.44",
                "dockerSocket": input.socket,
            },
        })
    }

    #[cfg(test)]
    mod tests {
        use super::{payload, EnginePayloadInput};

        #[test]
        fn payload_identifies_containerd_backend_and_compat_layer() {
            let payload = payload(EnginePayloadInput {
                socket: "/run/cratebay/engine.sock".to_string(),
                containerd_socket: "/run/containerd/containerd.sock".to_string(),
                namespace: "cratebay-test".to_string(),
                version: "0.9.0".to_string(),
            });

            assert_eq!(payload["name"], "CrateBay Engine");
            assert_eq!(payload["kind"], "cratebay-containerd");
            assert_eq!(payload["backend"]["runtime"], "containerd");
            assert_eq!(payload["backend"]["ociRuntime"], "runc");
            assert_eq!(payload["backend"]["namespace"], "cratebay-test");
            assert_eq!(payload["network"]["stack"], "CNI");
            assert_eq!(payload["adapter"]["api"], "cratebay.engine.v1");
            assert_eq!(payload["compatibility"]["dockerCompatible"], true);
            assert_eq!(payload["compatibility"]["dockerApiVersion"], "1.44");
        }
    }
}

#[cfg(any(target_os = "linux", test))]
mod native_contract {
    use serde_json::{json, Value};

    pub(crate) fn container_summary(entry: Value) -> Value {
        let id = string_field(&entry, &["Id", "ID", "ContainerID"]).unwrap_or_default();
        let name = entry
            .get("Names")
            .and_then(Value::as_array)
            .and_then(|names| names.first())
            .and_then(Value::as_str)
            .map(|name| name.trim_start_matches('/').to_string())
            .filter(|name| !name.trim().is_empty())
            .or_else(|| string_field(&entry, &["Name", "Names"]))
            .unwrap_or_else(|| id.clone());
        let image = string_field(&entry, &["Image"]).unwrap_or_default();
        let status = string_field(&entry, &["Status"]).unwrap_or_default();
        let state = string_field(&entry, &["State"]).unwrap_or_else(|| state_from_status(&status));

        json!({
            "id": id,
            "name": name,
            "image": image,
            "state": state,
            "status": status,
            "labels": object_or_empty(entry.get("Labels")),
            "managedBy": "cratebay",
        })
    }

    pub(crate) fn image_summary(entry: Value) -> Value {
        let id = string_field(&entry, &["Id", "ID", "ImageID", "Digest"]).unwrap_or_default();
        let tags = match string_array(entry.get("RepoTags")) {
            tags if !tags.is_empty() => tags,
            _ => repo_tags_from_parts(
                &string_field(&entry, &["Repository", "RepositoryName", "Name"])
                    .unwrap_or_else(|| "<none>".to_string()),
                &string_field(&entry, &["Tag"]).unwrap_or_else(|| "latest".to_string()),
            ),
        };
        let primary_tag = tags
            .iter()
            .find(|tag| !tag.starts_with("<none>"))
            .cloned()
            .unwrap_or_else(|| tags.first().cloned().unwrap_or_default());
        let (repository, tag) = primary_tag
            .rsplit_once(':')
            .map(|(repository, tag)| (repository.to_string(), tag.to_string()))
            .unwrap_or_else(|| (primary_tag.clone(), String::new()));

        json!({
            "id": id,
            "repository": repository,
            "tag": tag,
            "tags": tags,
            "digests": string_array(entry.get("RepoDigests")),
            "sizeBytes": numeric_u64(entry.get("Size"))
                .or_else(|| numeric_u64(entry.get("VirtualSize")))
                .unwrap_or_default(),
            "created": numeric_i64(entry.get("Created")).unwrap_or_default(),
            "labels": object_or_empty(entry.get("Labels")),
            "managedBy": "cratebay",
        })
    }

    pub(crate) fn network_summary(entry: Value) -> Value {
        json!({
            "id": string_field(&entry, &["Id", "ID", "id"]).unwrap_or_default(),
            "name": string_field(&entry, &["Name", "NAME", "name"]).unwrap_or_default(),
            "driver": string_field(&entry, &["Driver", "DRIVER", "driver"])
                .unwrap_or_else(|| "bridge".to_string()),
            "scope": string_field(&entry, &["Scope", "scope"]).unwrap_or_else(|| "local".to_string()),
            "internal": bool_value(entry.get("Internal").or_else(|| entry.get("internal"))),
            "attachable": entry
                .get("Attachable")
                .or_else(|| entry.get("attachable"))
                .map(|value| bool_value(Some(value)))
                .unwrap_or(true),
            "labels": object_or_empty(entry.get("Labels").or_else(|| entry.get("labels"))),
            "containers": object_or_empty(entry.get("Containers").or_else(|| entry.get("containers"))),
            "managedBy": "cratebay",
        })
    }

    pub(crate) fn volume_summary(entry: Value) -> Value {
        json!({
            "name": string_field(&entry, &["Name", "name"]).unwrap_or_default(),
            "driver": string_field(&entry, &["Driver", "driver"])
                .unwrap_or_else(|| "local".to_string()),
            "mountpoint": string_field(&entry, &["Mountpoint", "mountpoint"]).unwrap_or_default(),
            "createdAt": string_field(&entry, &["CreatedAt", "createdAt", "created"]).unwrap_or_default(),
            "scope": string_field(&entry, &["Scope", "scope"]).unwrap_or_else(|| "local".to_string()),
            "labels": object_or_empty(entry.get("Labels").or_else(|| entry.get("labels"))),
            "options": object_or_empty(entry.get("Options").or_else(|| entry.get("options"))),
            "managedBy": "cratebay",
        })
    }

    pub(crate) fn pod_summary(entry: Value) -> Value {
        let labels = object_or_empty(entry.get("Labels").or_else(|| entry.get("labels")));
        let containers =
            pod_containers(entry.get("Containers").or_else(|| entry.get("containers")));
        json!({
            "id": string_field(&entry, &["Id", "ID", "id"]).unwrap_or_default(),
            "name": string_field(&entry, &["Name", "NAME", "name"]).unwrap_or_default(),
            "driver": string_field(&entry, &["Driver", "DRIVER", "driver"])
                .unwrap_or_else(|| "bridge".to_string()),
            "createdAt": string_field(&entry, &["Created", "CreatedAt", "created"]).unwrap_or_default(),
            "labels": labels,
            "containers": containers,
            "containerCount": containers.as_array().map(|items| items.len()).unwrap_or_default(),
            "managedBy": "cratebay",
        })
    }

    pub(crate) fn container_inspect(entry: Value) -> Value {
        let id = string_field(&entry, &["Id", "ID", "id", "ContainerID"]).unwrap_or_default();
        let name = string_field(&entry, &["Name", "Names", "name"])
            .map(|name| name.trim_start_matches('/').to_string())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| id.clone());
        let image = nested_string(&entry, &["Config", "Image"])
            .or_else(|| string_field(&entry, &["Image", "ImageName"]))
            .unwrap_or_default();
        let state = entry.get("State").cloned().unwrap_or_else(|| json!({}));

        json!({
            "id": id,
            "name": name,
            "image": image,
            "createdAt": string_field(&entry, &["Created", "CreatedAt", "created"]).unwrap_or_default(),
            "state": state,
            "config": object_or_empty(entry.get("Config").or_else(|| entry.get("config"))),
            "hostConfig": object_or_empty(entry.get("HostConfig").or_else(|| entry.get("hostConfig"))),
            "networkSettings": entry
                .get("NetworkSettings")
                .or_else(|| entry.get("networkSettings"))
                .cloned()
                .unwrap_or_else(|| json!({ "Networks": {} })),
            "mounts": array_or_empty(entry.get("Mounts").or_else(|| entry.get("mounts"))),
            "managedBy": "cratebay",
        })
    }

    fn string_field(entry: &Value, keys: &[&str]) -> Option<String> {
        keys.iter()
            .find_map(|key| optional_string_value(entry.get(*key)))
    }

    fn optional_string_value(value: Option<&Value>) -> Option<String> {
        match value? {
            Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        }
    }

    fn object_or_empty(value: Option<&Value>) -> Value {
        match value {
            Some(Value::Object(map)) => Value::Object(map.clone()),
            _ => json!({}),
        }
    }

    fn array_or_empty(value: Option<&Value>) -> Value {
        match value {
            Some(Value::Array(items)) => Value::Array(items.clone()),
            _ => json!([]),
        }
    }

    fn nested_string(entry: &Value, keys: &[&str]) -> Option<String> {
        let mut current = entry;
        for key in keys {
            current = current.get(*key)?;
        }
        optional_string_value(Some(current))
    }

    fn pod_containers(value: Option<&Value>) -> Value {
        let Some(Value::Object(containers)) = value else {
            return json!([]);
        };
        Value::Array(
            containers
                .iter()
                .map(|(id, container)| {
                    json!({
                        "id": id,
                        "name": string_field(container, &["Name", "name"]).unwrap_or_default(),
                        "ipv4Address": string_field(container, &["IPv4Address", "ipv4Address"]).unwrap_or_default(),
                        "ipv6Address": string_field(container, &["IPv6Address", "ipv6Address"]).unwrap_or_default(),
                    })
                })
                .collect(),
        )
    }

    fn bool_value(value: Option<&Value>) -> bool {
        match value {
            Some(Value::Bool(value)) => *value,
            Some(Value::Number(number)) => number.as_i64().unwrap_or_default() != 0,
            Some(Value::String(text)) => matches!(
                text.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        }
    }

    fn string_array(value: Option<&Value>) -> Vec<String> {
        match value {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| optional_string_value(Some(item)))
                .collect(),
            Some(Value::String(text)) if !text.trim().is_empty() => vec![text.trim().to_string()],
            _ => Vec::new(),
        }
    }

    fn numeric_i64(value: Option<&Value>) -> Option<i64> {
        match value? {
            Value::Number(number) => number
                .as_i64()
                .or_else(|| number.as_u64().map(|v| v as i64)),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        }
    }

    fn numeric_u64(value: Option<&Value>) -> Option<u64> {
        match value? {
            Value::Number(number) => number
                .as_u64()
                .or_else(|| number.as_i64().map(|v| v as u64)),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        }
    }

    fn repo_tags_from_parts(repository: &str, tag: &str) -> Vec<String> {
        if repository.is_empty() || repository == "<none>" {
            return Vec::new();
        }

        if tag.is_empty() || tag == "<none>" {
            vec![repository.to_string()]
        } else {
            vec![format!("{repository}:{tag}")]
        }
    }

    fn state_from_status(status: &str) -> String {
        let normalized = status.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return "created".to_string();
        }
        if normalized == "running" || normalized.contains("up ") || normalized.starts_with("up") {
            "running".to_string()
        } else if normalized.contains("paused") {
            "paused".to_string()
        } else if normalized.contains("restarting") {
            "restarting".to_string()
        } else if normalized.contains("removing") {
            "removing".to_string()
        } else if normalized.contains("dead") {
            "dead".to_string()
        } else if normalized.contains("exited") || normalized.contains("stopped") {
            "exited".to_string()
        } else if normalized.contains("created") {
            "created".to_string()
        } else {
            normalized
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            container_inspect, container_summary, image_summary, network_summary, pod_summary,
            volume_summary,
        };
        use serde_json::json;

        #[test]
        fn container_summary_normalizes_docker_shape() {
            let payload = container_summary(json!({
                "Id": "abc123",
                "Names": ["/sandbox-demo"],
                "Image": "cratebay-ubuntu-base:v1",
                "State": "running",
                "Status": "Up 10 seconds",
                "Labels": { "com.cratebay.managed": "true" }
            }));

            assert_eq!(payload["id"], "abc123");
            assert_eq!(payload["name"], "sandbox-demo");
            assert_eq!(payload["image"], "cratebay-ubuntu-base:v1");
            assert_eq!(payload["state"], "running");
            assert_eq!(payload["status"], "Up 10 seconds");
            assert_eq!(payload["labels"]["com.cratebay.managed"], "true");
            assert_eq!(payload["managedBy"], "cratebay");
        }

        #[test]
        fn image_summary_normalizes_docker_shape() {
            let payload = image_summary(json!({
                "Id": "sha256:abc123",
                "RepoTags": ["cratebay-ubuntu-base:v1"],
                "RepoDigests": ["cratebay-ubuntu-base@sha256:def456"],
                "Size": 123456,
                "Created": 1700000000,
                "Labels": { "com.cratebay.bundle": "true" }
            }));

            assert_eq!(payload["id"], "sha256:abc123");
            assert_eq!(payload["repository"], "cratebay-ubuntu-base");
            assert_eq!(payload["tag"], "v1");
            assert_eq!(payload["tags"][0], "cratebay-ubuntu-base:v1");
            assert_eq!(payload["digests"][0], "cratebay-ubuntu-base@sha256:def456");
            assert_eq!(payload["sizeBytes"], 123456);
            assert_eq!(payload["created"], 1700000000);
            assert_eq!(payload["labels"]["com.cratebay.bundle"], "true");
            assert_eq!(payload["managedBy"], "cratebay");
        }

        #[test]
        fn network_summary_normalizes_docker_shape() {
            let payload = network_summary(json!({
                "Id": "net123",
                "Name": "pod-demo",
                "Driver": "bridge",
                "Scope": "local",
                "Internal": false,
                "Attachable": true,
                "Labels": { "com.cratebay.pod": "true" },
                "Containers": { "abc123": { "Name": "sandbox-demo" } }
            }));

            assert_eq!(payload["id"], "net123");
            assert_eq!(payload["name"], "pod-demo");
            assert_eq!(payload["driver"], "bridge");
            assert_eq!(payload["scope"], "local");
            assert_eq!(payload["internal"], false);
            assert_eq!(payload["attachable"], true);
            assert_eq!(payload["labels"]["com.cratebay.pod"], "true");
            assert_eq!(payload["containers"]["abc123"]["Name"], "sandbox-demo");
            assert_eq!(payload["managedBy"], "cratebay");
        }

        #[test]
        fn volume_summary_normalizes_docker_shape() {
            let payload = volume_summary(json!({
                "Name": "workspace-cache",
                "Driver": "local",
                "Mountpoint": "/var/lib/cratebay-engine/volumes/workspace-cache/_data",
                "CreatedAt": "2026-06-03T00:00:00Z",
                "Scope": "local",
                "Labels": { "com.cratebay.volume": "true" },
                "Options": {}
            }));

            assert_eq!(payload["name"], "workspace-cache");
            assert_eq!(payload["driver"], "local");
            assert_eq!(
                payload["mountpoint"],
                "/var/lib/cratebay-engine/volumes/workspace-cache/_data"
            );
            assert_eq!(payload["createdAt"], "2026-06-03T00:00:00Z");
            assert_eq!(payload["scope"], "local");
            assert_eq!(payload["labels"]["com.cratebay.volume"], "true");
            assert_eq!(payload["managedBy"], "cratebay");
        }

        #[test]
        fn pod_summary_normalizes_network_shape() {
            let payload = pod_summary(json!({
                "Id": "pod123",
                "Name": "demo-pod",
                "Driver": "bridge",
                "Created": "2026-06-03T00:00:00Z",
                "Labels": {
                    "com.cratebay.pod": "true",
                    "com.cratebay.managed": "true"
                },
                "Containers": {
                    "abc123": {
                        "Name": "sandbox-demo",
                        "IPv4Address": "10.4.0.2/24"
                    }
                }
            }));

            assert_eq!(payload["id"], "pod123");
            assert_eq!(payload["name"], "demo-pod");
            assert_eq!(payload["labels"]["com.cratebay.pod"], "true");
            assert_eq!(payload["containers"][0]["id"], "abc123");
            assert_eq!(payload["containers"][0]["name"], "sandbox-demo");
            assert_eq!(payload["containerCount"], 1);
            assert_eq!(payload["managedBy"], "cratebay");
        }

        #[test]
        fn container_inspect_normalizes_docker_shape() {
            let payload = container_inspect(json!({
                "Id": "abc123",
                "Name": "/sandbox-demo",
                "Created": "2026-06-03T00:00:00Z",
                "Config": {
                    "Image": "cratebay-ubuntu-base:v1",
                    "Labels": { "com.cratebay.managed": "true" }
                },
                "State": {
                    "Status": "running",
                    "Running": true
                },
                "NetworkSettings": {
                    "Networks": { "bridge": {} }
                },
                "Mounts": []
            }));

            assert_eq!(payload["id"], "abc123");
            assert_eq!(payload["name"], "sandbox-demo");
            assert_eq!(payload["image"], "cratebay-ubuntu-base:v1");
            assert_eq!(payload["state"]["Status"], "running");
            assert_eq!(payload["config"]["Labels"]["com.cratebay.managed"], "true");
            assert_eq!(payload["managedBy"], "cratebay");
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("cratebay-engine-adapter is only supported on Linux guests");
    std::process::exit(2);
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = linux::run_entry() {
        eprintln!("cratebay-engine-adapter: {error}");
        std::process::exit(1);
    }
}

#[cfg(any(target_os = "linux", test))]
#[cfg_attr(test, allow(dead_code))]
mod linux {
    use crate::native_contract;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::collections::{HashMap, HashSet};
    use std::ffi::{CStr, OsString};
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Cursor, Read, Write};
    use std::net::{IpAddr, TcpStream};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::path::{Path, PathBuf};
    use std::process::{Child, ChildStdin, Command, Output, Stdio};
    use std::sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    };
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const CNI_PLUGIN_TIMEOUT: Duration = Duration::from_secs(8);
    const IP_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
    const TASK_DISCOVERY_GRACE: Duration = Duration::from_secs(3);
    const LOST_TASK_EXIT_CODE: i64 = 126;
    const DEFAULT_NO_PROXY: &str = "localhost,127.0.0.1,::1";
    const DEFAULT_ENGINE_ADAPTER_SOCKET: &str = "/run/cratebay/engine.sock";
    const CGROUP_MOUNT: &str = "/sys/fs/cgroup";
    const DEFAULT_RUNC_PATH: &str = "/usr/bin/runc";
    const DEFAULT_CRATEBAY_RUNC_WRAPPER_PATH: &str = "/usr/local/bin/cratebay-engine-adapter";
    const BUILDKIT_RUNC_PATH: &str = "/usr/bin/buildkit-runc";
    const PY36_CTYPES_MOVAPS_STORE: &[u8] = &[0x0f, 0x29, 0x43, 0x60];
    const PY36_CTYPES_MOVUPS_STORE: &[u8] = &[0x0f, 0x11, 0x43, 0x60];

    #[derive(Debug, Clone)]
    struct Config {
        socket: PathBuf,
        containerd_socket: PathBuf,
        namespace: String,
        ctr: String,
    }

    #[derive(Debug, Clone)]
    struct AdapterState {
        config: Config,
        execs: Arc<Mutex<HashMap<String, ExecRecord>>>,
        terminals: Arc<Mutex<HashMap<String, TerminalSession>>>,
        metrics: Arc<Mutex<HashMap<String, ContainerMetricSnapshot>>>,
        pending_containers: Arc<Mutex<HashMap<String, PendingContainer>>>,
    }

    #[derive(Debug, Clone)]
    struct ExecRecord {
        container_id: String,
        cmd: Vec<String>,
        working_dir: Option<String>,
        attach_stdin: bool,
        tty: bool,
        exit_code: Option<i64>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    }

    #[derive(Debug, Clone)]
    struct TerminalSession {
        container_id: String,
        input: Arc<Mutex<TerminalInput>>,
        child: Arc<Mutex<Child>>,
        output: Arc<Mutex<Vec<TerminalOutputChunk>>>,
        exit_code: Arc<Mutex<Option<i64>>>,
        transport: &'static str,
    }

    #[derive(Debug)]
    enum TerminalInput {
        Pipe(Option<ChildStdin>),
        Pty(File),
    }

    #[derive(Debug, Clone)]
    struct TerminalOutputChunk {
        stream: &'static str,
        data: String,
    }

    #[derive(Debug, Clone, Default)]
    struct ContainerMetricSnapshot {
        cpu_total: u64,
        system_total: u64,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct ContainerRuntimeMetrics {
        cpu_total: u64,
        memory_usage: u64,
        memory_limit: u64,
    }

    #[derive(Debug, Clone)]
    struct PendingContainer {
        id: String,
        name: String,
        created_at: i64,
        runtime_id: String,
        image: String,
        command: Vec<String>,
        env: Vec<String>,
        working_dir: Option<String>,
        mounts: Vec<CtrMount>,
        network: Option<String>,
        aliases: Vec<String>,
        labels: serde_json::Map<String, Value>,
        netns_name: Option<String>,
        netns_path: Option<PathBuf>,
        ports: Vec<CniPortMapping>,
        log_path: PathBuf,
        no_pull: bool,
        registry_mirrors: Vec<String>,
        privileged: bool,
        started_with_ctr: bool,
        exit_code: Option<i64>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CtrMount {
        source: String,
        target: String,
        readonly: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CniPortMapping {
        host_ip: Option<String>,
        host_port: u16,
        container_port: u16,
        protocol: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct PendingNetworkAttachment {
        network: String,
        netns_name: String,
        netns_path: PathBuf,
    }

    struct ImagePullResult {
        backend: &'static str,
        image_ref: String,
        mirror: Option<String>,
        output: Output,
        containerd_errors: Vec<Value>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RegistryImageRef {
        registry: String,
        repository: String,
        reference: String,
    }

    #[derive(Debug)]
    struct RegistryHttpResponse {
        headers: HashMap<String, String>,
        body: Vec<u8>,
    }

    struct ExecRunResult {
        backend: &'static str,
        args: Vec<String>,
        output: Output,
        exit_code: i64,
        timed_out: bool,
        stdout_truncated: bool,
        stderr_truncated: bool,
    }

    struct ContainerCommitResult {
        target_ref: String,
        layer_digest: String,
        config_digest: String,
        rootfs: PathBuf,
    }

    #[derive(Debug)]
    struct HttpRequest {
        method: String,
        path: String,
        body: Vec<u8>,
        body_spool_path: Option<PathBuf>,
    }

    impl Drop for HttpRequest {
        fn drop(&mut self) {
            if let Some(path) = self.body_spool_path.as_ref() {
                let _ = fs::remove_file(path);
            }
        }
    }

    #[derive(Debug)]
    struct HttpResponse {
        status: u16,
        reason: &'static str,
        content_type: &'static str,
        upgrade: bool,
        body: Vec<u8>,
    }

    pub fn run_entry() -> Result<(), String> {
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        if is_runc_wrapper_invocation(&args) {
            return run_runc_wrapper(args);
        }
        run()
    }

    pub fn run() -> Result<(), String> {
        let config = Config::from_env_and_args()?;
        wait_for_containerd_socket(&config.containerd_socket, Duration::from_secs(30))?;

        if let Some(parent) = config.socket.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        let _ = fs::remove_file(&config.socket);

        let listener = UnixListener::bind(&config.socket)
            .map_err(|error| format!("bind {}: {error}", config.socket.display()))?;
        fs::set_permissions(&config.socket, fs::Permissions::from_mode(0o660))
            .map_err(|error| format!("chmod {}: {error}", config.socket.display()))?;

        eprintln!(
            "cratebay-engine-adapter listening: {} -> containerd {} namespace={}",
            config.socket.display(),
            config.containerd_socket.display(),
            config.namespace
        );

        let state = AdapterState {
            config,
            execs: Arc::new(Mutex::new(HashMap::new())),
            terminals: Arc::new(Mutex::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(HashMap::new())),
            pending_containers: Arc::new(Mutex::new(HashMap::new())),
        };

        for incoming in listener.incoming() {
            let stream = incoming.map_err(|error| format!("accept: {error}"))?;
            let state = state.clone();
            thread::spawn(move || {
                if let Err(error) = handle_connection(stream, &state) {
                    eprintln!("cratebay-engine-adapter: request failed: {error}");
                }
            });
        }

        Ok(())
    }

    fn is_runc_wrapper_invocation(args: &[OsString]) -> bool {
        if args.iter().any(|arg| arg == "--cratebay-runc-wrapper") {
            return true;
        }
        args.iter().any(|arg| {
            let Some(arg) = arg.to_str() else {
                return false;
            };
            matches!(
                arg,
                "create"
                    | "delete"
                    | "exec"
                    | "kill"
                    | "list"
                    | "pause"
                    | "ps"
                    | "resume"
                    | "run"
                    | "start"
                    | "state"
                    | "update"
                    | "events"
                    | "features"
                    | "checkpoint"
                    | "restore"
            )
        })
    }

    fn run_runc_wrapper(args: Vec<OsString>) -> Result<(), String> {
        if let Some(bundle) = runc_bundle_arg(&args) {
            if let Err(error) = apply_runc_bundle_compat(&bundle) {
                runc_wrapper_debug(format!(
                    "compat patch failed for {}: {error}",
                    bundle.display()
                ));
            }
        }

        let real_runc = std::env::var("CRATEBAY_REAL_RUNC")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                if Path::new(BUILDKIT_RUNC_PATH).exists() {
                    BUILDKIT_RUNC_PATH.to_string()
                } else {
                    DEFAULT_RUNC_PATH.to_string()
                }
            });
        let filtered_args = args
            .into_iter()
            .filter(|arg| arg != "--cratebay-runc-wrapper")
            .collect::<Vec<_>>();
        let error = Command::new(&real_runc).args(filtered_args).exec();
        Err(format!("exec {real_runc}: {error}"))
    }

    fn runc_bundle_arg(args: &[OsString]) -> Option<PathBuf> {
        let mut want_bundle = false;
        for arg in args {
            if want_bundle {
                return Some(PathBuf::from(arg));
            }
            let value = arg.to_string_lossy();
            if value == "--bundle" || value == "-b" {
                want_bundle = true;
            } else if let Some(bundle) = value.strip_prefix("--bundle=") {
                return Some(PathBuf::from(bundle));
            } else if let Some(bundle) = value.strip_prefix("-b=") {
                return Some(PathBuf::from(bundle));
            }
        }
        None
    }

    fn apply_runc_bundle_compat(bundle: &Path) -> io::Result<()> {
        let config_path = bundle.join("config.json");
        let mut config = read_runc_config(&config_path)?;
        apply_runc_config_env_compat(&mut config);
        relax_runc_seccomp_default(&mut config);
        write_runc_config(&config_path, &config)?;

        if let Some(rootfs) = runc_rootfs_path(bundle, &config) {
            for patched in patch_legacy_python36_ctypes(&rootfs)? {
                runc_wrapper_debug(format!(
                    "patched legacy Python _ctypes: {}",
                    patched.display()
                ));
            }
        }
        Ok(())
    }

    fn read_runc_config(path: &Path) -> io::Result<Value> {
        let bytes = fs::read(path)?;
        serde_json::from_slice(&bytes).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("parse {}: {error}", path.display()),
            )
        })
    }

    fn write_runc_config(path: &Path, config: &Value) -> io::Result<()> {
        let bytes = serde_json::to_vec(config).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("serialize {}: {error}", path.display()),
            )
        })?;
        fs::write(path, bytes)
    }

    fn apply_runc_config_env_compat(config: &mut Value) {
        let http_proxy = std::env::var("HTTP_PROXY")
            .ok()
            .or_else(|| std::env::var("http_proxy").ok())
            .and_then(|value| normalize_http_proxy_url(&value));
        let https_proxy = std::env::var("HTTPS_PROXY")
            .ok()
            .or_else(|| std::env::var("https_proxy").ok())
            .and_then(|value| normalize_http_proxy_url(&value))
            .or_else(|| http_proxy.clone());
        let no_proxy = std::env::var("NO_PROXY")
            .ok()
            .or_else(|| std::env::var("no_proxy").ok())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_NO_PROXY.to_string());

        if let Some(value) = http_proxy.as_deref() {
            add_runc_process_env(config, "HTTP_PROXY", value);
            add_runc_process_env(config, "http_proxy", value);
        }
        if let Some(value) = https_proxy.as_deref() {
            add_runc_process_env(config, "HTTPS_PROXY", value);
            add_runc_process_env(config, "https_proxy", value);
        }
        add_runc_process_env(config, "NO_PROXY", &no_proxy);
        add_runc_process_env(config, "no_proxy", &no_proxy);
    }

    fn add_runc_process_env(config: &mut Value, key: &str, value: &str) {
        if value.trim().is_empty() {
            return;
        }
        let Some(env) = config
            .get_mut("process")
            .and_then(|process| process.get_mut("env"))
            .and_then(Value::as_array_mut)
        else {
            return;
        };
        if env.iter().any(|item| {
            item.as_str()
                .map(|item| env_key(item) == key)
                .unwrap_or(false)
        }) {
            return;
        }
        env.push(Value::String(format!("{key}={value}")));
    }

    fn relax_runc_seccomp_default(config: &mut Value) {
        let Some(seccomp) = config
            .get_mut("linux")
            .and_then(|linux| linux.get_mut("seccomp"))
            .and_then(Value::as_object_mut)
        else {
            return;
        };
        if seccomp
            .get("defaultAction")
            .and_then(Value::as_str)
            .map(|action| action == "SCMP_ACT_ERRNO")
            .unwrap_or(false)
        {
            seccomp.insert(
                "defaultAction".to_string(),
                Value::String("SCMP_ACT_ALLOW".to_string()),
            );
            seccomp.insert("defaultErrnoRet".to_string(), Value::Number(0.into()));
        }
    }

    fn runc_rootfs_path(bundle: &Path, config: &Value) -> Option<PathBuf> {
        let root_path = config
            .get("root")
            .and_then(|root| root.get("path"))
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .unwrap_or("rootfs");
        let rootfs = PathBuf::from(root_path);
        if rootfs.is_absolute() {
            Some(rootfs)
        } else {
            Some(bundle.join(rootfs))
        }
    }

    fn patch_legacy_python36_ctypes(rootfs: &Path) -> io::Result<Vec<PathBuf>> {
        let mut patched = Vec::new();
        for path in legacy_python36_ctypes_candidates(rootfs) {
            if patch_legacy_python36_ctypes_file(&path)? {
                patched.push(path);
            }
        }
        Ok(patched)
    }

    fn legacy_python36_ctypes_candidates(rootfs: &Path) -> Vec<PathBuf> {
        [
            "usr/local/lib/python3.6/lib-dynload/_ctypes.cpython-36m-x86_64-linux-gnu.so",
            "usr/lib/python3.6/lib-dynload/_ctypes.cpython-36m-x86_64-linux-gnu.so",
            "usr/lib64/python3.6/lib-dynload/_ctypes.cpython-36m-x86_64-linux-gnu.so",
        ]
        .into_iter()
        .map(|path| rootfs.join(path))
        .collect()
    }

    fn patch_legacy_python36_ctypes_file(path: &Path) -> io::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let mut bytes = fs::read(path)?;
        let mut changed = false;
        let mut offset = 0;
        while offset + PY36_CTYPES_MOVAPS_STORE.len() <= bytes.len() {
            if &bytes[offset..offset + PY36_CTYPES_MOVAPS_STORE.len()] == PY36_CTYPES_MOVAPS_STORE {
                bytes[offset..offset + PY36_CTYPES_MOVUPS_STORE.len()]
                    .copy_from_slice(PY36_CTYPES_MOVUPS_STORE);
                changed = true;
                offset += PY36_CTYPES_MOVUPS_STORE.len();
            } else {
                offset += 1;
            }
        }
        if changed {
            fs::write(path, bytes)?;
        }
        Ok(changed)
    }

    fn runc_wrapper_debug(message: String) {
        if std::env::var("CRATEBAY_RUNC_WRAPPER_DEBUG")
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
        {
            eprintln!("cratebay-runc-wrapper: {message}");
        }
    }

    impl Config {
        fn from_env_and_args() -> Result<Self, String> {
            let mut socket = env_adapter_socket_path()
                .unwrap_or_else(|| PathBuf::from(DEFAULT_ENGINE_ADAPTER_SOCKET));
            let mut containerd_socket = std::env::var("CRATEBAY_CONTAINERD_SOCKET")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/run/containerd/containerd.sock"));
            let mut namespace = std::env::var("CRATEBAY_CONTAINERD_NAMESPACE")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "cratebay".to_string());
            let mut ctr = std::env::var("CRATEBAY_CTR")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "ctr".to_string());

            let mut it = std::env::args().skip(1);
            while let Some(arg) = it.next() {
                match arg.as_str() {
                    "--socket" => {
                        socket = PathBuf::from(
                            it.next()
                                .ok_or_else(|| "--socket requires a value".to_string())?,
                        );
                    }
                    "--containerd-sock" => {
                        containerd_socket = PathBuf::from(
                            it.next()
                                .ok_or_else(|| "--containerd-sock requires a value".to_string())?,
                        );
                    }
                    "--namespace" => {
                        namespace = it
                            .next()
                            .ok_or_else(|| "--namespace requires a value".to_string())?;
                    }
                    "--ctr" => {
                        ctr = it
                            .next()
                            .ok_or_else(|| "--ctr requires a value".to_string())?;
                    }
                    "--help" | "-h" => {
                        print_usage();
                        std::process::exit(0);
                    }
                    other => return Err(format!("unknown argument: {other}")),
                }
            }

            if namespace.trim().is_empty() {
                return Err("--namespace cannot be empty".to_string());
            }

            Ok(Self {
                socket,
                containerd_socket,
                namespace,
                ctr,
            })
        }
    }

    fn print_usage() {
        println!(
            "Usage:\n  cratebay-engine-adapter [--socket <path>] [--containerd-sock <path>] [--namespace <name>] [--ctr <path>]\n\n\
             Exposes CrateBay Engine and Docker-compatible API surfaces backed by containerd, runc, CNI, and CrateBay-managed registries."
        );
    }

    fn env_adapter_socket_path() -> Option<PathBuf> {
        [
            "CRATEBAY_ENGINE_ADAPTER_SOCKET",
            "CRATEBAY_DOCKER_ADAPTER_SOCKET",
        ]
        .into_iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
        })
    }

    fn wait_for_containerd_socket(
        socket: &std::path::Path,
        timeout: Duration,
    ) -> Result<(), String> {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if socket.exists() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }

        Err(format!(
            "containerd socket was not ready within {}s: {}",
            timeout.as_secs(),
            socket.display()
        ))
    }

    fn handle_connection(mut stream: UnixStream, state: &AdapterState) -> Result<(), String> {
        let request = read_request(&mut stream)?;
        let (path, query) = normalize_docker_request_path(&request.path);
        if request.method == "POST" {
            if let Some(container_id) = container_action(&path, "attach") {
                return stream_container_attach(&mut stream, state, container_id, &query);
            }
            if let Some(exec_id) = exec_action(&path, "start") {
                return stream_exec_start(&mut stream, state, exec_id);
            }
        }
        let response = handle_request(request, state);
        write_response(&mut stream, response)
    }

    fn read_request(stream: &mut UnixStream) -> Result<HttpRequest, String> {
        let mut buffer = Vec::new();
        let mut temp = [0u8; 4096];
        let header_end;
        loop {
            let read = stream
                .read(&mut temp)
                .map_err(|error| format!("read: {error}"))?;
            if read == 0 {
                return Err("connection closed before headers".to_string());
            }
            buffer.extend_from_slice(&temp[..read]);
            if let Some(index) = find_header_end(&buffer) {
                header_end = index;
                break;
            }
            if buffer.len() > 1024 * 1024 {
                return Err("request headers too large".to_string());
            }
        }

        let header_text = String::from_utf8_lossy(&buffer[..header_end]);
        let mut lines = header_text.lines();
        let first = lines
            .next()
            .ok_or_else(|| "empty request".to_string())?
            .trim();
        let mut first_parts = first.split_whitespace();
        let method = first_parts
            .next()
            .ok_or_else(|| "missing method".to_string())?
            .to_string();
        let path = first_parts
            .next()
            .ok_or_else(|| "missing path".to_string())?
            .to_string();

        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
            .collect::<Vec<_>>();
        let content_length = headers
            .iter()
            .find_map(|(name, value)| {
                if name.eq_ignore_ascii_case("content-length") {
                    value.parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let is_chunked = headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case("chunked"))
        });

        let body_start = header_end + 4;
        let initial_body = buffer.get(body_start..).unwrap_or_default();
        if is_chunked {
            return Ok(HttpRequest {
                method,
                path,
                body: read_chunked_body(stream, initial_body)?,
                body_spool_path: None,
            });
        }
        if should_spool_request_body(&method, &path, content_length) {
            let spool_path = temp_archive_path("cratebay-native-image-import-body");
            let mut spool_file = fs::File::create(&spool_path)
                .map_err(|error| format!("create body spool {}: {error}", spool_path.display()))?;
            let initial_len = initial_body.len().min(content_length);
            if initial_len > 0 {
                spool_file
                    .write_all(&initial_body[..initial_len])
                    .map_err(|error| {
                        format!("write body spool {}: {error}", spool_path.display())
                    })?;
            }

            let mut written = initial_len;
            while written < content_length {
                let read = stream
                    .read(&mut temp)
                    .map_err(|error| format!("read body: {error}"))?;
                if read == 0 {
                    break;
                }
                let take = read.min(content_length - written);
                spool_file.write_all(&temp[..take]).map_err(|error| {
                    format!("write body spool {}: {error}", spool_path.display())
                })?;
                written += take;
            }

            if written != content_length {
                let _ = fs::remove_file(&spool_path);
                return Err(format!(
                    "request body ended early: expected {content_length} bytes, read {written}"
                ));
            }

            spool_file
                .sync_all()
                .map_err(|error| format!("sync body spool {}: {error}", spool_path.display()))?;
            return Ok(HttpRequest {
                method,
                path,
                body: Vec::new(),
                body_spool_path: Some(spool_path),
            });
        }

        let mut body = initial_body.to_vec();
        while body.len() < content_length {
            let read = stream
                .read(&mut temp)
                .map_err(|error| format!("read body: {error}"))?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&temp[..read]);
        }
        body.truncate(content_length);

        Ok(HttpRequest {
            method,
            path,
            body,
            body_spool_path: None,
        })
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn read_chunked_body(stream: &mut UnixStream, initial: &[u8]) -> Result<Vec<u8>, String> {
        let mut pending = initial.to_vec();
        let mut body = Vec::new();
        let mut temp = [0u8; 4096];
        loop {
            let line = read_chunk_line(stream, &mut pending, &mut temp)?;
            let size_text = line
                .split_once(';')
                .map(|(size, _)| size)
                .unwrap_or(line.as_str())
                .trim();
            let size = usize::from_str_radix(size_text, 16)
                .map_err(|error| format!("invalid chunk size '{size_text}': {error}"))?;
            if size == 0 {
                let _ = read_chunk_line(stream, &mut pending, &mut temp);
                return Ok(body);
            }

            read_until_pending_len(stream, &mut pending, &mut temp, size + 2)?;
            body.extend_from_slice(&pending[..size]);
            pending.drain(..size);
            if pending.starts_with(b"\r\n") {
                pending.drain(..2);
            } else if pending.starts_with(b"\n") {
                pending.drain(..1);
            } else {
                return Err("chunk data was not followed by CRLF".to_string());
            }
        }
    }

    fn read_chunk_line(
        stream: &mut UnixStream,
        pending: &mut Vec<u8>,
        temp: &mut [u8; 4096],
    ) -> Result<String, String> {
        loop {
            if let Some(index) = find_crlf(pending) {
                let line = String::from_utf8_lossy(&pending[..index]).into_owned();
                pending.drain(..index + 2);
                return Ok(line);
            }
            let read = stream
                .read(temp)
                .map_err(|error| format!("read chunk: {error}"))?;
            if read == 0 {
                return Err("connection closed while reading chunked body".to_string());
            }
            pending.extend_from_slice(&temp[..read]);
            if pending.len() > 16 * 1024 * 1024 {
                return Err("chunked request body buffer too large".to_string());
            }
        }
    }

    fn read_until_pending_len(
        stream: &mut UnixStream,
        pending: &mut Vec<u8>,
        temp: &mut [u8; 4096],
        min_len: usize,
    ) -> Result<(), String> {
        while pending.len() < min_len {
            let read = stream
                .read(temp)
                .map_err(|error| format!("read chunk data: {error}"))?;
            if read == 0 {
                return Err("connection closed while reading chunk data".to_string());
            }
            pending.extend_from_slice(&temp[..read]);
        }
        Ok(())
    }

    fn find_crlf(buffer: &[u8]) -> Option<usize> {
        buffer.windows(2).position(|window| window == b"\r\n")
    }

    fn should_spool_request_body(method: &str, raw_path: &str, content_length: usize) -> bool {
        if content_length == 0 || method != "POST" {
            return false;
        }
        let (path, _) = normalize_docker_request_path(raw_path);
        path == "/cratebay/images/import"
    }

    fn handle_request(request: HttpRequest, state: &AdapterState) -> HttpResponse {
        let (path, query) = normalize_docker_request_path(&request.path);
        let config = &state.config;
        match (request.method.as_str(), path.as_str()) {
            ("GET", "/_ping") | ("HEAD", "/_ping") => text_response(200, "OK", "OK"),
            ("GET", "/cratebay/engine") | ("GET", "/cratebay/engine/status") => {
                json_response(200, cratebay_engine_payload(config))
            }
            ("GET", "/cratebay/substrate") => json_response(200, cratebay_substrate_payload(state)),
            ("POST", "/cratebay/storage/gc") => native_storage_gc(state, &request.body),
            ("GET", "/cratebay/shim/tasks") => list_cratebay_shim_tasks(state),
            ("GET", "/cratebay/containers") => list_cratebay_containers(state),
            ("POST", "/cratebay/containers") => native_create_container(state, &request.body),
            ("GET", "/cratebay/images") => list_cratebay_images(config),
            ("POST", "/cratebay/images/pull") => native_pull_image(config, &request.body),
            ("GET", "/cratebay/images/export") => {
                native_export_images(config, &request.path, &query)
            }
            ("POST", "/cratebay/images/import") => {
                if let Some(path) = request.body_spool_path.as_deref() {
                    native_import_image_path(config, path)
                } else {
                    native_import_image(config, &request.body)
                }
            }
            ("POST", "/cratebay/images/pack-container") => {
                native_pack_container_image(config, &request.body)
            }
            ("GET", _) if cratebay_image_action(&path, "inspect").is_some() => {
                native_inspect_image(config, cratebay_image_action(&path, "inspect").unwrap())
            }
            ("POST", _) if cratebay_image_action(&path, "tag").is_some() => native_tag_image(
                config,
                cratebay_image_action(&path, "tag").unwrap(),
                &request.body,
            ),
            ("POST", _) if cratebay_image_action(&path, "remove").is_some() => native_remove_image(
                state,
                config,
                cratebay_image_action(&path, "remove").unwrap(),
                &request.body,
            ),
            ("GET", "/cratebay/networks") => list_cratebay_networks(config, &query),
            ("POST", "/cratebay/networks") => native_create_network(config, &request.body),
            ("GET", "/cratebay/volumes") => list_cratebay_volumes(),
            ("POST", "/cratebay/volumes") => native_create_volume(&request.body),
            ("GET", "/cratebay/pods") => list_cratebay_pods(state),
            ("GET", _) if cratebay_pod_id_path(&path).is_some() => {
                inspect_cratebay_pod(state, cratebay_pod_id_path(&path).unwrap())
            }
            ("POST", "/cratebay/pods") => native_create_pod(config, &request.body),
            ("POST", _) if cratebay_pod_action(&path, "attach").is_some() => {
                native_attach_container_to_pod(
                    state,
                    cratebay_pod_action(&path, "attach").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_pod_action(&path, "detach").is_some() => {
                native_detach_container_from_pod(
                    state,
                    cratebay_pod_action(&path, "detach").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_pod_action(&path, "remove").is_some() => native_remove_pod(
                state,
                config,
                cratebay_pod_action(&path, "remove").unwrap(),
                &request.body,
            ),
            ("POST", _) if cratebay_network_action(&path, "remove").is_some() => {
                native_remove_network(
                    state,
                    config,
                    cratebay_network_action(&path, "remove").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_volume_action(&path, "remove").is_some() => {
                native_remove_volume(
                    cratebay_volume_action(&path, "remove").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_shim_task_action(&path, "reap").is_some() => {
                native_reap_shim_task(
                    state,
                    cratebay_shim_task_action(&path, "reap").unwrap(),
                    &request.body,
                )
            }
            ("GET", _) if cratebay_network_id_path(&path).is_some() => {
                native_inspect_network(state, cratebay_network_id_path(&path).unwrap())
            }
            ("GET", _) if cratebay_volume_id_path(&path).is_some() => {
                native_inspect_volume(cratebay_volume_id_path(&path).unwrap())
            }
            ("POST", _) if cratebay_container_action(&path, "start").is_some() => {
                native_start_container(state, cratebay_container_action(&path, "start").unwrap())
            }
            ("POST", _) if cratebay_container_action(&path, "stop").is_some() => {
                native_stop_container(
                    state,
                    cratebay_container_action(&path, "stop").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_container_action(&path, "remove").is_some() => {
                native_remove_container(
                    state,
                    cratebay_container_action(&path, "remove").unwrap(),
                    &request.body,
                )
            }
            ("GET", _) if cratebay_container_action(&path, "inspect").is_some() => {
                native_inspect_container(
                    state,
                    cratebay_container_action(&path, "inspect").unwrap(),
                )
            }
            ("GET", _) if cratebay_container_action(&path, "logs").is_some() => {
                native_logs_container(
                    state,
                    cratebay_container_action(&path, "logs").unwrap(),
                    &query,
                )
            }
            ("GET", _) if cratebay_container_action(&path, "stats").is_some() => {
                native_stats_container(state, cratebay_container_action(&path, "stats").unwrap())
            }
            ("POST", _) if cratebay_container_action(&path, "wait").is_some() => {
                native_wait_container(
                    state,
                    cratebay_container_action(&path, "wait").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_container_action(&path, "exec").is_some() => {
                native_exec_container(
                    state,
                    cratebay_container_action(&path, "exec").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_container_terminal_action(&path, "open").is_some() => {
                native_terminal_open(
                    state,
                    cratebay_container_terminal_action(&path, "open").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_container_terminal_action(&path, "input").is_some() => {
                native_terminal_input(
                    state,
                    cratebay_container_terminal_action(&path, "input").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_container_terminal_action(&path, "read").is_some() => {
                native_terminal_read(
                    state,
                    cratebay_container_terminal_action(&path, "read").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_container_terminal_action(&path, "resize").is_some() => {
                native_terminal_resize(
                    state,
                    cratebay_container_terminal_action(&path, "resize").unwrap(),
                    &request.body,
                )
            }
            ("POST", _) if cratebay_container_terminal_action(&path, "close").is_some() => {
                native_terminal_close(
                    state,
                    cratebay_container_terminal_action(&path, "close").unwrap(),
                    &request.body,
                )
            }
            ("GET", "/version") => json_response(200, version_payload(config)),
            ("GET", "/info") => json_response(200, info_payload(config)),
            ("GET", "/containers/json") => match list_containers(state) {
                Ok(containers) => json_response(200, containers),
                Err(error) => error_response(500, "container list failed", error),
            },
            ("POST", "/containers/create") => create_container(state, &query, &request.body),
            ("POST", _) if container_action(&path, "start").is_some() => {
                start_container(state, container_action(&path, "start").unwrap())
            }
            ("POST", _) if container_action(&path, "stop").is_some() => {
                stop_container(state, container_action(&path, "stop").unwrap(), &query)
            }
            ("POST", _) if container_action(&path, "rename").is_some() => {
                rename_container(state, container_action(&path, "rename").unwrap(), &query)
            }
            ("DELETE", _) if container_id_path(&path).is_some() => {
                remove_container(state, container_id_path(&path).unwrap(), &query)
            }
            ("GET", _) if container_action(&path, "json").is_some() => {
                inspect_container(state, container_action(&path, "json").unwrap())
            }
            ("PUT", _) if container_action(&path, "archive").is_some() => put_container_archive(
                state,
                container_action(&path, "archive").unwrap(),
                &query,
                &request.body,
            ),
            ("GET", _) if container_action(&path, "logs").is_some() => {
                logs_container(state, container_action(&path, "logs").unwrap(), &query)
            }
            ("POST", _) if container_action(&path, "attach").is_some() => {
                attach_container(state, container_action(&path, "attach").unwrap(), &query)
            }
            ("POST", _) if container_action(&path, "wait").is_some() => {
                wait_container(state, container_action(&path, "wait").unwrap())
            }
            ("GET", _) if container_action(&path, "stats").is_some() => {
                stats_container(state, container_action(&path, "stats").unwrap())
            }
            ("POST", _) if container_action(&path, "exec").is_some() => create_exec(
                state,
                container_action(&path, "exec").unwrap(),
                &request.body,
            ),
            ("POST", _) if exec_action(&path, "start").is_some() => {
                start_exec(state, exec_action(&path, "start").unwrap())
            }
            ("GET", _) if exec_action(&path, "json").is_some() => {
                inspect_exec(state, exec_action(&path, "json").unwrap())
            }
            ("GET", "/images/json") => list_images(config),
            ("POST", "/images/create") => pull_image(config, &query),
            ("POST", "/images/load") => load_image(config, &request.body),
            ("GET", "/images/get") => export_images(config, &request.path, &query),
            ("GET", _) if image_action(&path, "get").is_some() => {
                export_image_names(config, vec![image_action(&path, "get").unwrap()])
            }
            ("POST", "/commit") => commit_container(config, &query),
            ("POST", "/volumes/create") => create_volume(&request.body),
            ("GET", "/volumes") => list_volumes(),
            ("GET", _) if volume_id_path(&path).is_some() => {
                inspect_volume(volume_id_path(&path).unwrap())
            }
            ("DELETE", _) if volume_id_path(&path).is_some() => {
                remove_volume(volume_id_path(&path).unwrap())
            }
            ("GET", _) if image_action(&path, "json").is_some() => {
                inspect_image(config, image_action(&path, "json").unwrap())
            }
            ("POST", _) if image_action(&path, "tag").is_some() => {
                tag_image(config, image_action(&path, "tag").unwrap(), &query)
            }
            ("DELETE", _) if image_id_path(&path).is_some() => {
                remove_image(config, image_id_path(&path).unwrap(), &query)
            }
            ("GET", "/networks") => list_networks(config, &query),
            ("POST", "/networks/create") => create_network(config, &request.body),
            ("POST", _) if network_action(&path, "connect").is_some() => connect_network(
                state,
                network_action(&path, "connect").unwrap(),
                &request.body,
            ),
            ("POST", _) if network_action(&path, "disconnect").is_some() => disconnect_network(
                state,
                network_action(&path, "disconnect").unwrap(),
                &request.body,
            ),
            ("GET", _) if network_id_path(&path).is_some() => {
                inspect_network(state, network_id_path(&path).unwrap())
            }
            ("DELETE", _) if network_id_path(&path).is_some() => {
                remove_network(config, network_id_path(&path).unwrap())
            }
            _ => unsupported_response(request, path),
        }
    }

    #[cfg(test)]
    fn normalize_docker_path(raw: &str) -> String {
        normalize_docker_request_path(raw).0
    }

    fn normalize_docker_request_path(raw: &str) -> (String, HashMap<String, String>) {
        let (path, query) = raw.split_once('?').unwrap_or((raw, ""));
        let mut parts = path.split('/').filter(|part| !part.is_empty());
        let normalized_path = match parts.next() {
            Some(version) if version.starts_with('v') && version[1..].contains('.') => {
                format!("/{}", parts.collect::<Vec<_>>().join("/"))
            }
            _ => path.to_string(),
        };
        (normalized_path, parse_query(query))
    }

    fn parse_query(raw: &str) -> HashMap<String, String> {
        raw.split('&')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let (key, value) = part.split_once('=').unwrap_or((part, ""));
                (percent_decode(key), percent_decode(value))
            })
            .collect()
    }

    fn query_values(raw_path: &str, expected_key: &str) -> Vec<String> {
        let Some((_, raw_query)) = raw_path.split_once('?') else {
            return Vec::new();
        };

        raw_query
            .split('&')
            .filter(|part| !part.is_empty())
            .filter_map(|part| {
                let (key, value) = part.split_once('=').unwrap_or((part, ""));
                if percent_decode(key) == expected_key {
                    Some(percent_decode(value))
                } else {
                    None
                }
            })
            .collect()
    }

    fn expand_image_names_value(value: String) -> Vec<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
            return items
                .into_iter()
                .filter_map(|item| optional_string_value(Some(&item)))
                .collect();
        }

        trimmed
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn percent_decode(raw: &str) -> String {
        let bytes = raw.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            match bytes[index] {
                b'+' => {
                    out.push(b' ');
                    index += 1;
                }
                b'%' if index + 2 < bytes.len() => {
                    let hex = &raw[index + 1..index + 3];
                    if let Ok(value) = u8::from_str_radix(hex, 16) {
                        out.push(value);
                        index += 3;
                    } else {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
                byte => {
                    out.push(byte);
                    index += 1;
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn container_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["containers", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn cratebay_container_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "containers", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn cratebay_container_terminal_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "containers", id, "terminal", found] if *found == action => {
                Some(percent_decode(id))
            }
            _ => None,
        }
    }

    fn cratebay_network_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "networks", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn cratebay_network_id_path(path: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "networks", id] => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn cratebay_volume_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "volumes", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn cratebay_volume_id_path(path: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "volumes", id] => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn cratebay_shim_task_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "shim", "tasks", id, found] if *found == action => {
                Some(percent_decode(id))
            }
            _ => None,
        }
    }

    fn cratebay_pod_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "pods", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn cratebay_pod_id_path(path: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "pods", id] => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn container_id_path(path: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["containers", id] => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn exec_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["exec", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn image_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        if parts.len() >= 3
            && parts.first().copied() == Some("images")
            && parts.last().copied() == Some(action)
        {
            return Some(percent_decode(&parts[1..parts.len() - 1].join("/")));
        }
        None
    }

    fn image_id_path(path: &str) -> Option<String> {
        let parts = path_segments(path);
        if parts.len() >= 2 && parts.first().copied() == Some("images") {
            return Some(percent_decode(&parts[1..].join("/")));
        }
        None
    }

    fn cratebay_image_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["cratebay", "images", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn network_action(path: &str, action: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["networks", id, found] if *found == action => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn network_id_path(path: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["networks", id] if *id != "create" => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn volume_id_path(path: &str) -> Option<String> {
        let parts = path_segments(path);
        match parts.as_slice() {
            ["volumes", id] if *id != "create" => Some(percent_decode(id)),
            _ => None,
        }
    }

    fn path_segments(path: &str) -> Vec<&str> {
        path.split('/').filter(|part| !part.is_empty()).collect()
    }

    fn create_container(
        state: &AdapterState,
        query: &HashMap<String, String>,
        body: &[u8],
    ) -> HttpResponse {
        let payload = parse_json_body(body);
        let name = query
            .get("name")
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| unique_task_id("cratebay"));
        if !valid_local_name(&name) {
            return error_response(
                400,
                "container name is invalid",
                json!({ "name": name, "allowed": "ASCII letters, numbers, dot, dash, underscore" }),
            );
        }
        let image = string_value(payload.get("Image"));
        if image.is_empty() {
            return error_response(400, "container image is required", json!({ "name": name }));
        }
        if pending_container(state, &name).is_some() {
            return error_response(
                409,
                "container already exists",
                json!({ "name": name, "backend": "containerd-pending" }),
            );
        }

        cleanup_containerd_name_artifacts(&state.config, &name);
        let pending = pending_from_create_payload_with_config(Some(&state.config), &name, &payload);
        let id = pending.id.clone();
        if let Err(error) = store_pending_container(state, &id, &name, pending) {
            return error_response(500, "container create registry write failed", error);
        }
        json_response(
            201,
            json!({
                "Id": id,
                "Warnings": [],
                "CrateBay": {
                    "backend": "containerd-pending",
                    "engine": "containerd",
                },
            }),
        )
    }

    fn start_container(state: &AdapterState, id: String) -> HttpResponse {
        if let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        {
            let pending = refresh_pending_task_state(state, pending);
            if pending.started_with_ctr && pending.exit_code.is_none() {
                return empty_response(204);
            }
            let pending = reset_pending_runtime_state_for_start(state, pending);

            let _ = fs::create_dir_all(
                pending
                    .log_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("/tmp")),
            );
            let _ = fs::remove_file(&pending.log_path);
            let create_args =
                build_ctr_container_create_args(&pending, &ctr_image_for_run(&pending.image));
            let start_args = build_ctr_task_start_args(&pending);
            return match spawn_ctr_runner(state.clone(), id.clone(), pending.clone()) {
                Ok(_) => {
                    mark_pending_started_with_ctr(state, &id, &pending.name);
                    if let Some(network) = pending.network.as_deref() {
                        refresh_network_hosts_for_network(state, network);
                    }
                    empty_response(204)
                }
                Err(error) => error_response(
                    500,
                    "container start failed",
                    json!({
                        "backend": "containerd",
                        "createArgs": create_args,
                        "startArgs": start_args,
                        "error": error,
                    }),
                ),
            };
        }

        managed_container_not_found("container start failed", &id)
    }

    fn store_pending_container(
        state: &AdapterState,
        id: &str,
        name: &str,
        pending: PendingContainer,
    ) -> Result<(), Value> {
        cache_pending_container(state, id, name, pending.clone());
        write_pending_container_record(&pending)
    }

    fn stop_container(
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        if let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        {
            if !pending.started_with_ctr || pending.exit_code.is_some() {
                return empty_response(204);
            }
            let task_name = containerd_task_name(&pending).to_string();
            let term_output = run_ctr_allow_failure(
                &state.config,
                vec![
                    "tasks".to_string(),
                    "kill".to_string(),
                    "--signal".to_string(),
                    "TERM".to_string(),
                    task_name.clone(),
                ],
            );
            if wait_for_pending_exit_code_with_timeout(
                state,
                &pending.id,
                &pending.name,
                Duration::from_secs(3),
            )
            .is_some()
            {
                return empty_response(204);
            }

            let kill_output = run_ctr_allow_failure(
                &state.config,
                vec![
                    "tasks".to_string(),
                    "kill".to_string(),
                    "--signal".to_string(),
                    "KILL".to_string(),
                    task_name,
                ],
            );
            if wait_for_pending_exit_code_with_timeout(
                state,
                &pending.id,
                &pending.name,
                Duration::from_secs(10),
            )
            .is_some()
            {
                return empty_response(204);
            }

            return error_response(
                500,
                "container stop failed",
                json!({
                    "container": id,
                    "backend": "containerd",
                    "term": term_output.as_ref().map(ctr_output_value).unwrap_or_else(|error| error.clone()),
                    "kill": kill_output.as_ref().map(ctr_output_value).unwrap_or_else(|error| error.clone()),
                    "error": "container task did not exit after TERM/KILL",
                }),
            );
        }

        let _ = query;
        managed_container_not_found("container stop failed", &id)
    }

    fn rename_container(
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        let Some(new_name) = query
            .get("name")
            .filter(|value| !value.trim().is_empty())
            .cloned()
        else {
            return error_response(
                400,
                "container rename requires name",
                json!({ "container": id }),
            );
        };
        if !valid_local_name(&new_name) {
            return error_response(
                400,
                "container rename target is invalid",
                json!({ "container": id, "name": new_name }),
            );
        }
        if let Some(existing) = pending_container(state, &new_name) {
            if existing.id != id && existing.name != id {
                return error_response(
                    409,
                    "container rename target already exists",
                    json!({ "container": id, "name": new_name }),
                );
            }
        }

        let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        else {
            return managed_container_not_found("container rename failed", &id);
        };
        let old_id = pending.id.clone();
        let old_name = pending.name.clone();
        let mut renamed = pending;
        renamed.name = new_name.clone();
        renamed
            .aliases
            .retain(|alias| alias != &old_name && alias != &new_name);
        push_host_alias(&mut renamed.aliases, &new_name);
        strip_completed_compose_replace_label(
            &renamed.name,
            &renamed.runtime_id,
            &mut renamed.labels,
        );
        remove_pending_container(state, &old_id, &old_name);
        let renamed_name = renamed.name.clone();
        match store_pending_container(state, &old_id, &renamed_name, renamed) {
            Ok(()) => empty_response(204),
            Err(error) => error_response(500, "container rename registry write failed", error),
        }
    }

    fn remove_container(
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        let force = query_bool_or(query, "force", false);
        if let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        {
            if pending.started_with_ctr {
                if pending.exit_code.is_none() {
                    if !force
                        && wait_for_pending_exit_code_with_timeout(
                            state,
                            &pending.id,
                            &pending.name,
                            remove_non_force_wait_timeout(),
                        )
                        .is_none()
                    {
                        return error_response(
                            409,
                            "container remove failed",
                            json!({
                                "container": id,
                                "state": "running",
                                "forceRequired": true,
                            }),
                        );
                    }
                    if force {
                        wait_for_pending_exit_code_with_timeout(
                            state,
                            &pending.id,
                            &pending.name,
                            Duration::from_secs(3),
                        );
                    }
                }
                cleanup_containerd_pending_artifacts(&state.config, &pending);
            }
            let _ = fs::remove_file(&pending.log_path);
            remove_pending_container(state, &pending.id, &pending.name);
            return empty_response(204);
        }

        managed_container_not_found("container remove failed", &id)
    }

    fn put_container_archive(
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
        body: &[u8],
    ) -> HttpResponse {
        let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        else {
            return managed_container_not_found("container archive extract failed", &id);
        };
        if pending.started_with_ctr {
            return error_response(
                409,
                "container archive extract is only supported before start",
                json!({
                    "container": id,
                    "state": pending_state(&pending).0,
                    "backend": "containerd",
                }),
            );
        }
        let Some(target) = archive_target_path(query) else {
            return error_response(
                400,
                "container archive extract path is required",
                json!({ "container": id, "query": query }),
            );
        };
        if body.is_empty() {
            return error_response(
                400,
                "container archive body is empty",
                json!({ "container": id, "path": target }),
            );
        }

        let stage_dir = pending_archive_stage_path(&pending.name, &target);
        if let Err(error) = fs::remove_dir_all(&stage_dir) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return error_response(
                    500,
                    "container archive staging cleanup failed",
                    json!({
                        "container": id,
                        "path": target,
                        "stageDir": stage_dir.display().to_string(),
                        "error": error.to_string(),
                    }),
                );
            }
        }
        if let Err(error) = fs::create_dir_all(&stage_dir) {
            return error_response(
                500,
                "container archive staging failed",
                json!({
                    "container": id,
                    "path": target,
                    "stageDir": stage_dir.display().to_string(),
                    "error": error.to_string(),
                }),
            );
        }
        let mut archive = tar::Archive::new(Cursor::new(body));
        if let Err(error) = archive.unpack(&stage_dir) {
            let _ = fs::remove_dir_all(&stage_dir);
            return error_response(
                500,
                "container archive extract failed",
                json!({
                    "container": id,
                    "path": target,
                    "stageDir": stage_dir.display().to_string(),
                    "error": error.to_string(),
                }),
            );
        }

        let archive_mounts = pending_archive_mounts(&stage_dir, &target);
        update_pending_container(state, &pending.id, &pending.name, |pending| {
            let archive_root = pending_archive_root().to_string_lossy().to_string();
            let archive_targets = archive_mounts
                .iter()
                .map(|mount| mount.target.clone())
                .collect::<HashSet<_>>();
            pending.mounts.retain(|mount| {
                !(archive_targets.contains(&mount.target)
                    && mount.source.starts_with(&archive_root))
            });
            pending.mounts.extend(archive_mounts.clone());
        });

        json_response(
            200,
            json!({
                "message": "archive extracted",
                "CrateBay": {
                    "backend": "containerd-pending",
                    "container": pending.name,
                    "path": target,
                    "stageDir": stage_dir.display().to_string(),
                },
            }),
        )
    }

    fn native_pull_image(config: &Config, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some(image) = native_image_ref(&payload) else {
            return error_response(
                400,
                "native image pull requires image",
                json!({ "api": "cratebay.image.pull.v1" }),
            );
        };

        match pull_image_with_engine(config, &image, &[]) {
            Ok(result) => json_response(
                200,
                json!({
                    "api": "cratebay.image.pull.v1",
                    "image": image,
                    "imageRef": result.image_ref,
                    "mirror": result.mirror,
                    "pulled": true,
                    "backend": result.backend,
                    "stdout": String::from_utf8_lossy(&result.output.stdout).into_owned(),
                    "stderr": String::from_utf8_lossy(&result.output.stderr).into_owned(),
                    "containerdErrors": result.containerd_errors,
                }),
            ),
            Err(error) => error_response(500, "native image pull failed", error),
        }
    }

    fn native_export_images(
        config: &Config,
        raw_path: &str,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        export_images(config, raw_path, query)
    }

    fn native_import_image(config: &Config, body: &[u8]) -> HttpResponse {
        if body.is_empty() {
            return error_response(
                400,
                "native image import body is empty",
                json!({ "api": "cratebay.image.import.v1" }),
            );
        }

        let archive_path = temp_archive_path("cratebay-native-image-import");
        if let Err(error) = fs::write(&archive_path, body) {
            return error_response(
                500,
                "native image import failed",
                json!({
                    "api": "cratebay.image.import.v1",
                    "path": archive_path.display().to_string(),
                    "error": error.to_string(),
                }),
            );
        }

        let response = native_import_image_path(config, &archive_path);
        let _ = fs::remove_file(&archive_path);
        response
    }

    fn native_import_image_path(config: &Config, archive_path: &Path) -> HttpResponse {
        let result = run_ctr(
            config,
            vec![
                "images".to_string(),
                "import".to_string(),
                archive_path.display().to_string(),
            ],
        );
        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let images = stdout
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                json_response(
                    200,
                    json!({
                        "api": "cratebay.image.import.v1",
                        "backend": "containerd",
                        "managedBy": "cratebay",
                        "imported": true,
                        "images": images,
                        "stdout": stdout,
                        "stderr": stderr,
                    }),
                )
            }
            Err(error) => error_response(
                500,
                "native image import failed",
                json!({
                    "api": "cratebay.image.import.v1",
                    "path": archive_path.display().to_string(),
                    "backend": "containerd",
                    "error": error,
                }),
            ),
        }
    }

    fn native_inspect_image(config: &Config, id: String) -> HttpResponse {
        match native_image_inspect_value(config, &id) {
            Ok(payload) => json_response(200, payload),
            Err(error) => error_response(404, "native image inspect failed", error),
        }
    }

    fn native_remove_image(
        state: &AdapterState,
        config: &Config,
        id: String,
        body: &[u8],
    ) -> HttpResponse {
        let payload = parse_json_body(body);
        let force = bool_value(payload.get("force").or_else(|| payload.get("Force")));
        let image_ref = match resolve_containerd_image_ref(config, &id) {
            Ok(image_ref) => image_ref,
            Err(_) => id.clone(),
        };
        let references = image_container_references(state, &image_ref);
        if !force && !references.is_empty() {
            return error_response(
                409,
                "image is in use by CrateBay containers",
                json!({
                    "api": "cratebay.image.remove.v1",
                    "id": id,
                    "imageRef": image_ref,
                    "forceRequired": true,
                    "containers": references,
                }),
            );
        }
        match run_ctr(
            config,
            vec!["images".to_string(), "rm".to_string(), image_ref.clone()],
        ) {
            Ok(output) => json_response(
                200,
                json!({
                    "api": "cratebay.image.remove.v1",
                    "id": id,
                    "imageRef": image_ref,
                    "backend": "containerd",
                    "managedBy": "cratebay",
                    "removed": true,
                    "force": force,
                    "stdout": String::from_utf8_lossy(&output.stdout).into_owned(),
                    "stderr": String::from_utf8_lossy(&output.stderr).into_owned(),
                }),
            ),
            Err(error) => error_response(
                500,
                "native image remove failed",
                json!({
                    "api": "cratebay.image.remove.v1",
                    "backend": "containerd",
                    "id": id,
                    "imageRef": image_ref,
                    "force": force,
                    "error": error,
                }),
            ),
        }
    }

    fn image_container_references(state: &AdapterState, image_ref: &str) -> Vec<Value> {
        let mut references = unique_pending_containers(state)
            .into_iter()
            .filter(|pending| pending_references_image(pending, image_ref))
            .map(|pending| {
                json!({
                    "id": pending.id,
                    "name": pending.name,
                    "image": pending.image,
                })
            })
            .collect::<Vec<_>>();
        references.sort_by(|a, b| {
            string_field(a, &["name"])
                .unwrap_or_default()
                .cmp(&string_field(b, &["name"]).unwrap_or_default())
        });
        references
    }

    fn pending_references_image(pending: &PendingContainer, image_ref: &str) -> bool {
        let mut candidates = vec![pending.image.clone(), pending_image_id(pending)];
        candidates.sort();
        candidates.dedup();
        candidates
            .into_iter()
            .any(|candidate| image_ref_matches(image_ref, &candidate))
    }

    fn native_tag_image(config: &Config, source: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let target = string_value(
            payload
                .get("target")
                .or_else(|| payload.get("image"))
                .or_else(|| payload.get("repo")),
        );
        if target.trim().is_empty() {
            return error_response(
                400,
                "native image tag requires target",
                json!({ "api": "cratebay.image.tag.v1", "source": source }),
            );
        }

        let source_ref = match resolve_containerd_image_ref(config, &source) {
            Ok(source_ref) => source_ref,
            Err(error) => return error_response(404, "native image tag failed", error),
        };
        let target_ref = ctr_image_for_run(&target);

        match run_ctr(
            config,
            vec![
                "images".to_string(),
                "tag".to_string(),
                source_ref.clone(),
                target_ref.clone(),
            ],
        ) {
            Ok(output) => json_response(
                200,
                json!({
                    "api": "cratebay.image.tag.v1",
                    "source": source,
                    "sourceRef": source_ref,
                    "target": target,
                    "targetRef": target_ref,
                    "backend": "containerd",
                    "managedBy": "cratebay",
                    "tagged": true,
                    "stdout": String::from_utf8_lossy(&output.stdout).into_owned(),
                    "stderr": String::from_utf8_lossy(&output.stderr).into_owned(),
                }),
            ),
            Err(error) => error_response(
                500,
                "native image tag failed",
                json!({
                    "api": "cratebay.image.tag.v1",
                    "backend": "containerd",
                    "source": source,
                    "sourceRef": source_ref,
                    "target": target,
                    "targetRef": target_ref,
                    "error": error,
                }),
            ),
        }
    }

    fn native_pack_container_image(config: &Config, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let container = string_value(
            payload
                .get("container")
                .or_else(|| payload.get("Container")),
        );
        let image = string_value(
            payload
                .get("image")
                .or_else(|| payload.get("target"))
                .or_else(|| payload.get("Image")),
        );
        if container.trim().is_empty() || image.trim().is_empty() {
            return error_response(
                400,
                "native image pack requires container and image",
                json!({
                    "api": "cratebay.image.pack.v1",
                    "container": container,
                    "image": image,
                }),
            );
        }

        let Some(pending) = read_pending_container_record(&container) else {
            return error_response(
                404,
                "native image pack failed",
                json!({
                    "api": "cratebay.image.pack.v1",
                    "backend": "containerd",
                    "container": container,
                    "image": image,
                    "error": "CrateBay-managed container was not found",
                }),
            );
        };

        match commit_container_rootfs_to_image(config, &pending, &image) {
            Ok(result) => json_response(
                200,
                native_pack_container_image_payload(&pending, &image, result),
            ),
            Err(error) => error_response(
                500,
                "native image pack failed",
                json!({
                    "api": "cratebay.image.pack.v1",
                    "backend": "containerd",
                    "container": pending.name,
                    "image": image,
                    "error": error,
                }),
            ),
        }
    }

    fn native_pack_container_image_payload(
        pending: &PendingContainer,
        image: &str,
        result: ContainerCommitResult,
    ) -> Value {
        json!({
            "api": "cratebay.image.pack.v1",
            "backend": "containerd",
            "managedBy": "cratebay",
            "container": pending.name,
            "image": image,
            "imageRef": result.target_ref,
            "packed": true,
            "mode": "rootfs-archive",
            "layerDigest": format!("sha256:{}", result.layer_digest),
            "configDigest": format!("sha256:{}", result.config_digest),
            "rootfs": result.rootfs.display().to_string(),
        })
    }

    fn native_image_inspect_value(config: &Config, id: &str) -> Result<Value, Value> {
        let image_ref = resolve_containerd_image_ref(config, id)?;
        let inspect = normalize_image_inspect(ctr_image_summary(&image_ref), id);
        Ok(native_image_inspect_payload(id, &image_ref, inspect))
    }

    fn native_image_inspect_payload(id: &str, image_ref: &str, inspect: Value) -> Value {
        let repo_tags = string_array(inspect.get("RepoTags"));
        let repo_digests = string_array(inspect.get("RepoDigests"));
        let size_bytes = size_bytes(inspect.get("Size"));
        let layers = inspect
            .get("RootFS")
            .and_then(|root| root.get("Layers"))
            .and_then(Value::as_array)
            .map(|layers| layers.len())
            .unwrap_or_default();
        json!({
            "api": "cratebay.image.inspect.v1",
            "id": string_field(&inspect, &["Id"]).unwrap_or_else(|| id.to_string()),
            "imageRef": image_ref,
            "repoTags": repo_tags,
            "repoDigests": repo_digests,
            "sizeBytes": size_bytes,
            "created": string_field(&inspect, &["Created"]).unwrap_or_else(chrono_like_now),
            "architecture": string_field(&inspect, &["Architecture"]).unwrap_or_else(|| std::env::consts::ARCH.to_string()),
            "os": string_field(&inspect, &["Os", "OS"]).unwrap_or_else(|| "linux".to_string()),
            "runtimeVersion": "cratebay-containerd",
            "dockerVersion": "cratebay-containerd",
            "layers": layers,
            "backend": "containerd",
            "managedBy": "cratebay",
            "inspect": inspect,
        })
    }

    fn native_create_network(config: &Config, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some((name, create_payload)) = native_network_create_request(&payload) else {
            return error_response(
                400,
                "native network create requires name",
                json!({ "api": "cratebay.network.create.v1" }),
            );
        };
        match create_managed_network(config, &create_payload) {
            Ok(created) => {
                let id = string_value(created.get("Id"));
                json_response(
                    200,
                    json!({
                        "api": "cratebay.network.create.v1",
                        "id": if id.is_empty() { name.clone() } else { id },
                        "name": name,
                        "backend": "cratebay-cni",
                        "created": true,
                    }),
                )
            }
            Err(error) => error_response(500, "native network create failed", error),
        }
    }

    fn native_remove_network(
        state: &AdapterState,
        config: &Config,
        id: String,
        body: &[u8],
    ) -> HttpResponse {
        let force = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|payload| {
                payload
                    .get("force")
                    .or_else(|| payload.get("Force"))
                    .cloned()
            })
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let references = network_container_references(&id);
        if !force && !references.is_empty() {
            return error_response(
                409,
                "network is in use by CrateBay containers",
                json!({
                    "api": "cratebay.network.remove.v1",
                    "id": id,
                    "forceRequired": true,
                    "containers": references,
                }),
            );
        }
        match remove_managed_network(config, &id) {
            Ok(()) => {
                if force {
                    detach_network_from_referenced_containers(state, &id);
                }
                json_response(
                    200,
                    json!({
                        "api": "cratebay.network.remove.v1",
                        "id": id,
                        "force": force,
                        "removed": true,
                    }),
                )
            }
            Err(error) => error_response(500, "native network remove failed", error),
        }
    }

    fn network_container_references(id: &str) -> Vec<Value> {
        let mut references = pending_container_records()
            .into_iter()
            .filter(|pending| pending.network.as_deref() == Some(id))
            .map(|pending| {
                json!({
                    "id": pending.id,
                    "name": pending.name,
                })
            })
            .collect::<Vec<_>>();
        references.sort_by(|a, b| {
            string_field(a, &["name"])
                .unwrap_or_default()
                .cmp(&string_field(b, &["name"]).unwrap_or_default())
        });
        references
    }

    fn detach_network_from_referenced_containers(state: &AdapterState, id: &str) {
        for pending in pending_container_records()
            .into_iter()
            .filter(|pending| pending.network.as_deref() == Some(id))
        {
            update_pending_container(state, &pending.id, &pending.name, |detached| {
                detached.network = None;
                detached.aliases.clear();
                detached.netns_name = None;
                detached.netns_path = None;
            });
        }
    }

    fn native_inspect_network(state: &AdapterState, id: String) -> HttpResponse {
        match inspect_network_value(&state.config, &id) {
            Ok(value) => {
                let inspect = normalize_network_inspect_with_pending(state, value, &id);
                json_response(
                    200,
                    json!({
                        "api": "cratebay.network.inspect.v1",
                        "backend": "cratebay-cni",
                        "managedBy": "cratebay",
                        "id": string_field(&inspect, &["Id"]).unwrap_or_else(|| id.clone()),
                        "name": string_field(&inspect, &["Name"]).unwrap_or_else(|| id.clone()),
                        "item": super::native_contract::network_summary(inspect.clone()),
                        "inspect": inspect,
                    }),
                )
            }
            Err(error) => error_response(404, "native network inspect failed", error),
        }
    }

    fn native_create_volume(body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some((name, create_payload)) = native_volume_create_request(&payload) else {
            return error_response(
                400,
                "native volume create requires name",
                json!({ "api": "cratebay.volume.create.v1" }),
            );
        };
        let body = match serde_json::to_vec(&create_payload) {
            Ok(body) => body,
            Err(error) => {
                return error_response(
                    500,
                    "native volume create payload encode failed",
                    json!({ "error": error.to_string() }),
                );
            }
        };
        let response = create_volume(&body);
        if !(200..300).contains(&response.status) {
            return response;
        }
        let created = parse_json_body(&response.body);
        json_response(
            200,
            json!({
                "api": "cratebay.volume.create.v1",
                "name": name,
                "created": true,
                "item": native_contract::volume_summary(created),
            }),
        )
    }

    fn native_remove_volume(name: String, body: &[u8]) -> HttpResponse {
        let force = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|payload| {
                payload
                    .get("force")
                    .or_else(|| payload.get("Force"))
                    .cloned()
            })
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !force {
            let references = volume_container_references(&name);
            if !references.is_empty() {
                return error_response(
                    409,
                    "volume is in use by CrateBay containers",
                    json!({
                        "api": "cratebay.volume.remove.v1",
                        "name": name,
                        "forceRequired": true,
                        "containers": references,
                    }),
                );
            }
        }
        let response = remove_volume(name.clone());
        if !(200..300).contains(&response.status) {
            return response;
        }
        json_response(
            200,
            json!({
                "api": "cratebay.volume.remove.v1",
                "name": name,
                "force": force,
                "removed": true,
            }),
        )
    }

    fn volume_container_references(name: &str) -> Vec<Value> {
        let expected = volume_data_path(name);
        let mut references = pending_container_records()
            .into_iter()
            .filter(|pending| {
                pending
                    .mounts
                    .iter()
                    .any(|mount| Path::new(&mount.source) == expected.as_path())
            })
            .map(|pending| {
                json!({
                    "id": pending.id,
                    "name": pending.name,
                })
            })
            .collect::<Vec<_>>();
        references.sort_by(|a, b| {
            string_field(a, &["name"])
                .unwrap_or_default()
                .cmp(&string_field(b, &["name"]).unwrap_or_default())
        });
        references
    }

    fn native_inspect_volume(name: String) -> HttpResponse {
        if !valid_local_name(&name) || !volume_data_path(&name).exists() {
            return error_response(
                404,
                "native volume inspect failed",
                json!({ "name": name, "backend": "cratebay-storage" }),
            );
        }
        let mut item = super::native_contract::volume_summary(volume_value(&name));
        if let Some(object) = item.as_object_mut() {
            object.insert(
                "dataPath".to_string(),
                json!(volume_data_path(&name).display().to_string()),
            );
            object.insert(
                "sizeBytes".to_string(),
                json!(directory_size_bytes(&volume_data_path(&name))),
            );
            object.insert("managedBy".to_string(), json!("cratebay"));
        }
        json_response(
            200,
            json!({
                "api": "cratebay.volume.inspect.v1",
                "backend": "cratebay-storage",
                "managedBy": "cratebay",
                "name": name,
                "item": item,
            }),
        )
    }

    fn list_cratebay_pods(state: &AdapterState) -> HttpResponse {
        let mut items = managed_network_values()
            .into_iter()
            .filter(is_cratebay_pod_network)
            .map(|pod| {
                let id = string_value(pod.get("Id").or_else(|| pod.get("Name")));
                native_contract::pod_summary(normalize_network_inspect_with_pending(
                    state, pod, &id,
                ))
            })
            .collect::<Vec<_>>();
        dedupe_native_items_by_name(&mut items);

        json_response(
            200,
            json!({
                "api": "cratebay.pods.v1",
                "count": items.len(),
                "items": items,
            }),
        )
    }

    fn inspect_cratebay_pod(state: &AdapterState, name: String) -> HttpResponse {
        let Some(pod) = managed_network_value_by_id(&name).filter(is_cratebay_pod_network) else {
            return error_response(
                404,
                "CrateBay pod not found",
                json!({
                    "api": "cratebay.pod.inspect.v1",
                    "name": name,
                    "backend": "cratebay-cni",
                }),
            );
        };
        json_response(
            200,
            json!({
                "api": "cratebay.pod.inspect.v1",
                "item": native_contract::pod_summary(
                    normalize_network_inspect_with_pending(state, pod, &name)
                ),
            }),
        )
    }

    fn native_create_pod(config: &Config, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some((name, create_payload)) = native_pod_create_request(&payload) else {
            return error_response(
                400,
                "native pod create requires name",
                json!({ "api": "cratebay.pod.create.v1" }),
            );
        };
        match create_managed_network(config, &create_payload) {
            Ok(created) => {
                let id = string_value(created.get("Id"));
                json_response(
                    200,
                    json!({
                        "api": "cratebay.pod.create.v1",
                        "id": if id.is_empty() { name.clone() } else { id },
                        "name": name,
                        "driver": string_value(create_payload.get("Driver")),
                        "backend": "cratebay-cni",
                        "created": true,
                    }),
                )
            }
            Err(error) => error_response(500, "native pod create failed", error),
        }
    }

    fn native_remove_pod(
        state: &AdapterState,
        config: &Config,
        name: String,
        body: &[u8],
    ) -> HttpResponse {
        let Some(_) = managed_network_value_by_id(&name).filter(is_cratebay_pod_network) else {
            return error_response(
                404,
                "CrateBay pod not found",
                json!({
                    "api": "cratebay.pod.remove.v1",
                    "name": name,
                    "backend": "cratebay-cni",
                }),
            );
        };
        let force = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|payload| {
                payload
                    .get("force")
                    .or_else(|| payload.get("Force"))
                    .cloned()
            })
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let references = network_container_references(&name);
        if !force && !references.is_empty() {
            return error_response(
                409,
                "pod is in use by CrateBay containers",
                json!({
                    "api": "cratebay.pod.remove.v1",
                    "name": name,
                    "forceRequired": true,
                    "containers": references,
                }),
            );
        }
        match remove_managed_network(config, &name) {
            Ok(()) => {
                if force {
                    detach_network_from_referenced_containers(state, &name);
                }
                json_response(
                    200,
                    json!({
                        "api": "cratebay.pod.remove.v1",
                        "name": name,
                        "force": force,
                        "removed": true,
                    }),
                )
            }
            Err(error) => error_response(500, "native pod remove failed", error),
        }
    }

    fn native_attach_container_to_pod(
        state: &AdapterState,
        pod_name: String,
        body: &[u8],
    ) -> HttpResponse {
        let payload = parse_json_body(body);
        let container = optional_string_value(
            payload
                .get("container")
                .or_else(|| payload.get("Container"))
                .or_else(|| payload.get("id")),
        )
        .unwrap_or_default();
        if container.is_empty() {
            return error_response(
                400,
                "native pod attach requires container",
                json!({ "api": "cratebay.pod.attach.v1", "pod": pod_name }),
            );
        }
        if managed_network_value_by_id(&pod_name)
            .filter(is_cratebay_pod_network)
            .is_none()
        {
            return error_response(
                404,
                "CrateBay pod not found",
                json!({
                    "api": "cratebay.pod.attach.v1",
                    "pod": pod_name,
                    "container": container,
                    "backend": "cratebay-cni",
                }),
            );
        }

        let response = connect_network(
            state,
            pod_name.clone(),
            serde_json::to_vec(&json!({ "Container": container }))
                .unwrap_or_default()
                .as_slice(),
        );
        if (200..300).contains(&response.status) {
            return json_response(
                200,
                json!({
                    "api": "cratebay.pod.attach.v1",
                    "pod": pod_name,
                    "container": container,
                    "attached": true,
                    "backend": "cratebay-cni",
                }),
            );
        }
        response
    }

    fn native_detach_container_from_pod(
        state: &AdapterState,
        pod_name: String,
        body: &[u8],
    ) -> HttpResponse {
        let payload = parse_json_body(body);
        let container = optional_string_value(
            payload
                .get("container")
                .or_else(|| payload.get("Container"))
                .or_else(|| payload.get("id")),
        )
        .unwrap_or_default();
        if container.is_empty() {
            return error_response(
                400,
                "native pod detach requires container",
                json!({ "api": "cratebay.pod.detach.v1", "pod": pod_name }),
            );
        }
        if managed_network_value_by_id(&pod_name)
            .filter(is_cratebay_pod_network)
            .is_none()
        {
            return error_response(
                404,
                "CrateBay pod not found",
                json!({
                    "api": "cratebay.pod.detach.v1",
                    "pod": pod_name,
                    "container": container,
                    "backend": "cratebay-cni",
                }),
            );
        }

        let force = bool_value(payload.get("force").or_else(|| payload.get("Force")));
        let response = disconnect_network(
            state,
            pod_name.clone(),
            serde_json::to_vec(&json!({
                "Container": container,
                "Force": force,
            }))
            .unwrap_or_default()
            .as_slice(),
        );
        if (200..300).contains(&response.status) {
            return json_response(
                200,
                json!({
                    "api": "cratebay.pod.detach.v1",
                    "pod": pod_name,
                    "container": container,
                    "detached": true,
                    "force": force,
                    "backend": "cratebay-cni",
                }),
            );
        }
        response
    }

    fn native_create_container(state: &AdapterState, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some((name, create_payload, auto_start)) = native_create_request(&payload) else {
            return error_response(
                400,
                "native container create requires image",
                json!({ "api": "cratebay.container.create.v1" }),
            );
        };

        let pending =
            pending_from_create_payload_with_config(Some(&state.config), &name, &create_payload);
        let id = pending.id.clone();
        if let Err(error) = store_pending_container(state, &id, &name, pending) {
            return error_response(500, "native container registry write failed", error);
        }

        let mut started = false;
        if auto_start {
            let start_response = start_container(state, id.clone());
            if !(200..300).contains(&start_response.status) {
                return start_response;
            }
            started = true;
        }

        json_response(
            200,
            json!({
                "api": "cratebay.container.create.v1",
                "id": id,
                "name": name,
                "image": string_value(create_payload.get("Image")),
                "backend": "containerd-pending",
                "started": started,
            }),
        )
    }

    fn native_image_ref(payload: &Value) -> Option<String> {
        let image = optional_string_value(
            payload
                .get("image")
                .or_else(|| payload.get("name"))
                .or_else(|| payload.get("fromImage"))
                .or_else(|| payload.get("Image")),
        )?;
        let tag = optional_string_value(payload.get("tag").or_else(|| payload.get("Tag")));
        Some(match tag {
            Some(tag) if !image.contains(':') => format!("{image}:{tag}"),
            _ => image,
        })
    }

    fn native_network_create_request(payload: &Value) -> Option<(String, Value)> {
        let name = optional_string_value(payload.get("name").or_else(|| payload.get("Name")))?;
        let driver = optional_string_value(payload.get("driver").or_else(|| payload.get("Driver")));
        let labels = object_or_empty(payload.get("labels").or_else(|| payload.get("Labels")));
        let options = object_or_empty(payload.get("options").or_else(|| payload.get("Options")));
        let internal = bool_value(payload.get("internal").or_else(|| payload.get("Internal")));
        let enable_ipv6 = bool_value(
            payload
                .get("enableIPv6")
                .or_else(|| payload.get("enable_ipv6"))
                .or_else(|| payload.get("EnableIPv6")),
        );

        Some((
            name.clone(),
            json!({
                "Name": name,
                "Driver": driver.unwrap_or_else(|| "bridge".to_string()),
                "Labels": labels,
                "Options": options,
                "Internal": internal,
                "EnableIPv6": enable_ipv6,
                "IPAM": payload
                    .get("ipam")
                    .or_else(|| payload.get("IPAM"))
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            }),
        ))
    }

    fn native_volume_create_request(payload: &Value) -> Option<(String, Value)> {
        let name = optional_string_value(payload.get("name").or_else(|| payload.get("Name")))?;
        Some((
            name.clone(),
            json!({
                "Name": name,
                "Driver": optional_string_value(payload.get("driver").or_else(|| payload.get("Driver")))
                    .unwrap_or_else(|| "local".to_string()),
                "Labels": object_or_empty(payload.get("labels").or_else(|| payload.get("Labels"))),
                "Options": object_or_empty(payload.get("options").or_else(|| payload.get("Options"))),
            }),
        ))
    }

    fn native_pod_create_request(payload: &Value) -> Option<(String, Value)> {
        let name = optional_string_value(payload.get("name").or_else(|| payload.get("Name")))?;
        let mut labels = serde_json::Map::new();
        labels.insert("com.cratebay.managed".to_string(), json!("true"));
        labels.insert("com.cratebay.pod".to_string(), json!("true"));
        if let Value::Object(extra) =
            object_or_empty(payload.get("labels").or_else(|| payload.get("Labels")))
        {
            for (key, value) in extra {
                labels.insert(key, value);
            }
        }
        labels.insert("com.cratebay.managed".to_string(), json!("true"));
        labels.insert("com.cratebay.pod".to_string(), json!("true"));

        Some((
            name.clone(),
            json!({
                "Name": name,
                "Driver": optional_string_value(payload.get("driver").or_else(|| payload.get("Driver")))
                    .unwrap_or_else(|| "bridge".to_string()),
                "Labels": Value::Object(labels),
                "Options": object_or_empty(payload.get("options").or_else(|| payload.get("Options"))),
                "Internal": bool_value(payload.get("internal").or_else(|| payload.get("Internal"))),
                "EnableIPv6": bool_value(
                    payload
                        .get("enableIPv6")
                        .or_else(|| payload.get("enable_ipv6"))
                        .or_else(|| payload.get("EnableIPv6")),
                ),
                "IPAM": payload
                    .get("ipam")
                    .or_else(|| payload.get("IPAM"))
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            }),
        ))
    }

    fn is_cratebay_pod_network(network: &Value) -> bool {
        network_label_matches(network, "com.cratebay.pod=true")
    }

    fn native_create_request(payload: &Value) -> Option<(String, Value, bool)> {
        let image = optional_string_value(payload.get("image").or_else(|| payload.get("Image")))?;
        let name = optional_string_value(payload.get("name").or_else(|| payload.get("Name")))
            .unwrap_or_else(|| unique_task_id("cratebay"));
        let entrypoint = optional_string_value(
            payload
                .get("entrypoint")
                .or_else(|| payload.get("Entrypoint")),
        );
        let working_dir = optional_string_value(
            payload
                .get("workingDir")
                .or_else(|| payload.get("working_dir"))
                .or_else(|| payload.get("WorkingDir")),
        );
        let user = optional_string_value(payload.get("user").or_else(|| payload.get("User")));
        let env = string_array(payload.get("env").or_else(|| payload.get("Env")));
        let volumes = string_array(
            payload
                .get("volume")
                .or_else(|| payload.get("volumes"))
                .or_else(|| payload.get("Binds")),
        );
        let publish = string_array(
            payload
                .get("publish")
                .or_else(|| payload.get("ports"))
                .or_else(|| payload.get("PortBindings")),
        );
        let labels = object_or_empty(payload.get("labels").or_else(|| payload.get("Labels")));
        let network = optional_string_value(payload.get("network"))
            .or_else(|| optional_string_value(payload.get("networkMode")))
            .or_else(|| optional_string_value(payload.get("NetworkMode")))
            .or_else(|| optional_string_value(payload.get("pod")))
            .or_else(|| optional_string_value(payload.get("Pod")));
        let read_only = bool_value(
            payload
                .get("readOnly")
                .or_else(|| payload.get("read_only"))
                .or_else(|| payload.get("ReadonlyRootfs")),
        );
        let no_pull = bool_value(
            payload
                .get("noPull")
                .or_else(|| payload.get("no_pull"))
                .or_else(|| payload.get("CrateBayNoPull")),
        );
        let registry_mirrors = normalize_registry_mirrors(
            payload
                .get("registryMirrors")
                .or_else(|| payload.get("registry_mirrors"))
                .or_else(|| payload.get("CrateBayRegistryMirrors")),
        );
        let auto_start = payload
            .get("autoStart")
            .or_else(|| payload.get("auto_start"))
            .map(|value| bool_value(Some(value)))
            .unwrap_or_else(|| {
                !bool_value(payload.get("noStart").or_else(|| payload.get("no_start")))
            });

        let command = native_command_array(payload, entrypoint.is_some());
        let mut host_config = serde_json::Map::new();
        if !volumes.is_empty() {
            host_config.insert("Binds".to_string(), json!(volumes));
        }
        if !publish.is_empty() {
            host_config.insert("PortBindings".to_string(), native_port_bindings(&publish));
        }
        if let Some(network) = network.filter(|value| !value.trim().is_empty()) {
            host_config.insert("NetworkMode".to_string(), json!(network));
        }
        if read_only {
            host_config.insert("ReadonlyRootfs".to_string(), json!(true));
        }
        if bool_value(
            payload
                .get("privileged")
                .or_else(|| payload.get("Privileged")),
        ) {
            host_config.insert("Privileged".to_string(), json!(true));
        }
        if let Some(memory_mb) =
            numeric_i64(payload.get("memory").or_else(|| payload.get("memoryMb")))
        {
            if memory_mb > 0 {
                host_config.insert("Memory".to_string(), json!(memory_mb * 1024 * 1024));
            }
        }
        if let Some(cpu) = numeric_f64(payload.get("cpu").or_else(|| payload.get("cpuCores"))) {
            if cpu > 0.0 {
                host_config.insert(
                    "NanoCpus".to_string(),
                    json!((cpu * 1_000_000_000.0) as i64),
                );
            }
        }

        Some((
            name,
            json!({
                "Image": image,
                "Entrypoint": entrypoint,
                "Cmd": command,
                "Env": env,
                "Labels": labels,
                "HostConfig": Value::Object(host_config),
                "WorkingDir": working_dir,
                "User": user,
                "Tty": bool_value(payload.get("tty").or_else(|| payload.get("Tty"))),
                "CrateBayNoPull": no_pull,
                "CrateBayRegistryMirrors": registry_mirrors,
            }),
            auto_start,
        ))
    }

    fn native_command_array(payload: &Value, has_entrypoint: bool) -> Vec<String> {
        let value = payload
            .get("command")
            .or_else(|| payload.get("cmd"))
            .or_else(|| payload.get("Cmd"));
        match value {
            Some(Value::Array(_)) => string_array(value),
            Some(Value::String(command)) if !command.trim().is_empty() && !has_entrypoint => {
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    command.trim().to_string(),
                ]
            }
            Some(Value::String(command)) if !command.trim().is_empty() => {
                vec![command.trim().to_string()]
            }
            _ => Vec::new(),
        }
    }

    fn native_port_bindings(specs: &[String]) -> Value {
        let mut bindings = serde_json::Map::new();
        for spec in specs {
            let Some((key, host_port)) = native_port_binding(spec) else {
                continue;
            };
            if let Some(items) = bindings
                .entry(key)
                .or_insert_with(|| json!([]))
                .as_array_mut()
            {
                items.push(json!({ "HostPort": host_port }));
            }
        }
        Value::Object(bindings)
    }

    fn native_port_binding(spec: &str) -> Option<(String, String)> {
        let spec = spec.trim();
        if spec.is_empty() {
            return None;
        }
        let (ports, protocol) = spec.rsplit_once('/').unwrap_or((spec, "tcp"));
        let parts = ports.split(':').collect::<Vec<_>>();
        match parts.as_slice() {
            [container] => Some((format!("{container}/{protocol}"), String::new())),
            [host, container] => Some((format!("{container}/{protocol}"), (*host).to_string())),
            _ => None,
        }
    }

    #[cfg(test)]
    fn pending_from_create_payload(name: &str, payload: &Value) -> PendingContainer {
        pending_from_create_payload_with_config(None, name, payload)
    }

    fn pending_from_create_payload_with_config(
        config: Option<&Config>,
        name: &str,
        payload: &Value,
    ) -> PendingContainer {
        let image = string_value(payload.get("Image"));
        let image_config =
            config.and_then(|config| match containerd_image_config(config, &image) {
                Ok(value) => Some(value),
                Err(error) => {
                    eprintln!(
                        "cratebay-engine-adapter: image config resolve failed for {}: {}",
                        image,
                        serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
                    );
                    None
                }
            });
        let command = merged_container_command(payload, image_config.as_ref());
        let network = pending_network(payload.get("HostConfig"));
        PendingContainer {
            id: generated_container_id(name),
            name: name.to_string(),
            created_at: now_seconds(),
            runtime_id: name.to_string(),
            image,
            command,
            env: string_array(payload.get("Env")),
            working_dir: optional_string_value(payload.get("WorkingDir")),
            mounts: pending_mounts(payload.get("HostConfig")),
            aliases: pending_network_aliases(payload, name, network.as_deref()),
            labels: pending_labels(payload),
            network,
            netns_name: None,
            netns_path: None,
            ports: pending_port_mappings(payload.get("HostConfig")),
            log_path: container_log_path(name),
            no_pull: bool_value(payload.get("CrateBayNoPull")),
            registry_mirrors: registry_mirrors_from_payload(payload),
            privileged: pending_privileged(payload.get("HostConfig")),
            started_with_ctr: false,
            exit_code: None,
        }
    }

    fn registry_mirrors_from_payload(payload: &Value) -> Vec<String> {
        normalize_registry_mirrors(
            payload
                .get("CrateBayRegistryMirrors")
                .or_else(|| payload.get("registryMirrors"))
                .or_else(|| payload.get("registry_mirrors")),
        )
    }

    fn normalize_registry_mirrors(value: Option<&Value>) -> Vec<String> {
        string_array(value)
            .into_iter()
            .map(|mirror| normalize_registry_mirror(&mirror))
            .filter(|mirror| !mirror.is_empty())
            .collect()
    }

    fn normalize_registry_mirror(mirror: &str) -> String {
        mirror
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string()
    }

    fn merged_container_command(payload: &Value, image_config: Option<&Value>) -> Vec<String> {
        let image_entrypoint = image_config
            .and_then(|config| nested_value(config, &["config", "Entrypoint"]))
            .map(|value| entrypoint_array(Some(value)))
            .unwrap_or_default();
        let image_cmd = image_config
            .and_then(|config| nested_value(config, &["config", "Cmd"]))
            .map(|value| string_array(Some(value)))
            .unwrap_or_default();

        let entrypoint = if non_null_field(payload, "Entrypoint") {
            entrypoint_array(payload.get("Entrypoint"))
        } else {
            image_entrypoint
        };
        let cmd = if non_null_field(payload, "Cmd") {
            string_array(payload.get("Cmd"))
        } else {
            image_cmd
        };

        entrypoint.into_iter().chain(cmd).collect()
    }

    fn non_null_field(value: &Value, key: &str) -> bool {
        value.get(key).is_some_and(|field| !field.is_null())
    }

    fn native_start_container(state: &AdapterState, id: String) -> HttpResponse {
        let response = start_container(state, id.clone());
        if !(200..300).contains(&response.status) {
            return response;
        }
        json_response(
            200,
            json!({
                "api": "cratebay.container.start.v1",
                "backend": "containerd",
                "id": id,
                "state": "started",
            }),
        )
    }

    fn native_stop_container(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let mut query = HashMap::new();
        if let Some(timeout) = integer_string(
            payload
                .get("timeout")
                .or_else(|| payload.get("Timeout"))
                .or_else(|| payload.get("t")),
        ) {
            query.insert("t".to_string(), timeout);
        }
        let response = stop_container(state, id.clone(), &query);
        if !(200..300).contains(&response.status) {
            return response;
        }
        json_response(
            200,
            json!({
                "api": "cratebay.container.stop.v1",
                "id": id,
                "state": "stopped",
            }),
        )
    }

    fn native_remove_container(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let mut query = HashMap::new();
        if bool_value(
            payload
                .get("force")
                .or_else(|| payload.get("Force"))
                .or_else(|| payload.get("removeRunning")),
        ) {
            query.insert("force".to_string(), "true".to_string());
        }
        let response = remove_container(state, id.clone(), &query);
        if !(200..300).contains(&response.status) {
            return response;
        }
        json_response(
            200,
            json!({
                "api": "cratebay.container.remove.v1",
                "id": id,
                "removed": true,
            }),
        )
    }

    fn native_inspect_container(state: &AdapterState, id: String) -> HttpResponse {
        if let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        {
            return json_response(
                200,
                json!({
                    "api": "cratebay.container.inspect.v1",
                    "item": native_contract::container_inspect(pending_inspect_value(&pending)),
                }),
            );
        }

        managed_container_not_found("native container inspect failed", &id)
    }

    fn native_logs_container(
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        let output = if let Some(pending) =
            pending_container(state, &id).filter(|pending| pending.started_with_ctr)
        {
            match fs::read(&pending.log_path) {
                Ok(output) => Ok((output, Vec::new())),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok((Vec::new(), Vec::new()))
                }
                Err(error) => Err(json!({
                    "path": pending.log_path.display().to_string(),
                    "error": error.to_string()
                })),
            }
        } else if pending_container(state, &id).is_some() {
            Ok((Vec::new(), Vec::new()))
        } else {
            return managed_container_not_found("native container logs failed", &id);
        };

        match output {
            Ok((stdout, stderr)) => {
                let tail = query
                    .get("tail")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|value| *value > 0);
                let stdout = String::from_utf8_lossy(&stdout).into_owned();
                let stderr = String::from_utf8_lossy(&stderr).into_owned();
                let stdout = tail.map(|tail| tail_lines(&stdout, tail)).unwrap_or(stdout);
                let stderr = tail.map(|tail| tail_lines(&stderr, tail)).unwrap_or(stderr);
                json_response(
                    200,
                    json!({
                        "api": "cratebay.container.logs.v1",
                        "id": id,
                        "stdout": stdout,
                        "stderr": stderr,
                        "logs": format!("{stdout}{stderr}"),
                        "timestamps": query_bool(query, "timestamps"),
                    }),
                )
            }
            Err(error) => error_response(500, "native container logs failed", error),
        }
    }

    fn native_wait_container(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let timeout_secs = payload
            .get("timeout")
            .or_else(|| payload.get("timeoutSeconds"))
            .or_else(|| payload.get("Timeout"))
            .and_then(|value| numeric_i64(Some(value)))
            .and_then(|value| u64::try_from(value).ok())
            .filter(|value| *value > 0);

        if let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        {
            let exit_code = match pending.exit_code {
                Some(exit_code) => Some(exit_code),
                None => match timeout_secs {
                    Some(timeout) => wait_for_pending_exit_code_with_timeout(
                        state,
                        &id,
                        &pending.name,
                        Duration::from_secs(timeout),
                    ),
                    None => Some(wait_for_pending_exit_code(state, &id, &pending.name)),
                },
            };

            return json_response(
                200,
                json!({
                    "api": "cratebay.container.wait.v1",
                    "id": id,
                    "backend": "containerd",
                    "exitCode": exit_code,
                    "timedOut": exit_code.is_none(),
                }),
            );
        }

        managed_container_not_found("native container wait failed", &id)
    }

    fn native_exec_container(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let cmd = string_array(
            payload
                .get("cmd")
                .or_else(|| payload.get("command"))
                .or_else(|| payload.get("Cmd")),
        );
        if cmd.is_empty() {
            return error_response(
                400,
                "native exec command is required",
                json!({ "container": id }),
            );
        }

        let working_dir = optional_string_value(
            payload
                .get("workingDir")
                .or_else(|| payload.get("working_dir"))
                .or_else(|| payload.get("WorkingDir")),
        );
        let timeout_secs = positive_u64_value(
            payload
                .get("timeout")
                .or_else(|| payload.get("timeoutSeconds"))
                .or_else(|| payload.get("Timeout")),
        );
        let max_output_bytes = positive_u64_value(
            payload
                .get("maxOutputBytes")
                .or_else(|| payload.get("max_output_bytes"))
                .or_else(|| payload.get("MaxOutputBytes")),
        );
        let record = ExecRecord {
            container_id: id.clone(),
            cmd: cmd.clone(),
            working_dir: working_dir.clone(),
            attach_stdin: false,
            tty: false,
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };

        match run_exec_record_with_limits(state, &record, timeout_secs, max_output_bytes) {
            Ok(result) => json_response(
                200,
                json!({
                    "api": "cratebay.container.exec.v1",
                    "id": id,
                    "command": cmd,
                    "workingDir": working_dir,
                    "backend": result.backend,
                    "args": result.args,
                    "exitCode": result.exit_code,
                    "timedOut": result.timed_out,
                    "stdoutTruncated": result.stdout_truncated,
                    "stderrTruncated": result.stderr_truncated,
                    "stdout": String::from_utf8_lossy(&result.output.stdout).into_owned(),
                    "stderr": String::from_utf8_lossy(&result.output.stderr).into_owned(),
                }),
            ),
            Err(error) => error_response(500, "native container exec failed", error),
        }
    }

    fn native_terminal_open(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let session_id = optional_string_value(
            payload
                .get("sessionId")
                .or_else(|| payload.get("session_id"))
                .or_else(|| payload.get("id")),
        )
        .unwrap_or_else(|| unique_task_id("cratebay-terminal"));
        let mut cmd = string_array(
            payload
                .get("cmd")
                .or_else(|| payload.get("command"))
                .or_else(|| payload.get("Cmd")),
        );
        if cmd.is_empty() {
            cmd = vec!["sh".to_string(), "-i".to_string()];
        }
        let working_dir = optional_string_value(
            payload
                .get("workingDir")
                .or_else(|| payload.get("working_dir"))
                .or_else(|| payload.get("WorkingDir")),
        );
        let record = ExecRecord {
            container_id: id.clone(),
            cmd: cmd.clone(),
            working_dir: working_dir.clone(),
            attach_stdin: true,
            tty: true,
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };

        let (mut command, args, command_backend) = match terminal_record_command(state, &record) {
            Ok(command) => command,
            Err(error) => return error_response(500, "native terminal open failed", error),
        };
        let (cols, rows) = terminal_size_from_payload(&payload);
        let output = Arc::new(Mutex::new(Vec::<TerminalOutputChunk>::new()));
        let exit_code = Arc::new(Mutex::new(None));

        let pty_attempt = open_terminal_pty(cols, rows).and_then(|pty| {
            let output_master = pty
                .master
                .try_clone()
                .map_err(|error| format!("clone pty master: {error}"))?;
            let stdin = stdio_from_fd_dup(pty.slave.as_raw_fd())?;
            let stdout = stdio_from_fd_dup(pty.slave.as_raw_fd())?;
            let stderr = stdio_from_fd_dup(pty.slave.as_raw_fd())?;
            Ok((pty, output_master, stdin, stdout, stderr))
        });

        let (child, input, backend, transport, pty_error) = match pty_attempt {
            Ok((pty, output_master, stdin, stdout, stderr)) => {
                command.stdin(stdin).stdout(stdout).stderr(stderr);
                let child = match command.spawn() {
                    Ok(child) => child,
                    Err(error) => {
                        return error_response(
                            500,
                            "native terminal open failed",
                            json!({ "error": error.to_string(), "args": args }),
                        );
                    }
                };
                drop(pty.slave);
                spawn_terminal_pipe_reader(output_master, "stdout", output.clone());
                (
                    child,
                    TerminalInput::Pty(pty.master),
                    "containerd-pty",
                    "cratebay-native-pty",
                    None,
                )
            }
            Err(error) => {
                command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(spawn_error) => {
                        return error_response(
                            500,
                            "native terminal open failed",
                            json!({ "error": spawn_error.to_string(), "args": args, "ptyError": error }),
                        );
                    }
                };
                if let Some(stdout) = child.stdout.take() {
                    spawn_terminal_pipe_reader(stdout, "stdout", output.clone());
                }
                if let Some(stderr) = child.stderr.take() {
                    spawn_terminal_pipe_reader(stderr, "stderr", output.clone());
                }
                let stdin = child.stdin.take();
                (
                    child,
                    TerminalInput::Pipe(stdin),
                    command_backend,
                    "cratebay-native-pipe-fallback",
                    Some(error),
                )
            }
        };

        let child = Arc::new(Mutex::new(child));
        spawn_terminal_waiter(child.clone(), exit_code.clone(), output.clone());

        let session = TerminalSession {
            container_id: id.clone(),
            input: Arc::new(Mutex::new(input)),
            child,
            output,
            exit_code,
            transport,
        };

        match state.terminals.lock() {
            Ok(mut terminals) => {
                if let Some(previous) = terminals.remove(&session_id) {
                    stop_terminal_session(&previous);
                }
                terminals.insert(session_id.clone(), session);
            }
            Err(error) => {
                return error_response(500, "terminal store lock failed", json!(error.to_string()));
            }
        }

        json_response(
            201,
            json!({
                "api": "cratebay.container.terminal.open.v1",
                "backend": backend,
                "container": id,
                "sessionId": session_id,
                "command": cmd,
                "workingDir": working_dir,
                "interactive": true,
                "tty": transport == "cratebay-native-pty",
                "transport": transport,
                "cols": cols,
                "rows": rows,
                "ptyError": pty_error,
            }),
        )
    }

    fn native_terminal_input(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some(session_id) = terminal_session_id(&payload) else {
            return error_response(400, "terminal session id is required", json!({}));
        };
        let data = payload
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(session) = terminal_session(state, &session_id) else {
            return terminal_not_found(&session_id);
        };
        if session.container_id != id {
            return error_response(
                409,
                "terminal session belongs to a different container",
                json!({ "sessionId": session_id, "container": session.container_id, "requested": id }),
            );
        }

        match session.input.lock() {
            Ok(mut input) => {
                let write_result = match &mut *input {
                    TerminalInput::Pipe(stdin) => {
                        let Some(stdin) = stdin.as_mut() else {
                            return error_response(
                                410,
                                "terminal stdin is closed",
                                json!({ "sessionId": session_id }),
                            );
                        };
                        stdin.write_all(data.as_bytes()).and_then(|_| stdin.flush())
                    }
                    TerminalInput::Pty(master) => master
                        .write_all(data.as_bytes())
                        .and_then(|_| master.flush()),
                };
                if let Err(error) = write_result {
                    return error_response(
                        500,
                        "terminal input failed",
                        json!({ "sessionId": session_id, "error": error.to_string() }),
                    );
                }
            }
            Err(error) => {
                return error_response(500, "terminal stdin lock failed", json!(error.to_string()));
            }
        }

        json_response(
            200,
            json!({
                "api": "cratebay.container.terminal.input.v1",
                "container": id,
                "sessionId": session_id,
                "bytes": data.len(),
            }),
        )
    }

    fn native_terminal_read(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some(session_id) = terminal_session_id(&payload) else {
            return error_response(400, "terminal session id is required", json!({}));
        };
        let Some(session) = terminal_session(state, &session_id) else {
            return terminal_not_found(&session_id);
        };
        if session.container_id != id {
            return error_response(
                409,
                "terminal session belongs to a different container",
                json!({ "sessionId": session_id, "container": session.container_id, "requested": id }),
            );
        }

        let chunks = match session.output.lock() {
            Ok(mut output) => output
                .drain(..)
                .map(|chunk| json!({ "stream": chunk.stream, "data": chunk.data }))
                .collect::<Vec<_>>(),
            Err(error) => {
                return error_response(
                    500,
                    "terminal output lock failed",
                    json!(error.to_string()),
                );
            }
        };
        let exit_code = session.exit_code.lock().ok().and_then(|guard| *guard);

        json_response(
            200,
            json!({
                "api": "cratebay.container.terminal.read.v1",
                "container": id,
                "sessionId": session_id,
                "chunks": chunks,
                "exitCode": exit_code,
                "running": exit_code.is_none(),
            }),
        )
    }

    fn native_terminal_resize(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some(session_id) = terminal_session_id(&payload) else {
            return error_response(400, "terminal session id is required", json!({}));
        };
        let Some(session) = terminal_session(state, &session_id) else {
            return terminal_not_found(&session_id);
        };
        if session.container_id != id {
            return error_response(
                409,
                "terminal session belongs to a different container",
                json!({ "sessionId": session_id, "container": session.container_id, "requested": id }),
            );
        }
        let (cols, rows) = terminal_size_from_payload(&payload);
        let resize_result = match session.input.lock() {
            Ok(input) => match &*input {
                TerminalInput::Pty(master) => set_pty_window_size(master.as_raw_fd(), cols, rows),
                TerminalInput::Pipe(_) => {
                    Err("terminal is using pipe fallback transport".to_string())
                }
            },
            Err(error) => Err(format!("terminal input lock failed: {error}")),
        };
        let (resized, message) = match resize_result {
            Ok(()) => (true, None),
            Err(error) => (false, Some(error)),
        };
        json_response(
            200,
            json!({
                "api": "cratebay.container.terminal.resize.v1",
                "container": id,
                "sessionId": session_id,
                "resized": resized,
                "tty": session.transport == "cratebay-native-pty",
                "transport": session.transport,
                "cols": cols,
                "rows": rows,
                "message": message,
            }),
        )
    }

    fn native_terminal_close(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let Some(session_id) = terminal_session_id(&payload) else {
            return error_response(400, "terminal session id is required", json!({}));
        };
        let session = match state.terminals.lock() {
            Ok(mut terminals) => terminals.remove(&session_id),
            Err(error) => {
                return error_response(500, "terminal store lock failed", json!(error.to_string()));
            }
        };
        let Some(session) = session else {
            return terminal_not_found(&session_id);
        };
        if session.container_id != id {
            return error_response(
                409,
                "terminal session belongs to a different container",
                json!({ "sessionId": session_id, "container": session.container_id, "requested": id }),
            );
        }
        stop_terminal_session(&session);
        json_response(
            200,
            json!({
                "api": "cratebay.container.terminal.close.v1",
                "container": id,
                "sessionId": session_id,
                "closed": true,
            }),
        )
    }

    fn native_stats_container(state: &AdapterState, id: String) -> HttpResponse {
        if let Some(pending) = pending_container(state, &id) {
            if !pending.started_with_ctr || pending.exit_code.is_some() {
                return json_response(
                    200,
                    native_stats_value(
                        state,
                        &pending,
                        ContainerRuntimeMetrics::default(),
                        "cratebay-registry",
                    ),
                );
            }

            return match containerd_task_metrics(&state.config, &pending) {
                Ok(metrics) => json_response(
                    200,
                    native_stats_value(state, &pending, metrics, "containerd"),
                ),
                Err(error) => error_response(500, "native container stats failed", error),
            };
        }

        managed_container_not_found("native container stats failed", &id)
    }

    fn inspect_container(state: &AdapterState, id: String) -> HttpResponse {
        if let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        {
            return json_response(200, pending_inspect_value(&pending));
        }
        managed_container_not_found("container inspect failed", &id)
    }

    fn logs_container(
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        if let Some(pending) = pending_container(state, &id) {
            return match fs::read(&pending.log_path) {
                Ok(output) => {
                    let output = apply_log_tail(output, query.get("tail"));
                    docker_stream_response(output, Vec::new())
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    docker_stream_response(Vec::new(), Vec::new())
                }
                Err(error) => error_response(
                    500,
                    "container logs failed",
                    json!({ "path": pending.log_path.display().to_string(), "error": error.to_string() }),
                ),
            };
        }

        managed_container_not_found("container logs failed", &id)
    }

    fn attach_container(
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        let Some(pending) = pending_container(state, &id) else {
            return managed_container_not_found("container attach failed", &id);
        };
        let output = if query_bool_or(query, "logs", false) {
            fs::read(&pending.log_path).unwrap_or_default()
        } else {
            Vec::new()
        };
        let (stdout, stderr) = attach_stream_flags(query);
        if stdout || !stderr {
            docker_stream_response(output, Vec::new())
        } else {
            docker_stream_response(Vec::new(), output)
        }
    }

    fn stream_container_attach(
        stream: &mut UnixStream,
        state: &AdapterState,
        id: String,
        query: &HashMap<String, String>,
    ) -> Result<(), String> {
        let Some(initial_pending) = pending_container(state, &id) else {
            return write_response(
                stream,
                managed_container_not_found("container attach failed", &id),
            );
        };
        let (stdout, stderr) = attach_stream_flags(query);
        let should_stream = query_bool_or(query, "stream", true);
        let mut offset = if query_bool_or(query, "logs", false) {
            0
        } else {
            fs::metadata(&initial_pending.log_path)
                .map(|metadata| metadata.len() as usize)
                .unwrap_or_default()
        };

        stream.write_all(
            b"HTTP/1.1 101 UPGRADED\r\nContent-Type: application/vnd.docker.raw-stream\r\nConnection: Upgrade\r\nUpgrade: tcp\r\n\r\n",
        )
        .map_err(|error| format!("write container attach upgrade response: {error}"))?;
        stream
            .flush()
            .map_err(|error| format!("flush container attach upgrade response: {error}"))?;

        loop {
            let Some(pending) = pending_container(state, &id)
                .map(|pending| refresh_pending_task_state(state, pending))
            else {
                break;
            };
            if let Ok(output) = fs::read(&pending.log_path) {
                if output.len() > offset {
                    let chunk = &output[offset..];
                    if stdout || stderr {
                        let stream_type = if stdout { 1 } else { 2 };
                        if write_docker_stream_frame(stream, stream_type, chunk).is_err() {
                            break;
                        }
                        let _ = stream.flush();
                    }
                    offset = output.len();
                }
            }

            if !should_stream || (pending.started_with_ctr && pending.exit_code.is_some()) {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        Ok(())
    }

    fn attach_stream_flags(query: &HashMap<String, String>) -> (bool, bool) {
        (
            query_bool_or(query, "stdout", true),
            query_bool_or(query, "stderr", true),
        )
    }

    fn query_bool_or(query: &HashMap<String, String>, key: &str, default: bool) -> bool {
        query
            .get(key)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(default)
    }

    fn apply_log_tail(output: Vec<u8>, tail: Option<&String>) -> Vec<u8> {
        let Some(tail) = tail
            .filter(|value| !matches!(value.as_str(), "" | "all"))
            .and_then(|value| value.parse::<usize>().ok())
        else {
            return output;
        };
        tail_lines(&String::from_utf8_lossy(&output), tail).into_bytes()
    }

    fn wait_container(state: &AdapterState, id: String) -> HttpResponse {
        if let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        {
            if pending.started_with_ctr && pending.exit_code.is_none() {
                return wait_response(wait_for_pending_exit_code(state, &id, &pending.name));
            }
            return wait_response(pending.exit_code.unwrap_or_default());
        }

        managed_container_not_found("container wait failed", &id)
    }

    fn wait_response(exit_code: i64) -> HttpResponse {
        json_response(200, json!({ "StatusCode": exit_code, "Error": null }))
    }

    fn wait_for_pending_exit_code(state: &AdapterState, id: &str, name: &str) -> i64 {
        loop {
            if let Some(exit_code) = pending_container(state, id)
                .or_else(|| pending_container(state, name))
                .and_then(|pending| pending.exit_code)
            {
                return exit_code;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn wait_for_pending_exit_code_with_timeout(
        state: &AdapterState,
        id: &str,
        name: &str,
        timeout: Duration,
    ) -> Option<i64> {
        let deadline = SystemTime::now() + timeout;
        loop {
            if let Some(exit_code) = pending_container(state, id)
                .or_else(|| pending_container(state, name))
                .and_then(|pending| pending.exit_code)
            {
                return Some(exit_code);
            }
            if SystemTime::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn stats_container(state: &AdapterState, id: String) -> HttpResponse {
        if let Some(pending) = pending_container(state, &id) {
            if !pending.started_with_ctr || pending.exit_code.is_some() {
                return json_response(
                    200,
                    docker_stats_value(
                        state,
                        &pending,
                        ContainerRuntimeMetrics::default(),
                        "cratebay-registry",
                    ),
                );
            }

            return match containerd_task_metrics(&state.config, &pending) {
                Ok(metrics) => json_response(
                    200,
                    docker_stats_value(state, &pending, metrics, "containerd"),
                ),
                Err(error) => error_response(500, "container stats failed", error),
            };
        }

        managed_container_not_found("container stats failed", &id)
    }

    fn containerd_task_metrics(
        config: &Config,
        pending: &PendingContainer,
    ) -> Result<ContainerRuntimeMetrics, Value> {
        let output = run_ctr(
            config,
            vec![
                "tasks".to_string(),
                "metrics".to_string(),
                containerd_task_name(pending).to_string(),
            ],
        )?;
        Ok(parse_containerd_metrics(&output.stdout))
    }

    fn docker_stats_value(
        state: &AdapterState,
        pending: &PendingContainer,
        metrics: ContainerRuntimeMetrics,
        source: &str,
    ) -> Value {
        let online_cpus = online_cpu_count();
        let system_total = read_system_cpu_usage().unwrap_or(metrics.cpu_total);
        let previous = state
            .metrics
            .lock()
            .ok()
            .and_then(|mut snapshots| {
                snapshots.insert(
                    pending.id.clone(),
                    ContainerMetricSnapshot {
                        cpu_total: metrics.cpu_total,
                        system_total,
                    },
                )
            })
            .unwrap_or_default();
        json!({
            "id": pending.id,
            "name": format!("/{}", pending.name),
            "read": chrono_like_now(),
            "cpu_stats": {
                "cpu_usage": {
                    "total_usage": metrics.cpu_total,
                    "percpu_usage": [metrics.cpu_total],
                },
                "system_cpu_usage": system_total,
                "online_cpus": online_cpus,
            },
            "precpu_stats": {
                "cpu_usage": {
                    "total_usage": previous.cpu_total,
                    "percpu_usage": [previous.cpu_total],
                },
                "system_cpu_usage": previous.system_total,
                "online_cpus": online_cpus,
            },
            "memory_stats": {
                "usage": metrics.memory_usage,
                "limit": metrics.memory_limit,
            },
            "networks": {},
            "blkio_stats": {},
            "CrateBay": {
                "backend": source,
                "managed": true,
            },
        })
    }

    fn native_stats_value(
        state: &AdapterState,
        pending: &PendingContainer,
        metrics: ContainerRuntimeMetrics,
        source: &str,
    ) -> Value {
        let online_cpus = online_cpu_count();
        let system_total = read_system_cpu_usage().unwrap_or(metrics.cpu_total);
        let previous = state
            .metrics
            .lock()
            .ok()
            .and_then(|mut snapshots| {
                snapshots.insert(
                    pending.id.clone(),
                    ContainerMetricSnapshot {
                        cpu_total: metrics.cpu_total,
                        system_total,
                    },
                )
            })
            .unwrap_or_default();
        let cpu_delta = metrics.cpu_total.saturating_sub(previous.cpu_total) as f64;
        let system_delta = system_total.saturating_sub(previous.system_total) as f64;
        let cpu_percent = if cpu_delta > 0.0 && system_delta > 0.0 {
            (cpu_delta / system_delta) * online_cpus as f64 * 100.0
        } else {
            0.0
        };
        let memory_percent = if metrics.memory_limit > 0 {
            (metrics.memory_usage as f64 / metrics.memory_limit as f64) * 100.0
        } else {
            0.0
        };

        json!({
            "api": "cratebay.container.stats.v1",
            "id": pending.id,
            "name": pending.name,
            "readAt": chrono_like_now(),
            "backend": source,
            "managedBy": "cratebay",
            "cpu": {
                "totalUsage": metrics.cpu_total,
                "previousTotalUsage": previous.cpu_total,
                "systemUsage": system_total,
                "previousSystemUsage": previous.system_total,
                "onlineCpus": online_cpus,
                "percent": cpu_percent,
                "coresUsed": cpu_percent / 100.0,
            },
            "memory": {
                "usedBytes": metrics.memory_usage,
                "limitBytes": metrics.memory_limit,
                "usedMb": metrics.memory_usage as f64 / 1024.0 / 1024.0,
                "limitMb": metrics.memory_limit as f64 / 1024.0 / 1024.0,
                "percent": memory_percent,
            },
        })
    }

    fn parse_containerd_metrics(bytes: &[u8]) -> ContainerRuntimeMetrics {
        let text = String::from_utf8_lossy(bytes);
        let mut metrics = ContainerRuntimeMetrics::default();
        let mut cpu_usage_usec = None;

        for line in text.lines() {
            let lower = line.trim().to_ascii_lowercase();
            let Some(value) = last_u64_in_text(&lower) else {
                continue;
            };

            if lower.contains("cpuacct.usage")
                || lower.contains("cpu.usage.total")
                || lower.contains("cpu_usage_total")
                || lower.contains("total_usage")
                || lower.contains("usage_nanoseconds")
            {
                metrics.cpu_total = metrics.cpu_total.max(value);
                continue;
            }

            if lower.contains("usage_usec") {
                cpu_usage_usec = Some(value);
                continue;
            }

            if lower.contains("memory.usage_in_bytes")
                || lower.contains("memory.current")
                || lower.contains("memory_usage")
                || lower.contains("memory usage")
                || lower.contains("usage_bytes")
            {
                metrics.memory_usage = metrics.memory_usage.max(value);
                continue;
            }

            if lower.contains("memory.limit_in_bytes")
                || lower.contains("memory.max")
                || lower.contains("memory_limit")
                || lower.contains("limit_bytes")
            {
                metrics.memory_limit = metrics.memory_limit.max(value);
            }
        }

        if metrics.cpu_total == 0 {
            metrics.cpu_total = cpu_usage_usec.unwrap_or_default().saturating_mul(1000);
        }
        metrics
    }

    fn last_u64_in_text(text: &str) -> Option<u64> {
        text.split(|ch: char| !ch.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<u64>().ok())
            .next_back()
    }

    fn online_cpu_count() -> u64 {
        std::thread::available_parallelism()
            .map(|value| value.get() as u64)
            .unwrap_or(1)
            .max(1)
    }

    fn read_system_cpu_usage() -> Option<u64> {
        let stat = fs::read_to_string("/proc/stat").ok()?;
        let total = stat
            .lines()
            .find(|line| line.starts_with("cpu "))?
            .split_whitespace()
            .skip(1)
            .filter_map(|part| part.parse::<u64>().ok())
            .sum::<u64>();
        Some(total.saturating_mul(10_000_000))
    }

    fn create_exec(state: &AdapterState, container_id: String, body: &[u8]) -> HttpResponse {
        let Some(pending) = pending_container(state, &container_id)
            .map(|pending| refresh_pending_task_state(state, pending))
        else {
            return managed_container_not_found("exec create failed", &container_id);
        };
        if !pending.started_with_ctr || pending.exit_code.is_some() {
            return error_response(
                409,
                "exec create failed",
                json!({
                    "container": container_id,
                    "state": pending_state(&pending).0,
                    "backend": "containerd",
                    "error": "container is not running",
                }),
            );
        }

        let payload = parse_json_body(body);
        let cmd = string_array(payload.get("Cmd"));
        if cmd.is_empty() {
            return error_response(
                400,
                "exec command is required",
                json!({ "container": container_id }),
            );
        }
        let id = unique_task_id("cratebay-exec");
        let record = ExecRecord {
            container_id,
            cmd,
            working_dir: optional_string_value(payload.get("WorkingDir")),
            attach_stdin: bool_value(
                payload
                    .get("AttachStdin")
                    .or_else(|| payload.get("OpenStdin"))
                    .or_else(|| payload.get("attach_stdin"))
                    .or_else(|| payload.get("open_stdin")),
            ),
            tty: bool_value(payload.get("Tty").or_else(|| payload.get("tty"))),
            exit_code: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };

        match state.execs.lock() {
            Ok(mut execs) => {
                execs.insert(id.clone(), record);
                json_response(201, json!({ "Id": id }))
            }
            Err(error) => error_response(500, "exec store lock failed", json!(error.to_string())),
        }
    }

    fn start_exec(state: &AdapterState, id: String) -> HttpResponse {
        let record = match state.execs.lock() {
            Ok(execs) => execs.get(&id).cloned(),
            Err(error) => {
                return error_response(500, "exec store lock failed", json!(error.to_string()));
            }
        };
        let Some(mut record) = record else {
            return error_response(404, "exec instance not found", json!({ "id": id }));
        };

        match run_exec_record_allow_failure(state, &record) {
            Ok(result) => {
                record.exit_code = Some(exec_exit_code(&result.output));
                record.stdout = result.output.stdout.clone();
                record.stderr = result.output.stderr.clone();
                if let Ok(mut execs) = state.execs.lock() {
                    execs.insert(id, record);
                }
                docker_hijack_response(result.output.stdout, result.output.stderr)
            }
            Err(error) => error_response(500, "exec start failed", error),
        }
    }

    fn stream_exec_start(
        stream: &mut UnixStream,
        state: &AdapterState,
        id: String,
    ) -> Result<(), String> {
        let record = match state.execs.lock() {
            Ok(execs) => execs.get(&id).cloned(),
            Err(error) => {
                return write_response(
                    stream,
                    error_response(500, "exec store lock failed", json!(error.to_string())),
                );
            }
        };
        let Some(mut record) = record else {
            return write_response(
                stream,
                error_response(404, "exec instance not found", json!({ "id": id })),
            );
        };

        let (mut command, args, _backend) = match exec_record_command(state, &record) {
            Ok(command) => command,
            Err(error) => {
                return write_response(stream, error_response(500, "exec start failed", error));
            }
        };
        command
            .stdin(if record.attach_stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return write_response(
                    stream,
                    error_response(
                        500,
                        "exec start failed",
                        json!({ "error": error.to_string(), "args": args }),
                    ),
                );
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (tx, rx) = mpsc::channel();
        let mut stdout_reader = stdout.map(|reader| spawn_exec_pipe_reader(reader, 1, tx.clone()));
        let mut stderr_reader = stderr.map(|reader| spawn_exec_pipe_reader(reader, 2, tx));
        let stdin_stop = Arc::new(AtomicBool::new(false));
        let mut stdin_writer = if record.attach_stdin {
            child.stdin.take().and_then(|stdin| {
                stream
                    .try_clone()
                    .ok()
                    .map(|input| spawn_exec_stdin_writer(input, stdin, stdin_stop.clone()))
            })
        } else {
            None
        };

        stream.write_all(
            b"HTTP/1.1 101 UPGRADED\r\nContent-Type: application/vnd.docker.raw-stream\r\nConnection: Upgrade\r\nUpgrade: tcp\r\n\r\n",
        )
        .map_err(|error| format!("write exec upgrade response: {error}"))?;
        stream
            .flush()
            .map_err(|error| format!("flush exec upgrade response: {error}"))?;

        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let exit_code;

        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok((stream_type, chunk)) => {
                    if stream_type == 2 {
                        stderr_bytes.extend_from_slice(&chunk);
                    } else {
                        stdout_bytes.extend_from_slice(&chunk);
                    }
                    if write_docker_stream_frame(stream, stream_type, &chunk).is_err() {
                        let _ = child.kill();
                        let _ = child.wait();
                        stdin_stop.store(true, Ordering::Relaxed);
                        if let Some(handle) = stdin_writer.take() {
                            let _ = handle.join();
                        }
                        exit_code = 124;
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {}
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    stdin_stop.store(true, Ordering::Relaxed);
                    if let Some(handle) = stdin_writer.take() {
                        let _ = handle.join();
                    }
                    if let Some(handle) = stdout_reader.take() {
                        let _ = handle.join();
                    }
                    if let Some(handle) = stderr_reader.take() {
                        let _ = handle.join();
                    }
                    while let Ok((stream_type, chunk)) = rx.try_recv() {
                        if stream_type == 2 {
                            stderr_bytes.extend_from_slice(&chunk);
                        } else {
                            stdout_bytes.extend_from_slice(&chunk);
                        }
                        let _ = write_docker_stream_frame(stream, stream_type, &chunk);
                    }
                    exit_code =
                        exec_exit_code_from_parts(status.code(), &stdout_bytes, &stderr_bytes);
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    exit_code = 126;
                    stdin_stop.store(true, Ordering::Relaxed);
                    if let Some(handle) = stdin_writer.take() {
                        let _ = handle.join();
                    }
                    stderr_bytes
                        .extend_from_slice(format!("exec wait failed: {error}\n").as_bytes());
                    break;
                }
            }
        }

        record.exit_code = Some(exit_code);
        record.stdout = stdout_bytes;
        record.stderr = stderr_bytes;
        if let Ok(mut execs) = state.execs.lock() {
            execs.insert(id, record);
        }
        stdin_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = stdin_writer.take() {
            let _ = handle.join();
        }
        let _ = stream.flush();
        Ok(())
    }

    fn inspect_exec(state: &AdapterState, id: String) -> HttpResponse {
        match state.execs.lock() {
            Ok(execs) => {
                let Some(record) = execs.get(&id) else {
                    return error_response(404, "exec instance not found", json!({ "id": id }));
                };
                json_response(
                    200,
                    json!({
                        "ID": id,
                        "Running": record.exit_code.is_none(),
                        "ExitCode": record.exit_code.unwrap_or(-1),
                        "ProcessConfig": {
                            "entrypoint": record.cmd.first().cloned().unwrap_or_default(),
                            "arguments": record.cmd.iter().skip(1).cloned().collect::<Vec<_>>()
                        },
                        "ContainerID": record.container_id
                    }),
                )
            }
            Err(error) => error_response(500, "exec store lock failed", json!(error.to_string())),
        }
    }

    fn exec_ctr_args(container_name: &str, record: &ExecRecord, exec_id: &str) -> Vec<String> {
        let mut args = vec![
            "tasks".to_string(),
            "exec".to_string(),
            "--exec-id".to_string(),
            exec_id.to_string(),
        ];
        if let Some(workdir) = record.working_dir.clone() {
            args.extend(["--cwd".to_string(), workdir]);
        }
        if record.tty {
            args.push("--tty".to_string());
        }
        args.push(container_name.to_string());
        args.extend(record.cmd.clone());
        args
    }

    fn terminal_ctr_args(container_name: &str, record: &ExecRecord, exec_id: &str) -> Vec<String> {
        let mut args = vec![
            "tasks".to_string(),
            "exec".to_string(),
            "--exec-id".to_string(),
            exec_id.to_string(),
            "--tty".to_string(),
        ];
        if let Some(workdir) = record.working_dir.clone() {
            args.extend(["--cwd".to_string(), workdir]);
        }
        args.push(container_name.to_string());
        args.extend(record.cmd.clone());
        args
    }

    fn exec_record_command(
        state: &AdapterState,
        record: &ExecRecord,
    ) -> Result<(Command, Vec<String>, &'static str), Value> {
        if let Some(pending) = pending_container(state, &record.container_id)
            .map(|pending| refresh_pending_task_state(state, pending))
            .filter(|pending| pending.started_with_ctr && pending.exit_code.is_none())
        {
            let args = exec_ctr_args(
                containerd_task_name(&pending),
                record,
                &unique_task_id("cratebay-exec"),
            );
            let mut command = Command::new(&state.config.ctr);
            command
                .arg("--address")
                .arg(&state.config.containerd_socket)
                .arg("--namespace")
                .arg(&state.config.namespace)
                .args(&args);
            return Ok((command, args, "containerd"));
        }

        Err(json!({
            "backend": "containerd",
            "container": record.container_id,
            "error": "container is not a running CrateBay-managed task",
        }))
    }

    fn terminal_record_command(
        state: &AdapterState,
        record: &ExecRecord,
    ) -> Result<(Command, Vec<String>, &'static str), Value> {
        if let Some(pending) = pending_container(state, &record.container_id)
            .map(|pending| refresh_pending_task_state(state, pending))
            .filter(|pending| pending.started_with_ctr && pending.exit_code.is_none())
        {
            let args = terminal_ctr_args(
                containerd_task_name(&pending),
                record,
                &unique_task_id("cratebay-terminal"),
            );
            let mut command = Command::new(&state.config.ctr);
            command
                .arg("--address")
                .arg(&state.config.containerd_socket)
                .arg("--namespace")
                .arg(&state.config.namespace)
                .args(&args);
            return Ok((command, args, "containerd-tty"));
        }

        Err(json!({
            "backend": "containerd-tty",
            "container": record.container_id,
            "error": "container is not a running CrateBay-managed task",
        }))
    }

    struct TerminalPty {
        master: File,
        slave: File,
    }

    fn terminal_size_from_payload(payload: &Value) -> (u16, u16) {
        let cols = terminal_dimension(
            payload.get("cols").or_else(|| payload.get("columns")),
            80,
            20,
            500,
        );
        let rows = terminal_dimension(payload.get("rows"), 24, 2, 200);
        (cols, rows)
    }

    fn terminal_dimension(value: Option<&Value>, default: u16, min: u16, max: u16) -> u16 {
        value
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value > 0)
            .unwrap_or(default)
            .clamp(min, max)
    }

    fn open_terminal_pty(cols: u16, rows: u16) -> Result<TerminalPty, String> {
        unsafe {
            let master_fd = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
            if master_fd < 0 {
                return Err(format!("posix_openpt: {}", io::Error::last_os_error()));
            }

            if libc::grantpt(master_fd) != 0 {
                let error = io::Error::last_os_error();
                libc::close(master_fd);
                return Err(format!("grantpt: {error}"));
            }
            if libc::unlockpt(master_fd) != 0 {
                let error = io::Error::last_os_error();
                libc::close(master_fd);
                return Err(format!("unlockpt: {error}"));
            }

            let slave_name = libc::ptsname(master_fd);
            if slave_name.is_null() {
                let error = io::Error::last_os_error();
                libc::close(master_fd);
                return Err(format!("ptsname: {error}"));
            }
            let slave_name = CStr::from_ptr(slave_name).to_owned();
            let slave_fd = libc::open(slave_name.as_ptr(), libc::O_RDWR | libc::O_NOCTTY);
            if slave_fd < 0 {
                let error = io::Error::last_os_error();
                libc::close(master_fd);
                return Err(format!("open pty slave: {error}"));
            }

            let master = File::from_raw_fd(master_fd);
            let slave = File::from_raw_fd(slave_fd);
            set_pty_window_size(slave.as_raw_fd(), cols, rows)?;
            Ok(TerminalPty { master, slave })
        }
    }

    fn stdio_from_fd_dup(fd: i32) -> Result<Stdio, String> {
        let duped = unsafe { libc::dup(fd) };
        if duped < 0 {
            return Err(format!("dup pty fd: {}", io::Error::last_os_error()));
        }
        let file = unsafe { File::from_raw_fd(duped) };
        Ok(Stdio::from(file))
    }

    fn set_pty_window_size(fd: i32, cols: u16, rows: u16) -> Result<(), String> {
        let size = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let result = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &size) };
        if result == 0 {
            Ok(())
        } else {
            Err(format!("TIOCSWINSZ: {}", io::Error::last_os_error()))
        }
    }

    #[cfg(test)]
    fn pty_window_size(fd: i32) -> Result<(u16, u16), String> {
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let result = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) };
        if result == 0 {
            Ok((size.ws_col, size.ws_row))
        } else {
            Err(format!("TIOCGWINSZ: {}", io::Error::last_os_error()))
        }
    }

    fn run_exec_record_allow_failure(
        state: &AdapterState,
        record: &ExecRecord,
    ) -> Result<ExecRunResult, Value> {
        run_exec_record_with_limits(state, record, None, None)
    }

    fn run_exec_record_with_limits(
        state: &AdapterState,
        record: &ExecRecord,
        timeout_secs: Option<u64>,
        max_output_bytes: Option<u64>,
    ) -> Result<ExecRunResult, Value> {
        let (mut command, args, backend) = exec_record_command(state, record)?;
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(timeout_secs) = timeout_secs {
            return run_exec_command_with_timeout(
                command,
                args,
                backend,
                Duration::from_secs(timeout_secs),
                max_output_bytes,
            );
        }

        let mut output = command.output().map_err(|error| {
            json!({
                "error": error.to_string(),
                "backend": backend,
                "args": args,
            })
        })?;
        let exit_code = exec_exit_code(&output);
        let stdout_truncated = truncate_bytes(&mut output.stdout, max_output_bytes);
        let stderr_truncated = truncate_bytes(&mut output.stderr, max_output_bytes);
        Ok(ExecRunResult {
            backend,
            args: args.clone(),
            output,
            exit_code,
            timed_out: false,
            stdout_truncated,
            stderr_truncated,
        })
    }

    fn run_exec_command_with_timeout(
        mut command: Command,
        args: Vec<String>,
        backend: &'static str,
        timeout: Duration,
        max_output_bytes: Option<u64>,
    ) -> Result<ExecRunResult, Value> {
        let mut child = command.spawn().map_err(|error| {
            json!({
                "error": error.to_string(),
                "backend": backend,
                "args": args,
            })
        })?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (tx, rx) = mpsc::channel();
        let mut stdout_reader = stdout.map(|reader| spawn_exec_pipe_reader(reader, 1, tx.clone()));
        let mut stderr_reader = stderr.map(|reader| spawn_exec_pipe_reader(reader, 2, tx));
        let deadline = Instant::now() + timeout;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut stdout_truncated = false;
        let mut stderr_truncated = false;

        let (exit_code, timed_out) = loop {
            while let Ok((stream_type, chunk)) = rx.try_recv() {
                if stream_type == 2 {
                    stderr_truncated |= extend_limited_bytes(&mut stderr, &chunk, max_output_bytes);
                } else {
                    stdout_truncated |= extend_limited_bytes(&mut stdout, &chunk, max_output_bytes);
                }
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    if let Some(handle) = stdout_reader.take() {
                        let _ = handle.join();
                    }
                    if let Some(handle) = stderr_reader.take() {
                        let _ = handle.join();
                    }
                    while let Ok((stream_type, chunk)) = rx.try_recv() {
                        if stream_type == 2 {
                            stderr_truncated |=
                                extend_limited_bytes(&mut stderr, &chunk, max_output_bytes);
                        } else {
                            stdout_truncated |=
                                extend_limited_bytes(&mut stdout, &chunk, max_output_bytes);
                        }
                    }
                    break (
                        exec_exit_code_from_parts(status.code(), &stdout, &stderr),
                        false,
                    );
                }
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    if let Some(handle) = stdout_reader.take() {
                        let _ = handle.join();
                    }
                    if let Some(handle) = stderr_reader.take() {
                        let _ = handle.join();
                    }
                    while let Ok((stream_type, chunk)) = rx.try_recv() {
                        if stream_type == 2 {
                            stderr_truncated |=
                                extend_limited_bytes(&mut stderr, &chunk, max_output_bytes);
                        } else {
                            stdout_truncated |=
                                extend_limited_bytes(&mut stdout, &chunk, max_output_bytes);
                        }
                    }
                    let message = format!("command timed out after {}s\n", timeout.as_secs());
                    stderr_truncated |=
                        extend_limited_bytes(&mut stderr, message.as_bytes(), max_output_bytes);
                    break (124, true);
                }
                Err(error) => {
                    let message = format!("exec wait failed: {error}\n");
                    stderr_truncated |=
                        extend_limited_bytes(&mut stderr, message.as_bytes(), max_output_bytes);
                    break (126, false);
                }
            }
        };

        Ok(ExecRunResult {
            backend,
            args,
            output: Output {
                status: std::process::ExitStatus::from_raw((exit_code as i32) << 8),
                stdout,
                stderr,
            },
            exit_code,
            timed_out,
            stdout_truncated,
            stderr_truncated,
        })
    }

    fn extend_limited_bytes(target: &mut Vec<u8>, chunk: &[u8], max_bytes: Option<u64>) -> bool {
        let Some(max_bytes) = max_bytes.and_then(|value| usize::try_from(value).ok()) else {
            target.extend_from_slice(chunk);
            return false;
        };
        if target.len() >= max_bytes {
            return !chunk.is_empty();
        }
        let remaining = max_bytes.saturating_sub(target.len());
        if chunk.len() <= remaining {
            target.extend_from_slice(chunk);
            false
        } else {
            target.extend_from_slice(&chunk[..remaining]);
            true
        }
    }

    fn truncate_bytes(bytes: &mut Vec<u8>, max_bytes: Option<u64>) -> bool {
        let Some(max_bytes) = max_bytes.and_then(|value| usize::try_from(value).ok()) else {
            return false;
        };
        if bytes.len() <= max_bytes {
            false
        } else {
            bytes.truncate(max_bytes);
            true
        }
    }

    fn positive_u64_value(value: Option<&Value>) -> Option<u64> {
        numeric_i64(value)
            .and_then(|value| u64::try_from(value).ok())
            .filter(|value| *value > 0)
    }

    fn terminal_session_id(payload: &Value) -> Option<String> {
        optional_string_value(
            payload
                .get("sessionId")
                .or_else(|| payload.get("session_id"))
                .or_else(|| payload.get("id")),
        )
    }

    fn terminal_session(state: &AdapterState, session_id: &str) -> Option<TerminalSession> {
        state
            .terminals
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).cloned())
    }

    fn terminal_not_found(session_id: &str) -> HttpResponse {
        error_response(
            404,
            "terminal session not found",
            json!({ "sessionId": session_id }),
        )
    }

    fn stop_terminal_session(session: &TerminalSession) {
        if let Ok(mut input) = session.input.lock() {
            match &mut *input {
                TerminalInput::Pipe(stdin) => {
                    if let Some(mut stdin) = stdin.take() {
                        let _ = stdin.write_all(b"exit\n");
                        let _ = stdin.flush();
                    }
                }
                TerminalInput::Pty(master) => {
                    let _ = master.write_all(b"exit\n");
                    let _ = master.flush();
                }
            }
        }
        if let Ok(mut child) = session.child.lock() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }

    fn spawn_terminal_pipe_reader<R>(
        mut reader: R,
        stream: &'static str,
        output: Arc<Mutex<Vec<TerminalOutputChunk>>>,
    ) -> thread::JoinHandle<()>
    where
        R: Read + Send + 'static,
    {
        thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buffer[..n]).into_owned();
                        if let Ok(mut output) = output.lock() {
                            output.push(TerminalOutputChunk { stream, data });
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    }

    fn spawn_terminal_waiter(
        child: Arc<Mutex<Child>>,
        exit_code: Arc<Mutex<Option<i64>>>,
        output: Arc<Mutex<Vec<TerminalOutputChunk>>>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || loop {
            let status = match child.lock() {
                Ok(mut child) => child.try_wait(),
                Err(error) => {
                    if let Ok(mut output) = output.lock() {
                        output.push(TerminalOutputChunk {
                            stream: "stderr",
                            data: format!("terminal child lock failed: {error}\n"),
                        });
                    }
                    if let Ok(mut exit_code) = exit_code.lock() {
                        *exit_code = Some(126);
                    }
                    break;
                }
            };

            match status {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(128) as i64;
                    if let Ok(mut exit_code) = exit_code.lock() {
                        *exit_code = Some(code);
                    }
                    break;
                }
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(error) => {
                    if let Ok(mut output) = output.lock() {
                        output.push(TerminalOutputChunk {
                            stream: "stderr",
                            data: format!("terminal wait failed: {error}\n"),
                        });
                    }
                    if let Ok(mut exit_code) = exit_code.lock() {
                        *exit_code = Some(126);
                    }
                    break;
                }
            }
        })
    }

    fn spawn_exec_stdin_writer(
        mut stream: UnixStream,
        mut stdin: ChildStdin,
        stop: Arc<AtomicBool>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
            let mut buffer = [0u8; 8192];
            while !stop.load(Ordering::Relaxed) {
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if stdin.write_all(&buffer[..n]).is_err() {
                            break;
                        }
                        let _ = stdin.flush();
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock
                                | std::io::ErrorKind::TimedOut
                                | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => break,
                }
            }
        })
    }

    fn spawn_exec_pipe_reader<R>(
        mut reader: R,
        stream_type: u8,
        tx: mpsc::Sender<(u8, Vec<u8>)>,
    ) -> thread::JoinHandle<()>
    where
        R: Read + Send + 'static,
    {
        thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send((stream_type, buffer[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    }

    fn write_docker_stream_frame(
        stream: &mut UnixStream,
        stream_type: u8,
        payload: &[u8],
    ) -> std::io::Result<()> {
        if payload.is_empty() {
            return Ok(());
        }
        let mut frame = Vec::with_capacity(payload.len() + 8);
        frame.push(stream_type);
        frame.extend_from_slice(&[0, 0, 0]);
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(payload);
        stream.write_all(&frame)
    }

    fn list_images(config: &Config) -> HttpResponse {
        match engine_image_values(config) {
            Ok(images) => json_response(200, Value::Array(images)),
            Err(error) => error_response(500, "image list failed", error),
        }
    }

    fn list_cratebay_images(config: &Config) -> HttpResponse {
        match engine_image_values(config) {
            Ok(images) => {
                let items = images
                    .into_iter()
                    .map(super::native_contract::image_summary)
                    .collect::<Vec<_>>();
                json_response(
                    200,
                    json!({
                        "api": "cratebay.images.v1",
                        "count": items.len(),
                        "items": items,
                    }),
                )
            }
            Err(error) => error_response(500, "image list failed", error),
        }
    }

    fn engine_image_values(config: &Config) -> Result<Vec<Value>, Value> {
        let mut images = list_containerd_image_refs(config)?
            .into_iter()
            .map(|image_ref| ctr_image_summary(&image_ref))
            .collect::<Vec<_>>();
        dedupe_image_values(&mut images);
        Ok(images)
    }

    fn ctr_image_summary(image_ref: &str) -> Value {
        let (repository, tag, digest) = image_ref_parts(image_ref);
        let repo_tags = match digest.as_ref() {
            Some(_) if tag.is_empty() => Vec::new(),
            _ => vec![format!("{repository}:{tag}")],
        };
        let repo_digests = digest
            .as_ref()
            .map(|digest| vec![format!("{repository}@{digest}")])
            .unwrap_or_default();
        json!({
            "Containers": -1,
            "Created": 0,
            "Id": digest.clone().unwrap_or_else(|| image_ref.to_string()),
            "Labels": {},
            "ParentId": "",
            "RepoDigests": repo_digests,
            "RepoTags": repo_tags,
            "SharedSize": -1,
            "Size": 0,
            "VirtualSize": 0,
            "CrateBay": {
                "backend": "containerd",
                "imageRef": image_ref,
            },
        })
    }

    fn containerd_image_config(config: &Config, image: &str) -> Result<Value, Value> {
        let image_ref = resolve_containerd_image_ref(config, image)?;
        let target_digest = containerd_image_target_digest(config, &image_ref)?;
        let target = ctr_content_json(config, &target_digest)?;
        let manifest = if target.get("config").is_some() {
            target
        } else {
            let descriptor = select_platform_manifest_descriptor(&target).ok_or_else(|| {
                json!({
                    "backend": "containerd",
                    "imageRef": image_ref,
                    "targetDigest": target_digest,
                    "error": "image index did not contain a usable manifest descriptor",
                })
            })?;
            let manifest_digest = descriptor_digest(&descriptor).ok_or_else(|| {
                json!({
                    "backend": "containerd",
                    "imageRef": image_ref,
                    "targetDigest": target_digest,
                    "descriptor": descriptor,
                    "error": "image manifest descriptor did not include a digest",
                })
            })?;
            ctr_content_json(config, &manifest_digest)?
        };
        let config_descriptor = manifest.get("config").ok_or_else(|| {
            json!({
                "backend": "containerd",
                "imageRef": image_ref,
                "targetDigest": target_digest,
                "error": "image manifest did not include a config descriptor",
            })
        })?;
        let config_digest = descriptor_digest(config_descriptor).ok_or_else(|| {
            json!({
                "backend": "containerd",
                "imageRef": image_ref,
                "targetDigest": target_digest,
                "configDescriptor": config_descriptor,
                "error": "image config descriptor did not include a digest",
            })
        })?;
        ctr_content_json(config, &config_digest)
    }

    fn containerd_image_target_digest(config: &Config, image_ref: &str) -> Result<String, Value> {
        let output = run_ctr(config, vec!["images".to_string(), "list".to_string()])?;
        for line in String::from_utf8_lossy(&output.stdout).lines().skip(1) {
            let mut columns = line.split_whitespace();
            let Some(found_ref) = columns.next() else {
                continue;
            };
            let _media_type = columns.next();
            let Some(digest) = columns.next() else {
                continue;
            };
            if found_ref == image_ref && digest.starts_with("sha256:") {
                return Ok(digest.to_string());
            }
        }
        Err(json!({
            "backend": "containerd",
            "imageRef": image_ref,
            "error": "image target digest was not found in containerd image list",
        }))
    }

    fn ctr_content_json(config: &Config, digest: &str) -> Result<Value, Value> {
        let output = run_ctr(
            config,
            vec!["content".to_string(), "get".to_string(), digest.to_string()],
        )?;
        serde_json::from_slice(&output.stdout).map_err(|error| {
            json!({
                "backend": "containerd",
                "digest": digest,
                "error": format!("content blob was not JSON: {error}"),
            })
        })
    }

    fn select_platform_manifest_descriptor(index: &Value) -> Option<Value> {
        let manifests = index.get("manifests")?.as_array()?;
        let arch = oci_architecture();
        manifests
            .iter()
            .find(|descriptor| descriptor_platform_matches(descriptor, "linux", arch))
            .cloned()
            .or_else(|| {
                manifests
                    .iter()
                    .find(|descriptor| descriptor_platform_os(descriptor) == Some("linux"))
                    .cloned()
            })
            .or_else(|| manifests.first().cloned())
    }

    fn descriptor_platform_matches(descriptor: &Value, os: &str, arch: &str) -> bool {
        descriptor_platform_os(descriptor) == Some(os)
            && nested_value(descriptor, &["platform", "architecture"]).and_then(Value::as_str)
                == Some(arch)
    }

    fn descriptor_platform_os(descriptor: &Value) -> Option<&str> {
        nested_value(descriptor, &["platform", "os"]).and_then(Value::as_str)
    }

    fn descriptor_digest(descriptor: &Value) -> Option<String> {
        string_field(descriptor, &["digest", "Digest"])
    }

    fn image_ref_parts(image_ref: &str) -> (String, String, Option<String>) {
        let trimmed = image_ref.trim();
        let (name, digest) = trimmed
            .split_once('@')
            .map(|(name, digest)| (name, Some(digest.to_string())))
            .unwrap_or((trimmed, None));
        let last_slash = name.rfind('/').unwrap_or(0);
        let tag = name[last_slash..]
            .rfind(':')
            .map(|index| last_slash + index)
            .map(|index| name[index + 1..].to_string());
        match tag {
            Some(tag) => (name[..name.len() - tag.len() - 1].to_string(), tag, digest),
            None => (name.to_string(), "latest".to_string(), digest),
        }
    }

    fn parse_registry_image_ref(image_ref: &str) -> Option<RegistryImageRef> {
        let trimmed = image_ref.trim();
        if trimmed.is_empty() {
            return None;
        }
        let (name, digest) = trimmed
            .split_once('@')
            .map(|(name, digest)| (name, Some(digest.to_string())))
            .unwrap_or((trimmed, None));
        let (registry, repository_with_tag) = name.split_once('/')?;
        if registry.is_empty() || repository_with_tag.is_empty() {
            return None;
        }
        let last_slash = repository_with_tag.rfind('/').unwrap_or(0);
        let tag_index = repository_with_tag[last_slash..]
            .rfind(':')
            .map(|index| last_slash + index);
        let (repository, reference) = match digest {
            Some(digest) => (
                tag_index
                    .map(|index| repository_with_tag[..index].to_string())
                    .unwrap_or_else(|| repository_with_tag.to_string()),
                digest,
            ),
            None => match tag_index {
                Some(index) => (
                    repository_with_tag[..index].to_string(),
                    repository_with_tag[index + 1..].to_string(),
                ),
                None => (repository_with_tag.to_string(), "latest".to_string()),
            },
        };
        if repository.is_empty() || reference.is_empty() {
            return None;
        }
        Some(RegistryImageRef {
            registry: registry.to_string(),
            repository,
            reference,
        })
    }

    fn registry_blob_get(registry: &str, repository: &str, digest: &str) -> Result<Vec<u8>, Value> {
        let path = format!("/v2/{repository}/blobs/{digest}");
        let response =
            registry_http_get(registry, &path, &[("Accept", "application/octet-stream")])?;
        if let Some(header_digest) = response.headers.get("docker-content-digest") {
            if header_digest != digest {
                return Err(json!({
                    "backend": "cratebay-loopback-registry",
                    "registry": registry,
                    "path": path,
                    "expectedDigest": digest,
                    "headerDigest": header_digest,
                    "error": "registry blob digest header did not match descriptor",
                }));
            }
        }
        Ok(response.body)
    }

    fn registry_http_get(
        registry: &str,
        path: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<RegistryHttpResponse, Value> {
        let mut stream = TcpStream::connect(registry).map_err(|error| {
            json!({
                "backend": "cratebay-loopback-registry",
                "registry": registry,
                "path": path,
                "error": error.to_string(),
            })
        })?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

        let mut request = format!(
            "GET {path} HTTP/1.1\r\nHost: {registry}\r\nUser-Agent: cratebay-engine-adapter/{}\r\nConnection: close\r\n",
            env!("CARGO_PKG_VERSION")
        );
        for (name, value) in extra_headers {
            request.push_str(name);
            request.push_str(": ");
            request.push_str(value);
            request.push_str("\r\n");
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes()).map_err(|error| {
            json!({
                "backend": "cratebay-loopback-registry",
                "registry": registry,
                "path": path,
                "error": error.to_string(),
            })
        })?;

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).map_err(|error| {
            json!({
                "backend": "cratebay-loopback-registry",
                "registry": registry,
                "path": path,
                "error": error.to_string(),
            })
        })?;
        let header_end = find_header_end(&raw).ok_or_else(|| {
            json!({
                "backend": "cratebay-loopback-registry",
                "registry": registry,
                "path": path,
                "error": "registry response did not include HTTP headers",
            })
        })?;
        let header_text = String::from_utf8_lossy(&raw[..header_end]);
        let mut lines = header_text.lines();
        let status_line = lines.next().unwrap_or_default();
        let mut status_parts = status_line.splitn(3, ' ');
        let _version = status_parts.next();
        let status = status_parts
            .next()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0);
        let reason = status_parts.next().unwrap_or_default().to_string();
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
            .collect::<HashMap<_, _>>();
        let mut body = raw[header_end + 4..].to_vec();
        if headers
            .get("transfer-encoding")
            .map(|value| value.to_ascii_lowercase().contains("chunked"))
            .unwrap_or(false)
        {
            body = decode_http_chunked_body(&body).map_err(|error| {
                json!({
                    "backend": "cratebay-loopback-registry",
                    "registry": registry,
                    "path": path,
                    "error": error,
                })
            })?;
        } else if let Some(length) = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
        {
            if body.len() < length {
                return Err(json!({
                    "backend": "cratebay-loopback-registry",
                    "registry": registry,
                    "path": path,
                    "contentLength": length,
                    "actualLength": body.len(),
                    "error": "registry response body ended early",
                }));
            }
            body.truncate(length);
        }

        if !(200..300).contains(&status) {
            return Err(json!({
                "backend": "cratebay-loopback-registry",
                "registry": registry,
                "path": path,
                "status": status,
                "reason": reason,
                "body": http_body_preview(&body),
                "error": "registry request failed",
            }));
        }

        Ok(RegistryHttpResponse { headers, body })
    }

    fn decode_http_chunked_body(body: &[u8]) -> Result<Vec<u8>, String> {
        let mut index = 0;
        let mut decoded = Vec::new();
        loop {
            let Some(line_end) = body[index..]
                .windows(2)
                .position(|window| window == b"\r\n")
            else {
                return Err("chunk header was incomplete".to_string());
            };
            let size_line = String::from_utf8_lossy(&body[index..index + line_end]);
            let size_text = size_line
                .split_once(';')
                .map(|(size, _)| size)
                .unwrap_or(size_line.as_ref())
                .trim();
            let size = usize::from_str_radix(size_text, 16)
                .map_err(|error| format!("invalid chunk size '{size_text}': {error}"))?;
            index += line_end + 2;
            if size == 0 {
                return Ok(decoded);
            }
            if body.len() < index + size + 2 {
                return Err("chunk body was incomplete".to_string());
            }
            decoded.extend_from_slice(&body[index..index + size]);
            index += size;
            if body.get(index..index + 2) != Some(b"\r\n") {
                return Err("chunk body was not followed by CRLF".to_string());
            }
            index += 2;
        }
    }

    fn http_body_preview(body: &[u8]) -> String {
        let mut preview = body.to_vec();
        let truncated = truncate_bytes(&mut preview, Some(4096));
        let mut text = String::from_utf8_lossy(&preview).into_owned();
        if truncated {
            text.push_str("...");
        }
        text
    }

    fn sha256_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn dedupe_image_values(images: &mut Vec<Value>) {
        let mut seen = HashSet::new();
        images.retain(|image| {
            let key = string_array(image.get("RepoTags"))
                .into_iter()
                .chain(string_array(image.get("RepoDigests")))
                .next()
                .or_else(|| string_field(image, &["Id", "ID", "Digest"]))
                .unwrap_or_default();
            key.is_empty() || seen.insert(key)
        });
    }

    fn pull_image(config: &Config, query: &HashMap<String, String>) -> HttpResponse {
        let Some(from_image) = query
            .get("fromImage")
            .filter(|value| !value.trim().is_empty())
        else {
            return error_response(400, "fromImage is required", json!(query));
        };
        let image = match query.get("tag").filter(|tag| !tag.trim().is_empty()) {
            Some(tag) if !from_image.contains(':') => format!("{from_image}:{tag}"),
            _ => from_image.clone(),
        };

        match pull_image_with_engine(config, &image, &[]) {
            Ok(result) => {
                let body = format!(
                    "{}{}{}\n",
                    serde_json::to_string(&json!({
                        "status": "Pulled",
                        "id": image,
                        "cratebayBackend": result.backend,
                        "cratebayImageRef": result.image_ref,
                        "cratebayMirror": result.mirror,
                    }))
                    .unwrap(),
                    if result.output.stdout.is_empty() {
                        ""
                    } else {
                        "\n"
                    },
                    String::from_utf8_lossy(&result.output.stdout)
                        .lines()
                        .map(|line| serde_json::to_string(&json!({ "status": line })).unwrap())
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                HttpResponse {
                    status: 200,
                    reason: "OK",
                    content_type: "application/json",
                    upgrade: false,
                    body: body.into_bytes(),
                }
            }
            Err(error) => error_response(500, "image pull failed", error),
        }
    }

    fn pull_image_with_engine(
        config: &Config,
        image: &str,
        mirrors: &[String],
    ) -> Result<ImagePullResult, Value> {
        let mut containerd_errors = Vec::new();
        for mirror in mirrors {
            let mirror = normalize_registry_mirror(mirror);
            if mirror.is_empty() {
                continue;
            }
            let mirror_image = rewrite_image_for_registry_mirror(image, &mirror);
            if mirror_image == image {
                continue;
            }
            match pull_image_direct_with_engine(config, image, &mirror_image, Some(&mirror)) {
                Ok(result) => return Ok(result),
                Err(error) => containerd_errors.push(error),
            }
        }

        match pull_image_direct_with_engine(config, image, image, None) {
            Ok(mut result) => {
                result.containerd_errors.splice(0..0, containerd_errors);
                Ok(result)
            }
            Err(error) => {
                containerd_errors.push(error);
                Err(json!({
                    "backend": "containerd",
                    "image": image,
                    "containerd": containerd_errors,
                }))
            }
        }
    }

    fn pull_image_direct_with_engine(
        config: &Config,
        requested_image: &str,
        pull_image: &str,
        mirror: Option<&str>,
    ) -> Result<ImagePullResult, Value> {
        let mut containerd_errors = Vec::new();
        if mirror.is_none() && loopback_registry_host(pull_image).is_some() {
            match pull_loopback_registry_image(config, requested_image, pull_image) {
                Ok(result) => return Ok(result),
                Err(error) => containerd_errors.push(error),
            }
        }
        for image_ref in ctr_image_pull_candidates(pull_image) {
            let args = match ctr_image_pull_args(&image_ref) {
                Ok(args) => args,
                Err(error) => {
                    containerd_errors.push(error);
                    continue;
                }
            };
            match run_ctr(config, args) {
                Ok(output) => {
                    let tagged_ref =
                        tag_pulled_mirror_image(config, requested_image, &image_ref, mirror);
                    let image_ref = tagged_ref.unwrap_or(image_ref);
                    return Ok(ImagePullResult {
                        backend: "containerd",
                        image_ref,
                        mirror: mirror.map(ToString::to_string),
                        output,
                        containerd_errors,
                    });
                }
                Err(error) => containerd_errors.push(error),
            }
        }

        Err(json!({
            "backend": "containerd",
            "image": requested_image,
            "pullImage": pull_image,
            "mirror": mirror,
            "containerd": containerd_errors,
        }))
    }

    fn tag_pulled_mirror_image(
        config: &Config,
        requested_image: &str,
        pulled_ref: &str,
        mirror: Option<&str>,
    ) -> Option<String> {
        mirror?;
        let target_ref = ctr_image_for_run(requested_image);
        if target_ref.trim().is_empty() || target_ref == pulled_ref {
            return Some(pulled_ref.to_string());
        }

        match run_ctr(
            config,
            vec![
                "images".to_string(),
                "tag".to_string(),
                pulled_ref.to_string(),
                target_ref.clone(),
            ],
        ) {
            Ok(_) => Some(target_ref),
            Err(error) => {
                eprintln!(
                    "cratebay-engine-adapter: failed to tag pulled mirror image {} as {}: {}",
                    pulled_ref,
                    target_ref,
                    serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
                );
                None
            }
        }
    }

    fn pull_loopback_registry_image(
        config: &Config,
        requested_image: &str,
        pull_image: &str,
    ) -> Result<ImagePullResult, Value> {
        let image_ref = ctr_image_for_run(requested_image);
        let registry_ref = parse_registry_image_ref(pull_image).ok_or_else(|| {
            json!({
                "backend": "cratebay-loopback-registry",
                "image": pull_image,
                "error": "image reference did not include a registry, repository, and tag or digest",
            })
        })?;
        if loopback_registry_host(pull_image).is_none() {
            return Err(json!({
                "backend": "cratebay-loopback-registry",
                "image": pull_image,
                "error": "registry is not loopback",
            }));
        }

        let work_dir = temp_work_dir("cratebay-loopback-registry-pull", &image_ref);
        let result = (|| {
            let manifest_path = format!(
                "/v2/{}/manifests/{}",
                registry_ref.repository, registry_ref.reference
            );
            let manifest_response = registry_http_get(
                &registry_ref.registry,
                &manifest_path,
                &[(
                    "Accept",
                    "application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.manifest.v1+json",
                )],
            )?;
            let manifest_digest = manifest_response
                .headers
                .get("docker-content-digest")
                .cloned()
                .unwrap_or_else(|| format!("sha256:{}", sha256_bytes(&manifest_response.body)));
            let manifest: Value =
                serde_json::from_slice(&manifest_response.body).map_err(|error| {
                    json!({
                        "backend": "cratebay-loopback-registry",
                        "image": pull_image,
                        "path": manifest_path,
                        "error": format!("manifest was not JSON: {error}"),
                    })
                })?;
            if manifest
                .get("manifests")
                .and_then(Value::as_array)
                .is_some()
            {
                return Err(json!({
                    "backend": "cratebay-loopback-registry",
                    "image": pull_image,
                    "manifestDigest": manifest_digest,
                    "error": "OCI image indexes from loopback registries are not supported yet",
                }));
            }

            let config_descriptor = manifest.get("config").ok_or_else(|| {
                json!({
                    "backend": "cratebay-loopback-registry",
                    "image": pull_image,
                    "manifestDigest": manifest_digest,
                    "error": "manifest did not include a config descriptor",
                })
            })?;
            let config_digest = descriptor_digest(config_descriptor).ok_or_else(|| {
                json!({
                    "backend": "cratebay-loopback-registry",
                    "image": pull_image,
                    "manifestDigest": manifest_digest,
                    "configDescriptor": config_descriptor,
                    "error": "config descriptor did not include a digest",
                })
            })?;
            let layers = manifest
                .get("layers")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    json!({
                        "backend": "cratebay-loopback-registry",
                        "image": pull_image,
                        "manifestDigest": manifest_digest,
                        "error": "manifest did not include layers",
                    })
                })?;

            let image_dir = work_dir.join("image");
            fs::create_dir_all(&image_dir).map_err(|error| {
                json!({
                    "backend": "cratebay-loopback-registry",
                    "path": image_dir.display().to_string(),
                    "error": error.to_string(),
                })
            })?;

            let config_bytes = registry_blob_get(
                &registry_ref.registry,
                &registry_ref.repository,
                &config_digest,
            )?;
            let config_hash = sha256_bytes(&config_bytes);
            if config_digest != format!("sha256:{config_hash}") {
                return Err(json!({
                    "backend": "cratebay-loopback-registry",
                    "image": pull_image,
                    "digest": config_digest,
                    "actualDigest": format!("sha256:{config_hash}"),
                    "error": "config digest mismatch",
                }));
            }
            let config_name = format!("{config_hash}.json");
            fs::write(image_dir.join(&config_name), config_bytes).map_err(|error| {
                json!({
                    "backend": "cratebay-loopback-registry",
                    "path": image_dir.join(&config_name).display().to_string(),
                    "error": error.to_string(),
                })
            })?;

            let mut layer_digests = Vec::new();
            let mut layer_paths = Vec::new();
            for layer in layers {
                let layer_digest = descriptor_digest(layer).ok_or_else(|| {
                    json!({
                        "backend": "cratebay-loopback-registry",
                        "image": pull_image,
                        "manifestDigest": manifest_digest,
                        "layerDescriptor": layer,
                        "error": "layer descriptor did not include a digest",
                    })
                })?;
                let layer_bytes = registry_blob_get(
                    &registry_ref.registry,
                    &registry_ref.repository,
                    &layer_digest,
                )?;
                let layer_hash = sha256_bytes(&layer_bytes);
                if layer_digest != format!("sha256:{layer_hash}") {
                    return Err(json!({
                        "backend": "cratebay-loopback-registry",
                        "image": pull_image,
                        "digest": layer_digest,
                        "actualDigest": format!("sha256:{layer_hash}"),
                        "error": "layer digest mismatch",
                    }));
                }
                let layer_dir = image_dir.join(&layer_hash);
                fs::create_dir_all(&layer_dir).map_err(|error| {
                    json!({
                        "backend": "cratebay-loopback-registry",
                        "path": layer_dir.display().to_string(),
                        "error": error.to_string(),
                    })
                })?;
                fs::write(layer_dir.join("VERSION"), b"1.0").map_err(|error| {
                    json!({
                        "backend": "cratebay-loopback-registry",
                        "path": layer_dir.join("VERSION").display().to_string(),
                        "error": error.to_string(),
                    })
                })?;
                write_json_file(
                    &layer_dir.join("json"),
                    &json!({
                        "id": layer_hash,
                        "created": chrono_like_now(),
                        "container_config": {},
                    }),
                )?;
                fs::write(layer_dir.join("layer.tar"), layer_bytes).map_err(|error| {
                    json!({
                        "backend": "cratebay-loopback-registry",
                        "path": layer_dir.join("layer.tar").display().to_string(),
                        "error": error.to_string(),
                    })
                })?;
                layer_paths.push(format!("{layer_hash}/layer.tar"));
                layer_digests.push(layer_hash);
            }

            let (repository, tag, _) = image_ref_parts(&image_ref);
            write_json_file(
                &image_dir.join("manifest.json"),
                &json!([
                    {
                        "Config": config_name.clone(),
                        "RepoTags": [image_ref.clone()],
                        "Layers": layer_paths,
                    }
                ]),
            )?;
            let mut tags = serde_json::Map::new();
            tags.insert(
                tag.clone(),
                json!(layer_digests
                    .last()
                    .cloned()
                    .unwrap_or_else(|| config_hash.clone())),
            );
            let mut repositories = serde_json::Map::new();
            repositories.insert(repository.clone(), Value::Object(tags));
            write_json_file(
                &image_dir.join("repositories"),
                &Value::Object(repositories),
            )?;

            let archive_path = work_dir.join("image.tar");
            write_registry_archive_tar(&image_dir, &archive_path, &config_name, &layer_digests)?;
            let output = run_ctr(
                config,
                vec![
                    "images".to_string(),
                    "import".to_string(),
                    archive_path.display().to_string(),
                ],
            )
            .map_err(|error| {
                json!({
                    "backend": "cratebay-loopback-registry",
                    "image": pull_image,
                    "archive": archive_path.display().to_string(),
                    "error": error,
                })
            })?;
            let mut stdout = output.stdout;
            stdout.extend_from_slice(
                format!(
                    "pulled {image_ref} from local registry {registry} ({manifest_digest})\n",
                    registry = registry_ref.registry
                )
                .as_bytes(),
            );

            Ok(ImagePullResult {
                backend: "cratebay-loopback-registry",
                image_ref,
                mirror: None,
                output: Output {
                    status: output.status,
                    stdout,
                    stderr: output.stderr,
                },
                containerd_errors: Vec::new(),
            })
        })();
        let _ = fs::remove_dir_all(&work_dir);
        result
    }

    fn list_containerd_image_refs(config: &Config) -> Result<Vec<String>, Value> {
        let output = run_ctr(
            config,
            vec!["images".to_string(), "list".to_string(), "-q".to_string()],
        )?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    }

    fn resolve_containerd_image_ref(config: &Config, image: &str) -> Result<String, Value> {
        let refs = list_containerd_image_refs(config)?;
        select_containerd_image_ref_from_refs(refs.iter().map(String::as_str), image).ok_or_else(
            || {
                json!({
                    "backend": "containerd",
                    "image": image,
                    "error": "image was not found in the CrateBay containerd namespace",
                })
            },
        )
    }

    fn select_containerd_image_ref_from_refs<'a, I>(refs: I, image: &str) -> Option<String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        refs.into_iter()
            .map(str::trim)
            .find(|found| image_ref_matches(found, image))
            .map(str::to_string)
    }

    fn image_ref_matches(found: &str, wanted: &str) -> bool {
        let found = found.trim();
        let wanted = wanted.trim();
        if found.is_empty() || wanted.is_empty() {
            return false;
        }
        if found == wanted {
            return true;
        }

        if let Some(alias) = docker_compat_image_id_alias(wanted) {
            return image_ref_matches(found, alias);
        }

        let (_, _, digest) = image_ref_parts(found);
        if digest.as_deref() == Some(wanted)
            || normalize_image_id(wanted) == digest.clone().unwrap_or_default()
        {
            return true;
        }

        ctr_image_candidates(wanted)
            .iter()
            .any(|candidate| found == candidate || image_refs_equivalent(found, candidate))
    }

    fn docker_compat_image_id_alias(image: &str) -> Option<&str> {
        let alias = image.strip_prefix("sha256:")?;
        if alias.contains('/') && image_name_has_tag(alias) {
            Some(alias)
        } else {
            None
        }
    }

    fn load_image(config: &Config, body: &[u8]) -> HttpResponse {
        if body.is_empty() {
            return error_response(400, "image load body is empty", json!({}));
        }

        let archive_path = temp_archive_path("cratebay-image-load");
        if let Err(error) = fs::write(&archive_path, body) {
            return error_response(
                500,
                "image load failed",
                json!({ "path": archive_path.display().to_string(), "error": error.to_string() }),
            );
        }

        let result = run_ctr(
            config,
            vec![
                "images".to_string(),
                "import".to_string(),
                archive_path.display().to_string(),
            ],
        );
        let _ = fs::remove_file(&archive_path);
        match result {
            Ok(output) => json_stream_response(&output),
            Err(error) => error_response(
                500,
                "image load failed",
                json!({ "backend": "containerd", "error": error }),
            ),
        }
    }

    fn export_images(
        config: &Config,
        raw_path: &str,
        query: &HashMap<String, String>,
    ) -> HttpResponse {
        let mut names = query_values(raw_path, "names");
        if names.is_empty() {
            if let Some(name) = query.get("name").or_else(|| query.get("names")) {
                names.push(name.clone());
            }
        }
        let names = names
            .into_iter()
            .flat_map(expand_image_names_value)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();

        if names.is_empty() {
            return error_response(400, "at least one image name is required", json!(query));
        }

        export_image_names(config, names)
    }

    fn export_image_names(config: &Config, names: Vec<String>) -> HttpResponse {
        let archive_path = temp_archive_path("cratebay-image-export");
        let refs = match names
            .iter()
            .map(|name| resolve_containerd_image_ref(config, name))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(refs) => refs,
            Err(error) => return error_response(404, "image export failed", error),
        };

        match run_ctr(config, ctr_image_export_args(&archive_path, &refs)) {
            Ok(_) => match fs::read(&archive_path) {
                Ok(body) => {
                    let _ = fs::remove_file(&archive_path);
                    HttpResponse {
                        status: 200,
                        reason: "OK",
                        content_type: "application/x-tar",
                        upgrade: false,
                        body,
                    }
                }
                Err(error) => error_response(
                    500,
                    "image export archive could not be read",
                    json!({ "path": archive_path.display().to_string(), "error": error.to_string() }),
                ),
            },
            Err(error) => {
                let _ = fs::remove_file(&archive_path);
                error_response(
                    500,
                    "image export failed",
                    json!({ "backend": "containerd", "images": names, "error": error }),
                )
            }
        }
    }

    const ENGINE_TMP_ROOT: &str = "/var/lib/cratebay-engine/tmp";

    fn engine_temp_root() -> PathBuf {
        overridable_root("CRATEBAY_ENGINE_TMP_ROOT", ENGINE_TMP_ROOT, "tmp")
    }

    fn temp_archive_path(prefix: &str) -> PathBuf {
        let root = engine_temp_root();
        let _ = fs::create_dir_all(&root);
        root.join(format!("{prefix}-{}.tar", now_millis()))
    }

    fn ctr_image_export_args(archive_path: &Path, refs: &[String]) -> Vec<String> {
        let mut args = vec![
            "images".to_string(),
            "export".to_string(),
            archive_path.display().to_string(),
        ];
        args.extend(refs.iter().cloned());
        args
    }

    fn tag_image(config: &Config, source: String, query: &HashMap<String, String>) -> HttpResponse {
        let Some(repo) = query.get("repo").filter(|value| !value.trim().is_empty()) else {
            return error_response(400, "repo is required", json!(query));
        };
        let target = match query.get("tag").filter(|value| !value.trim().is_empty()) {
            Some(tag) if !repo.contains(':') => format!("{repo}:{tag}"),
            _ => repo.clone(),
        };

        let source_ref = match resolve_containerd_image_ref(config, &source) {
            Ok(source_ref) => source_ref,
            Err(error) => return error_response(404, "image tag failed", error),
        };
        let target_ref = ctr_image_for_run(&target);

        match run_ctr(
            config,
            vec![
                "images".to_string(),
                "tag".to_string(),
                source_ref,
                target_ref,
            ],
        ) {
            Ok(_) => empty_response(201),
            Err(error) => error_response(
                500,
                "image tag failed",
                json!({ "backend": "containerd", "source": source, "target": target, "error": error }),
            ),
        }
    }

    fn commit_container(config: &Config, query: &HashMap<String, String>) -> HttpResponse {
        let Some(container) = query
            .get("container")
            .filter(|value| !value.trim().is_empty())
            .cloned()
        else {
            return error_response(400, "container is required", json!(query));
        };
        let Some(repo) = query.get("repo").filter(|value| !value.trim().is_empty()) else {
            return error_response(400, "repo is required", json!(query));
        };
        let image = match query.get("tag").filter(|value| !value.trim().is_empty()) {
            Some(tag) if !repo.contains(':') => format!("{repo}:{tag}"),
            _ => repo.clone(),
        };

        let Some(pending) = read_pending_container_record(&container) else {
            return error_response(
                404,
                "container commit failed",
                json!({
                    "container": container,
                    "image": image,
                    "backend": "containerd",
                    "error": "CrateBay-managed container was not found",
                }),
            );
        };
        match commit_container_rootfs_to_image(config, &pending, &image) {
            Ok(result) => json_response(
                201,
                json!({
                    "Id": result.target_ref,
                    "CrateBay": {
                        "backend": "containerd",
                        "container": pending.name,
                        "targetImage": image,
                        "mode": "rootfs-archive",
                        "layerDigest": format!("sha256:{}", result.layer_digest),
                        "configDigest": format!("sha256:{}", result.config_digest),
                        "rootfs": result.rootfs.display().to_string(),
                    },
                }),
            ),
            Err(error) => error_response(
                500,
                "container commit failed",
                json!({
                    "container": pending.name,
                    "image": image,
                    "backend": "containerd",
                    "error": error,
                }),
            ),
        }
    }

    fn commit_container_rootfs_to_image(
        config: &Config,
        pending: &PendingContainer,
        image: &str,
    ) -> Result<ContainerCommitResult, Value> {
        if !pending.started_with_ctr || pending.exit_code.is_some() {
            return Err(json!({
                "container": pending.name,
                "state": pending_state(pending).0,
                "error": "only running CrateBay containerd tasks can be packed into an image right now",
            }));
        }

        let rootfs = resolve_container_rootfs(config, pending)?;
        let target_ref = ctr_image_for_run(image);
        let (repository, tag, _) = image_ref_parts(&target_ref);
        let work_dir = temp_work_dir("cratebay-container-commit", &pending.name);

        let result = (|| {
            let image_dir = work_dir.join("image");
            fs::create_dir_all(&image_dir).map_err(|error| {
                json!({
                    "path": image_dir.display().to_string(),
                    "error": error.to_string(),
                })
            })?;

            let layer_tmp = work_dir.join("layer.tar");
            write_rootfs_layer_tar(&rootfs, &layer_tmp)?;
            let layer_digest = sha256_file(&layer_tmp)?;
            let layer_dir = image_dir.join(&layer_digest);
            fs::create_dir_all(&layer_dir).map_err(|error| {
                json!({
                    "path": layer_dir.display().to_string(),
                    "error": error.to_string(),
                })
            })?;
            let layer_tar = layer_dir.join("layer.tar");
            fs::rename(&layer_tmp, &layer_tar).map_err(|error| {
                json!({
                    "from": layer_tmp.display().to_string(),
                    "to": layer_tar.display().to_string(),
                    "error": error.to_string(),
                })
            })?;
            fs::write(layer_dir.join("VERSION"), b"1.0").map_err(|error| {
                json!({
                    "path": layer_dir.join("VERSION").display().to_string(),
                    "error": error.to_string(),
                })
            })?;

            let created = chrono_like_now();
            let layer_json = json!({
                "id": layer_digest.clone(),
                "created": created,
                "container_config": {
                    "Cmd": pending.command.clone(),
                },
            });
            write_json_file(&layer_dir.join("json"), &layer_json)?;

            let config_json = docker_archive_config(pending, &created, &layer_digest);
            let config_tmp = image_dir.join("config.json.tmp");
            write_json_file(&config_tmp, &config_json)?;
            let config_digest = sha256_file(&config_tmp)?;
            let config_name = format!("{config_digest}.json");
            fs::rename(&config_tmp, image_dir.join(&config_name)).map_err(|error| {
                json!({
                    "from": config_tmp.display().to_string(),
                    "to": image_dir.join(&config_name).display().to_string(),
                    "error": error.to_string(),
                })
            })?;

            write_json_file(
                &image_dir.join("manifest.json"),
                &json!([
                    {
                        "Config": config_name.clone(),
                        "RepoTags": [target_ref.clone()],
                        "Layers": [format!("{}/layer.tar", layer_digest)],
                    }
                ]),
            )?;
            let mut tags = serde_json::Map::new();
            tags.insert(tag.clone(), json!(layer_digest.clone()));
            let mut repositories = serde_json::Map::new();
            repositories.insert(repository.clone(), Value::Object(tags));
            write_json_file(
                &image_dir.join("repositories"),
                &Value::Object(repositories),
            )?;

            let archive_path = work_dir.join("image.tar");
            write_docker_archive_tar(&image_dir, &archive_path, &config_name, &layer_digest)?;
            run_ctr(
                config,
                vec![
                    "images".to_string(),
                    "import".to_string(),
                    archive_path.display().to_string(),
                ],
            )
            .map_err(|error| {
                json!({
                    "archive": archive_path.display().to_string(),
                    "targetImage": target_ref.clone(),
                    "error": error,
                })
            })?;

            Ok(ContainerCommitResult {
                target_ref,
                layer_digest,
                config_digest,
                rootfs,
            })
        })();

        let _ = fs::remove_dir_all(&work_dir);
        result
    }

    fn docker_archive_config(
        pending: &PendingContainer,
        created: &str,
        layer_digest: &str,
    ) -> Value {
        json!({
            "created": created,
            "architecture": oci_architecture(),
            "os": "linux",
            "config": {
                "Env": pending.env.clone(),
                "Cmd": pending.command.clone(),
                "WorkingDir": pending.working_dir.clone().unwrap_or_else(|| "/".to_string()),
            },
            "container_config": {
                "Env": pending.env.clone(),
                "Cmd": pending.command.clone(),
                "WorkingDir": pending.working_dir.clone().unwrap_or_else(|| "/".to_string()),
            },
            "rootfs": {
                "type": "layers",
                "diff_ids": [format!("sha256:{layer_digest}")],
            },
            "history": [
                {
                    "created": created,
                    "created_by": "cratebay engine pack-container",
                }
            ],
        })
    }

    fn oci_architecture() -> &'static str {
        match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        }
    }

    fn resolve_container_rootfs(
        config: &Config,
        pending: &PendingContainer,
    ) -> Result<PathBuf, Value> {
        let candidates = container_rootfs_candidates(config, pending);
        candidates
            .iter()
            .find(|path| path.is_dir())
            .cloned()
            .ok_or_else(|| {
                json!({
                    "container": pending.name,
                    "backend": "containerd",
                    "error": "running container rootfs was not found",
                    "candidates": candidates
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>(),
                })
            })
    }

    fn container_rootfs_candidates(config: &Config, pending: &PendingContainer) -> Vec<PathBuf> {
        containerd_task_runtime_dirs(config, pending)
            .into_iter()
            .map(|path| path.join("rootfs"))
            .collect()
    }

    fn containerd_task_runtime_dirs(config: &Config, pending: &PendingContainer) -> Vec<PathBuf> {
        let state_root = config
            .containerd_socket
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/run/containerd"));
        let task_root = state_root
            .join("io.containerd.runtime.v2.task")
            .join(&config.namespace);
        let mut names = vec![containerd_task_name(pending).to_string()];
        if !names.iter().any(|name| name == &pending.name) {
            names.push(pending.name.clone());
        }
        if !names.iter().any(|name| name == &pending.id) {
            names.push(pending.id.clone());
        }
        names.into_iter().map(|name| task_root.join(name)).collect()
    }

    fn containerd_task_name(pending: &PendingContainer) -> &str {
        if !pending.runtime_id.is_empty() {
            pending.runtime_id.as_str()
        } else if pending.started_with_ctr && pending.id != pending.name {
            pending.id.as_str()
        } else {
            pending.name.as_str()
        }
    }

    fn remove_non_force_wait_timeout() -> Duration {
        #[cfg(test)]
        {
            Duration::from_millis(10)
        }
        #[cfg(not(test))]
        {
            Duration::from_secs(10)
        }
    }

    fn temp_work_dir(prefix: &str, name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}-{}",
            prefix,
            now_millis(),
            safe_network_file_name(name)
        ))
    }

    fn write_json_file(path: &Path, value: &Value) -> Result<(), Value> {
        fs::write(
            path,
            serde_json::to_vec_pretty(value).unwrap_or_else(|_| b"{}".to_vec()),
        )
        .map_err(|error| {
            json!({
                "path": path.display().to_string(),
                "error": error.to_string(),
            })
        })
    }

    fn sha256_file(path: &Path) -> Result<String, Value> {
        let mut file = File::open(path).map_err(|error| {
            json!({
                "path": path.display().to_string(),
                "error": error.to_string(),
            })
        })?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let bytes_read = file.read(&mut buffer).map_err(|error| {
                json!({
                    "path": path.display().to_string(),
                    "error": error.to_string(),
                })
            })?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn write_rootfs_layer_tar(rootfs: &Path, archive_path: &Path) -> Result<(), Value> {
        let file = File::create(archive_path).map_err(|error| {
            json!({
                "path": archive_path.display().to_string(),
                "error": error.to_string(),
            })
        })?;
        let mut builder = tar::Builder::new(file);
        append_rootfs_entries(&mut builder, rootfs, rootfs)?;
        builder.finish().map_err(|error| {
            json!({
                "path": archive_path.display().to_string(),
                "error": error.to_string(),
            })
        })
    }

    fn append_rootfs_entries(
        builder: &mut tar::Builder<File>,
        rootfs: &Path,
        path: &Path,
    ) -> Result<(), Value> {
        let rel = path.strip_prefix(rootfs).unwrap_or(path);
        if !rel.as_os_str().is_empty() {
            if rootfs_archive_skip(rel) {
                return Ok(());
            }
            append_rootfs_entry(builder, rel, path)?;
        }

        if path.is_dir() {
            let mut children = fs::read_dir(path)
                .map_err(|error| {
                    json!({
                        "path": path.display().to_string(),
                        "error": error.to_string(),
                    })
                })?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                append_rootfs_entries(builder, rootfs, &child)?;
            }
        }
        Ok(())
    }

    fn append_rootfs_entry(
        builder: &mut tar::Builder<File>,
        rel: &Path,
        path: &Path,
    ) -> Result<(), Value> {
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            json!({
                "path": path.display().to_string(),
                "error": error.to_string(),
            })
        })?;
        let file_type = metadata.file_type();
        let mut header = tar::Header::new_gnu();
        header.set_path(rel).map_err(
            |error| json!({ "path": rel.display().to_string(), "error": error.to_string() }),
        )?;
        header.set_mode(metadata.permissions().mode() & 0o7777);
        header.set_mtime(metadata_mtime(&metadata));
        header.set_uid(0);
        header.set_gid(0);

        if file_type.is_dir() {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_cksum();
            builder.append_data(&mut header, rel, io::empty()).map_err(
                |error| json!({ "path": path.display().to_string(), "error": error.to_string() }),
            )?;
            return Ok(());
        }

        if file_type.is_symlink() {
            let target = fs::read_link(path).map_err(|error| {
                json!({
                    "path": path.display().to_string(),
                    "error": error.to_string(),
                })
            })?;
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_link_name(&target).map_err(
                |error| json!({ "path": path.display().to_string(), "error": error.to_string() }),
            )?;
            header.set_cksum();
            builder.append(&header, io::empty()).map_err(
                |error| json!({ "path": path.display().to_string(), "error": error.to_string() }),
            )?;
            return Ok(());
        }

        if file_type.is_file() {
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(metadata.len());
            header.set_cksum();
            let file = File::open(path).map_err(|error| {
                json!({
                    "path": path.display().to_string(),
                    "error": error.to_string(),
                })
            })?;
            builder.append_data(&mut header, rel, file).map_err(
                |error| json!({ "path": path.display().to_string(), "error": error.to_string() }),
            )?;
        }

        Ok(())
    }

    fn metadata_mtime(metadata: &fs::Metadata) -> u64 {
        metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or_default()
    }

    fn rootfs_archive_skip(rel: &Path) -> bool {
        rel.components()
            .next()
            .and_then(|component| match component {
                std::path::Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .map(|top| matches!(top, "dev" | "proc" | "run" | "sys"))
            .unwrap_or(false)
    }

    fn write_docker_archive_tar(
        image_dir: &Path,
        archive_path: &Path,
        config_name: &str,
        layer_digest: &str,
    ) -> Result<(), Value> {
        write_registry_archive_tar(
            image_dir,
            archive_path,
            config_name,
            &[layer_digest.to_string()],
        )
    }

    fn write_registry_archive_tar(
        image_dir: &Path,
        archive_path: &Path,
        config_name: &str,
        layer_digests: &[String],
    ) -> Result<(), Value> {
        let file = File::create(archive_path).map_err(|error| {
            json!({
                "path": archive_path.display().to_string(),
                "error": error.to_string(),
            })
        })?;
        let mut builder = tar::Builder::new(file);
        append_archive_file(&mut builder, image_dir, "manifest.json")?;
        append_archive_file(&mut builder, image_dir, "repositories")?;
        append_archive_file(&mut builder, image_dir, config_name)?;
        for layer_digest in layer_digests {
            builder
                .append_dir(layer_digest, image_dir.join(layer_digest))
                .map_err(|error| json!({ "path": layer_digest, "error": error.to_string() }))?;
            append_archive_file(&mut builder, image_dir, &format!("{layer_digest}/VERSION"))?;
            append_archive_file(&mut builder, image_dir, &format!("{layer_digest}/json"))?;
            append_archive_file(
                &mut builder,
                image_dir,
                &format!("{layer_digest}/layer.tar"),
            )?;
        }
        builder.finish().map_err(|error| {
            json!({
                "path": archive_path.display().to_string(),
                "error": error.to_string(),
            })
        })
    }

    fn append_archive_file(
        builder: &mut tar::Builder<File>,
        root: &Path,
        relative: &str,
    ) -> Result<(), Value> {
        builder
            .append_path_with_name(root.join(relative), relative)
            .map_err(|error| json!({ "path": relative, "error": error.to_string() }))
    }

    fn inspect_image(config: &Config, id: String) -> HttpResponse {
        match resolve_containerd_image_ref(config, &id) {
            Ok(image_ref) => json_response(
                200,
                normalize_image_inspect(ctr_image_summary(&image_ref), &id),
            ),
            Err(error) => error_response(404, "image inspect failed", error),
        }
    }

    fn remove_image(config: &Config, id: String, query: &HashMap<String, String>) -> HttpResponse {
        let image_ref = match resolve_containerd_image_ref(config, &id) {
            Ok(image_ref) => image_ref,
            Err(_) => id.clone(),
        };
        match run_ctr(
            config,
            vec!["images".to_string(), "rm".to_string(), image_ref.clone()],
        ) {
            Ok(_) => json_response(200, json!([{ "Deleted": id }])),
            Err(error) => error_response(
                500,
                "image remove failed",
                json!({
                    "backend": "containerd",
                    "id": id,
                    "imageRef": image_ref,
                    "forceRequested": query_bool(query, "force"),
                    "error": error,
                }),
            ),
        }
    }

    fn create_volume(body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let name = string_value(payload.get("Name"));
        if !valid_local_name(&name) {
            return error_response(400, "volume name is required", payload);
        }
        if let Err(error) = fs::create_dir_all(volume_data_path(&name)) {
            return error_response(
                500,
                "volume create failed",
                json!({ "name": name, "error": error.to_string() }),
            );
        }
        let metadata = json!({
            "Name": name,
            "Driver": optional_string_value(payload.get("Driver"))
                .unwrap_or_else(|| "local".to_string()),
            "Labels": object_or_empty(payload.get("Labels")),
            "Options": object_or_empty(payload.get("Options")),
            "CreatedAt": chrono_like_now(),
        });
        if let Err(error) = write_json_file(&volume_metadata_path(&name), &metadata) {
            return error_response(500, "volume metadata write failed", error);
        }
        json_response(201, volume_value(&name))
    }

    fn list_volumes() -> HttpResponse {
        let mut volumes = Vec::new();
        let root = volume_root();
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if valid_local_name(&name) {
                    volumes.push(volume_value(&name));
                }
            }
        }
        volumes.sort_by(|a, b| {
            string_field(a, &["Name"])
                .unwrap_or_default()
                .cmp(&string_field(b, &["Name"]).unwrap_or_default())
        });
        json_response(200, json!({ "Volumes": volumes, "Warnings": [] }))
    }

    fn list_cratebay_volumes() -> HttpResponse {
        let items = cratebay_volume_items();
        json_response(
            200,
            json!({
                "api": "cratebay.volumes.v1",
                "count": items.len(),
                "items": items,
            }),
        )
    }

    fn cratebay_volume_items() -> Vec<Value> {
        let mut items = Vec::new();
        let root = volume_root();
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if valid_local_name(&name) {
                    let mut item = super::native_contract::volume_summary(volume_value(&name));
                    if let Some(object) = item.as_object_mut() {
                        object.insert(
                            "dataPath".to_string(),
                            json!(volume_data_path(&name).display().to_string()),
                        );
                        object.insert(
                            "sizeBytes".to_string(),
                            json!(directory_size_bytes(&volume_data_path(&name))),
                        );
                        object.insert("managedBy".to_string(), json!("cratebay"));
                    }
                    items.push(item);
                }
            }
        }
        items.sort_by(|a, b| {
            string_field(a, &["name"])
                .unwrap_or_default()
                .cmp(&string_field(b, &["name"]).unwrap_or_default())
        });
        items
    }

    fn storage_gc_candidates(state: &AdapterState, prune_exited_containers: bool) -> Vec<Value> {
        if !prune_exited_containers {
            return Vec::new();
        }

        let mut candidates = unique_pending_containers(state)
            .into_iter()
            .filter(|pending| pending.exit_code.is_some())
            .map(|pending| {
                let record_path = pending_container_registry_path(&pending.name);
                let log_bytes = file_size_bytes(&pending.log_path);
                let record_bytes = file_size_bytes(&record_path);
                json!({
                    "kind": "exited-container-metadata",
                    "id": pending.id,
                    "name": pending.name,
                    "backend": "containerd",
                    "exitCode": pending.exit_code,
                    "bytes": log_bytes + record_bytes,
                    "logPath": pending.log_path.display().to_string(),
                    "recordPath": record_path.display().to_string(),
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            string_field(a, &["name"])
                .unwrap_or_default()
                .cmp(&string_field(b, &["name"]).unwrap_or_default())
        });
        candidates
    }

    fn inspect_volume(name: String) -> HttpResponse {
        if !valid_local_name(&name) {
            return error_response(404, "volume not found", json!({ "name": name }));
        }
        if !volume_data_path(&name).exists() {
            return error_response(404, "volume not found", json!({ "name": name }));
        }
        json_response(200, volume_value(&name))
    }

    fn remove_volume(name: String) -> HttpResponse {
        if !valid_local_name(&name) {
            return error_response(404, "volume not found", json!({ "name": name }));
        }
        match fs::remove_dir_all(volume_path(&name)) {
            Ok(()) => empty_response(204),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => empty_response(204),
            Err(error) => error_response(
                500,
                "volume remove failed",
                json!({ "name": name, "error": error.to_string() }),
            ),
        }
    }

    fn list_networks(_config: &Config, query: &HashMap<String, String>) -> HttpResponse {
        let filters = parse_filters(query.get("filters"));
        let networks = managed_network_values()
            .into_iter()
            .filter(|network| network_matches_filters(network, &filters))
            .collect::<Vec<_>>();
        json_response(200, Value::Array(networks))
    }

    fn list_cratebay_networks(_config: &Config, query: &HashMap<String, String>) -> HttpResponse {
        let filters = parse_filters(query.get("filters"));
        let mut items = managed_network_values()
            .into_iter()
            .filter(|network| network_matches_filters(network, &filters))
            .map(super::native_contract::network_summary)
            .collect::<Vec<_>>();
        dedupe_native_items_by_name(&mut items);

        json_response(
            200,
            json!({
                "api": "cratebay.networks.v1",
                "count": items.len(),
                "items": items,
            }),
        )
    }

    fn create_network(config: &Config, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let name = string_value(payload.get("Name"));
        if name.is_empty() {
            return error_response(400, "network name is required", payload);
        }

        match create_managed_network(config, &payload) {
            Ok(created) => {
                let id = string_value(created.get("Id"));
                json_response(
                    201,
                    json!({ "Id": if id.is_empty() { name } else { id }, "Warning": "" }),
                )
            }
            Err(error) => error_response(500, "network create failed", error),
        }
    }

    fn create_managed_network(_config: &Config, payload: &Value) -> Result<Value, Value> {
        let name = string_value(payload.get("Name"));
        if !valid_local_name(&name) {
            return Err(json!({ "name": name, "error": "invalid network name" }));
        }
        let driver =
            optional_string_value(payload.get("Driver")).unwrap_or_else(|| "bridge".to_string());
        if driver != "bridge" {
            return Err(json!({
                "name": name,
                "driver": driver,
                "error": "only bridge networks are supported by the CrateBay CNI engine today",
            }));
        }
        let network = managed_network_value(&name, payload);
        fs::create_dir_all(network_root()).map_err(|error| {
            json!({ "name": name, "error": error.to_string(), "path": network_root().display().to_string() })
        })?;
        fs::create_dir_all(cni_config_root()).map_err(|error| {
            json!({ "name": name, "error": error.to_string(), "path": cni_config_root().display().to_string() })
        })?;
        fs::write(
            network_registry_path(&name),
            serde_json::to_vec_pretty(&network).unwrap_or_else(|_| b"{}".to_vec()),
        )
        .map_err(|error| {
            json!({
                "name": name,
                "error": error.to_string(),
                "path": network_registry_path(&name).display().to_string(),
            })
        })?;
        fs::write(
            cni_config_path(&name),
            serde_json::to_vec_pretty(&managed_network_cni_config(&name, payload))
                .unwrap_or_else(|_| b"{}".to_vec()),
        )
        .map_err(|error| {
            json!({
                "name": name,
                "error": error.to_string(),
                "path": cni_config_path(&name).display().to_string(),
            })
        })?;
        Ok(network)
    }

    fn remove_managed_network(_config: &Config, id: &str) -> Result<(), Value> {
        if !valid_local_name(id) {
            return Err(json!({ "name": id, "error": "invalid network name" }));
        }
        let mut errors = Vec::new();
        for path in [network_registry_path(id), cni_config_path(id)] {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => errors.push(json!({
                    "path": path.display().to_string(),
                    "error": error.to_string(),
                })),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(json!({ "name": id, "errors": errors }))
        }
    }

    fn inspect_network(state: &AdapterState, id: String) -> HttpResponse {
        match inspect_network_value(&state.config, &id) {
            Ok(value) => json_response(
                200,
                normalize_network_inspect_with_pending(state, value, &id),
            ),
            Err(error) => error_response(404, "network inspect failed", error),
        }
    }

    fn remove_network(config: &Config, id: String) -> HttpResponse {
        if managed_network_value_by_id(&id).is_some() {
            return match remove_managed_network(config, &id) {
                Ok(()) => empty_response(204),
                Err(error) => error_response(500, "network remove failed", error),
            };
        }
        error_response(
            404,
            "CrateBay-managed network not found",
            json!({ "network": id, "backend": "cratebay-cni" }),
        )
    }

    fn connect_network(state: &AdapterState, network: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let container = string_value(payload.get("Container"));
        if container.is_empty() {
            return error_response(400, "container is required", payload);
        }

        if let Some(pending) = pending_container(state, &container) {
            let pending = refresh_pending_task_state(state, pending);
            if pending.started_with_ctr && pending.exit_code.is_none() {
                return connect_running_container_network(state, pending, network);
            }
            if pending.started_with_ctr {
                return error_response(
                    409,
                    "container already exited; remove and recreate it with the requested network",
                    json!({
                        "container": container,
                        "network": network,
                        "exitCode": pending.exit_code,
                    }),
                );
            }
            if managed_network_value_by_id(&network).is_none() {
                return error_response(
                    404,
                    "CrateBay-managed network not found",
                    json!({ "network": network, "container": container }),
                );
            }
            update_pending_network(
                state,
                &pending.id,
                &pending.name,
                Some(network),
                endpoint_config_aliases(&payload),
            );
            return empty_response(200);
        }

        error_response(
            404,
            "CrateBay-managed container not found",
            json!({
                "container": container,
                "network": network,
                "backend": "cratebay-cni",
            }),
        )
    }

    fn disconnect_network(state: &AdapterState, network: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let container = string_value(payload.get("Container"));
        if container.is_empty() {
            return error_response(400, "container is required", payload);
        }

        if let Some(pending) = pending_container(state, &container) {
            let pending = refresh_pending_task_state(state, pending);
            if pending.started_with_ctr && pending.exit_code.is_none() {
                return disconnect_running_container_network(state, pending, network);
            }
            if pending.started_with_ctr {
                return error_response(
                    409,
                    "container already exited; remove and recreate it with the requested network",
                    json!({
                        "container": container,
                        "network": network,
                        "exitCode": pending.exit_code,
                    }),
                );
            }
            if pending.network.as_deref() == Some(network.as_str()) {
                update_pending_network(state, &pending.id, &pending.name, None, Vec::new());
            }
            return empty_response(200);
        }

        error_response(
            404,
            "CrateBay-managed container not found",
            json!({
                "container": container,
                "network": network,
                "backend": "cratebay-cni",
            }),
        )
    }

    fn connect_running_container_network(
        state: &AdapterState,
        pending: PendingContainer,
        network: String,
    ) -> HttpResponse {
        if pending.network.as_deref() == Some(network.as_str()) {
            return empty_response(200);
        }
        if pending.network.is_some() {
            return error_response(
                409,
                "container is already attached to a CrateBay network",
                json!({
                    "container": pending.name,
                    "currentNetwork": pending.network,
                    "requestedNetwork": network,
                    "action": "disconnect the current network before attaching another one",
                }),
            );
        }
        let Some(network_value) = managed_network_value_by_id(&network) else {
            return error_response(
                404,
                "CrateBay-managed network not found",
                json!({ "network": network, "container": pending.name }),
            );
        };
        let Some(attachment) = running_network_attachment(&pending, &network) else {
            return error_response(
                501,
                "running container was not started in a CrateBay network namespace",
                json!({
                    "container": pending.name,
                    "network": network,
                    "workaround": "create the container with a CrateBay pod/network selected before starting it",
                }),
            );
        };
        match run_cni_chain(&state.config, "ADD", &pending, &attachment, &network_value) {
            Ok(()) => {
                update_pending_network_attachment(
                    state,
                    &pending.id,
                    &pending.name,
                    Some(network),
                    Some(&attachment),
                );
                empty_response(200)
            }
            Err(error) => error_response(
                500,
                "running container network attach failed",
                json!({
                    "container": pending.name,
                    "network": attachment.network,
                    "netns": attachment.netns_path.display().to_string(),
                    "error": error,
                }),
            ),
        }
    }

    fn disconnect_running_container_network(
        state: &AdapterState,
        pending: PendingContainer,
        network: String,
    ) -> HttpResponse {
        if pending.network.as_deref() != Some(network.as_str()) {
            return empty_response(200);
        }
        let Some(network_value) = managed_network_value_by_id(&network) else {
            return error_response(
                404,
                "CrateBay-managed network not found",
                json!({ "network": network, "container": pending.name }),
            );
        };
        let attachment = running_network_attachment(&pending, &network)
            .unwrap_or_else(|| pending_network_attachment(&pending, &network));
        match run_cni_chain(&state.config, "DEL", &pending, &attachment, &network_value) {
            Ok(()) => {
                update_pending_network_attachment(
                    state,
                    &pending.id,
                    &pending.name,
                    None,
                    Some(&attachment),
                );
                empty_response(200)
            }
            Err(error) => error_response(
                500,
                "running container network detach failed",
                json!({
                    "container": pending.name,
                    "network": attachment.network,
                    "netns": attachment.netns_path.display().to_string(),
                    "error": error,
                }),
            ),
        }
    }

    fn unsupported_response(request: HttpRequest, path: String) -> HttpResponse {
        let body = if request.body.is_empty() {
            request
                .body_spool_path
                .as_ref()
                .map(|path| format!("<spooled body: {}>", path.display()))
        } else {
            Some(String::from_utf8_lossy(&request.body).into_owned())
        };
        let message = format!(
            "CrateBay containerd adapter endpoint is not implemented yet: {} {}",
            request.method, path
        );
        error_response(
            501,
            &message,
            json!({
                "method": request.method.clone(),
                "path": request.path.clone(),
                "normalizedPath": path,
                "body": body,
            }),
        )
    }

    fn version_payload(config: &Config) -> Value {
        json!({
            "Platform": { "Name": "CrateBay Runtime" },
            "Components": [
                {
                    "Name": "CrateBay Engine Adapter",
                    "Version": env!("CARGO_PKG_VERSION"),
                    "Details": {
                        "Engine": "containerd",
                        "Namespace": config.namespace,
                        "ContainerdSocket": config.containerd_socket.display().to_string(),
                    },
                }
            ],
            "Version": format!("cratebay-containerd-{}", env!("CARGO_PKG_VERSION")),
            "ApiVersion": "1.44",
            "MinAPIVersion": "1.24",
            "GitCommit": "cratebay",
            "GoVersion": "",
            "Os": "linux",
            "Arch": std::env::consts::ARCH,
            "KernelVersion": read_kernel_version(),
            "Experimental": true,
        })
    }

    fn cratebay_engine_payload(config: &Config) -> Value {
        super::engine_contract::payload(super::engine_contract::EnginePayloadInput {
            socket: config.socket.display().to_string(),
            containerd_socket: config.containerd_socket.display().to_string(),
            namespace: config.namespace.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    fn cratebay_substrate_payload(state: &AdapterState) -> Value {
        let containers = unique_pending_containers(state);
        let running_tasks = containers
            .iter()
            .filter(|pending| pending.started_with_ctr && pending.exit_code.is_none())
            .count();
        let exited_tasks = containers
            .iter()
            .filter(|pending| pending.exit_code.is_some())
            .count();
        let networks = managed_network_values();
        let volumes = cratebay_volume_items();
        let volume_bytes = volumes
            .iter()
            .filter_map(|volume| volume.get("sizeBytes").and_then(Value::as_u64))
            .sum::<u64>();
        let terminal_sessions = state
            .terminals
            .lock()
            .map(|sessions| sessions.len())
            .unwrap_or_default();
        let exec_records = state
            .execs
            .lock()
            .map(|records| records.len())
            .unwrap_or_default();
        let gc_candidates = storage_gc_candidates(state, true);
        let reclaimable_bytes = gc_candidates
            .iter()
            .filter_map(|candidate| candidate.get("bytes").and_then(Value::as_u64))
            .sum::<u64>();

        json!({
            "api": "cratebay.substrate.v1",
            "engine": "CrateBay Engine",
            "managedBy": "cratebay",
            "daemon": {
                "docker": "none",
                "compatibilityEndpoint": state.config.socket.display().to_string(),
                "compatibilityOnly": true,
            },
            "vm": {
                "managedBy": "cratebay",
                "runtime": "cratebay-managed-vm",
            },
            "shim": {
                "manager": "cratebay-containerd-shim",
                "backend": "containerd task service",
                "namespace": state.config.namespace,
                "containerdSocket": state.config.containerd_socket.display().to_string(),
                "runningTasks": running_tasks,
                "exitedTasks": exited_tasks,
                "pendingRecords": containers.len(),
                "terminalSessions": terminal_sessions,
                "execRecords": exec_records,
                "lifecycle": "cratebay-managed",
            },
            "network": {
                "manager": "cratebay-cni",
                "stack": "CNI",
                "configRoot": cni_config_root().display().to_string(),
                "registryRoot": network_root().display().to_string(),
                "networkCount": networks.len(),
                "ipam": "host-local",
                "portForwarding": "CNI portmap",
            },
            "storage": {
                "manager": "cratebay-storage",
                "volumeRoot": volume_root().display().to_string(),
                "containerRegistryRoot": pending_container_registry_root().display().to_string(),
                "volumeCount": volumes.len(),
                "volumeBytes": volume_bytes,
                "reclaimableBytes": reclaimable_bytes,
                "gc": {
                    "api": "cratebay.storage.gc.v1",
                    "dryRunDefault": true,
                    "candidateCount": gc_candidates.len(),
                },
            },
            "compatibility": {
                "dockerCompatible": true,
                "dockerDaemon": false,
                "purpose": "client compatibility only",
            },
        })
    }

    fn native_storage_gc(state: &AdapterState, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let apply = bool_value(payload.get("apply"));
        let prune_exited = payload
            .get("pruneExitedContainers")
            .map(|value| bool_value(Some(value)))
            .unwrap_or(true);
        let candidates = storage_gc_candidates(state, prune_exited);
        let reclaimable_bytes = candidates
            .iter()
            .filter_map(|candidate| candidate.get("bytes").and_then(Value::as_u64))
            .sum::<u64>();

        let mut removed = Vec::new();
        let mut errors = Vec::new();
        if apply {
            for candidate in &candidates {
                let id = string_value(candidate.get("id"));
                let name = string_value(candidate.get("name"));
                let log_path = optional_string_value(candidate.get("logPath"));
                if let Some(path) = log_path {
                    match fs::remove_file(&path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => errors.push(json!({
                            "path": path,
                            "error": error.to_string(),
                        })),
                    }
                }
                if !id.is_empty() && !name.is_empty() {
                    remove_pending_container(state, &id, &name);
                    removed.push(candidate.clone());
                }
            }
        }

        json_response(
            200,
            json!({
                "api": "cratebay.storage.gc.v1",
                "managedBy": "cratebay",
                "backend": "cratebay-storage",
                "applied": apply,
                "dryRun": !apply,
                "pruneExitedContainers": prune_exited,
                "candidateCount": candidates.len(),
                "reclaimableBytes": reclaimable_bytes,
                "candidates": candidates,
                "removed": removed,
                "errors": errors,
            }),
        )
    }

    fn list_cratebay_shim_tasks(state: &AdapterState) -> HttpResponse {
        let mut items = unique_pending_containers(state)
            .into_iter()
            .map(|pending| shim_task_summary(&pending))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| {
            string_field(a, &["name"])
                .unwrap_or_default()
                .cmp(&string_field(b, &["name"]).unwrap_or_default())
        });
        json_response(
            200,
            json!({
                "api": "cratebay.shim.tasks.v1",
                "manager": "cratebay-containerd-shim",
                "backend": "containerd",
                "managedBy": "cratebay",
                "count": items.len(),
                "items": items,
            }),
        )
    }

    fn native_reap_shim_task(state: &AdapterState, id: String, body: &[u8]) -> HttpResponse {
        let payload = parse_json_body(body);
        let apply = bool_value(payload.get("apply"));
        let Some(pending) =
            pending_container(state, &id).map(|pending| refresh_pending_task_state(state, pending))
        else {
            return error_response(
                404,
                "CrateBay shim task not found",
                json!({ "id": id, "backend": "containerd" }),
            );
        };
        if pending.exit_code.is_none() {
            return error_response(
                409,
                "CrateBay shim task is not exited",
                json!({
                    "api": "cratebay.shim.task.reap.v1",
                    "id": pending.id,
                    "name": pending.name,
                    "state": shim_task_summary(&pending)["state"].clone(),
                    "hint": "stop/remove the task before reaping exited metadata",
                }),
            );
        }

        let bytes = file_size_bytes(&pending.log_path)
            + file_size_bytes(&pending_container_registry_path(&pending.name));
        if apply {
            match fs::remove_file(&pending.log_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return error_response(
                        500,
                        "CrateBay shim task log cleanup failed",
                        json!({
                            "path": pending.log_path.display().to_string(),
                            "error": error.to_string(),
                        }),
                    );
                }
            }
            remove_pending_container(state, &pending.id, &pending.name);
        }

        json_response(
            200,
            json!({
                "api": "cratebay.shim.task.reap.v1",
                "backend": "containerd",
                "manager": "cratebay-containerd-shim",
                "managedBy": "cratebay",
                "applied": apply,
                "dryRun": !apply,
                "id": pending.id,
                "name": pending.name,
                "reclaimableBytes": bytes,
            }),
        )
    }

    fn info_payload(config: &Config) -> Value {
        json!({
            "ID": "cratebay-containerd",
            "Containers": 0,
            "Images": 0,
            "Driver": "containerd",
            "Name": "cratebay-runtime",
            "ServerVersion": format!("cratebay-containerd-{}", env!("CARGO_PKG_VERSION")),
            "OperatingSystem": "CrateBay Runtime",
            "OSType": "linux",
            "Architecture": std::env::consts::ARCH,
            "Warnings": [
                "CrateBay Engine is containerd-first; some compatibility API endpoints are still being ported."
            ],
            "CrateBay": {
                "engine": "containerd",
                "namespace": config.namespace,
                "containerdSocket": config.containerd_socket.display().to_string(),
            },
        })
    }

    fn list_containers(state: &AdapterState) -> Result<Value, Value> {
        let containers = unique_pending_containers(state)
            .into_iter()
            .map(|pending| pending_summary_value(&pending))
            .collect::<Vec<_>>();
        Ok(Value::Array(containers))
    }

    fn list_cratebay_containers(state: &AdapterState) -> HttpResponse {
        match list_containers(state) {
            Ok(Value::Array(containers)) => {
                let mut items = containers
                    .into_iter()
                    .map(super::native_contract::container_summary)
                    .collect::<Vec<_>>();
                dedupe_native_items_by_name(&mut items);
                json_response(
                    200,
                    json!({
                        "api": "cratebay.containers.v1",
                        "count": items.len(),
                        "items": items,
                    }),
                )
            }
            Ok(other) => error_response(
                500,
                "container list returned unexpected payload",
                json!({ "payload": other }),
            ),
            Err(error) => error_response(500, "container list failed", error),
        }
    }

    fn parse_json_body(body: &[u8]) -> Value {
        if body.is_empty() {
            return json!({});
        }

        serde_json::from_slice(body).unwrap_or_else(|_| json!({}))
    }

    fn now_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    }

    fn unique_task_id(prefix: &str) -> String {
        static TASK_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = TASK_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{prefix}-{}-{sequence}", now_millis())
    }

    fn now_seconds() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default()
    }

    fn string_value(value: Option<&Value>) -> String {
        optional_string_value(value).unwrap_or_default()
    }

    fn optional_string_value(value: Option<&Value>) -> Option<String> {
        match value? {
            Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        }
    }

    fn bool_value(value: Option<&Value>) -> bool {
        match value {
            Some(Value::Bool(value)) => *value,
            Some(Value::Number(number)) => number.as_i64().unwrap_or_default() != 0,
            Some(Value::String(text)) => matches!(
                text.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        }
    }

    fn entrypoint_array(value: Option<&Value>) -> Vec<String> {
        match value {
            Some(Value::Array(_)) => string_array(value),
            Some(Value::String(text)) if !text.trim().is_empty() => vec![text.trim().to_string()],
            _ => Vec::new(),
        }
    }

    fn pending_network(value: Option<&Value>) -> Option<String> {
        let Some(Value::Object(host_config)) = value else {
            return None;
        };
        optional_string_value(host_config.get("NetworkMode")).filter(|network| {
            !matches!(
                network.as_str(),
                "" | "default" | "bridge" | "none" | "host" | "container"
            )
        })
    }

    fn pending_labels(payload: &Value) -> serde_json::Map<String, Value> {
        object_map_or_empty(payload.get("Labels").or_else(|| payload.get("labels")))
    }

    fn pending_network_aliases(payload: &Value, name: &str, network: Option<&str>) -> Vec<String> {
        let mut aliases = Vec::new();
        push_host_alias(&mut aliases, name);
        if let Some(service) = nested_string(payload, &["Labels", "com.docker.compose.service"]) {
            push_host_alias(&mut aliases, &service);
        }

        if let Some(Value::Object(endpoints)) =
            nested_value(payload, &["NetworkingConfig", "EndpointsConfig"])
        {
            if let Some(network) = network {
                if let Some(endpoint) = endpoints.get(network) {
                    extend_endpoint_aliases(&mut aliases, endpoint);
                }
            }
            for endpoint in endpoints.values() {
                extend_endpoint_aliases(&mut aliases, endpoint);
            }
        }

        aliases
    }

    fn endpoint_config_aliases(payload: &Value) -> Vec<String> {
        let mut aliases = Vec::new();
        if let Some(endpoint) = payload.get("EndpointConfig") {
            extend_endpoint_aliases(&mut aliases, endpoint);
        }
        aliases
    }

    fn extend_endpoint_aliases(aliases: &mut Vec<String>, endpoint: &Value) {
        for alias in string_array(endpoint.get("Aliases")) {
            push_host_alias(aliases, &alias);
        }
        for alias in string_array(endpoint.get("DNSNames")) {
            push_host_alias(aliases, &alias);
        }
    }

    fn push_host_alias(aliases: &mut Vec<String>, alias: &str) {
        let alias = alias.trim().trim_start_matches('/');
        if !valid_host_alias(alias) || aliases.iter().any(|existing| existing == alias) {
            return;
        }
        aliases.push(alias.to_string());
    }

    fn valid_host_alias(alias: &str) -> bool {
        !alias.is_empty()
            && alias.len() <= 253
            && !alias.contains(char::is_whitespace)
            && !alias.contains('/')
            && alias
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    }

    fn pending_privileged(value: Option<&Value>) -> bool {
        let Some(Value::Object(host_config)) = value else {
            return false;
        };
        bool_value(
            host_config
                .get("Privileged")
                .or_else(|| host_config.get("privileged")),
        )
    }

    fn pending_port_mappings(value: Option<&Value>) -> Vec<CniPortMapping> {
        let Some(Value::Object(host_config)) = value else {
            return Vec::new();
        };
        let Some(Value::Object(bindings)) = host_config.get("PortBindings") else {
            return Vec::new();
        };

        let mut mappings = Vec::new();
        for (container_port, binding) in bindings {
            let Some((port, protocol)) = parse_container_port_proto(container_port) else {
                continue;
            };
            match binding {
                Value::Array(items) if items.is_empty() => {
                    mappings.push(CniPortMapping {
                        host_ip: None,
                        host_port: port,
                        container_port: port,
                        protocol,
                    });
                }
                Value::Array(items) => {
                    for item in items {
                        let host_port = optional_string_value(item.get("HostPort"))
                            .and_then(|value| value.parse::<u16>().ok())
                            .unwrap_or(port);
                        let host_ip = optional_string_value(item.get("HostIp"))
                            .or_else(|| optional_string_value(item.get("HostIP")));
                        mappings.push(CniPortMapping {
                            host_ip,
                            host_port,
                            container_port: port,
                            protocol: protocol.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
        mappings
    }

    fn parse_container_port_proto(value: &str) -> Option<(u16, String)> {
        let (port, protocol) = value.split_once('/').unwrap_or((value, "tcp"));
        let port = port.parse::<u16>().ok()?;
        let protocol = match protocol.trim().to_ascii_lowercase().as_str() {
            "" => "tcp".to_string(),
            "tcp" | "udp" | "sctp" => protocol.trim().to_ascii_lowercase(),
            _ => "tcp".to_string(),
        };
        Some((port, protocol))
    }

    #[cfg(test)]
    fn build_network_connect_args(network: &str, container: &str, payload: &Value) -> Vec<String> {
        let mut args = vec!["network".to_string(), "connect".to_string()];
        if let Some(ipv4) = nested_string(payload, &["EndpointConfig", "IPAMConfig", "IPv4Address"])
        {
            args.extend(["--ip".to_string(), ipv4]);
        }
        if let Some(ipv6) = nested_string(payload, &["EndpointConfig", "IPAMConfig", "IPv6Address"])
        {
            args.extend(["--ip6".to_string(), ipv6]);
        }
        for alias in string_array(nested_value(payload, &["EndpointConfig", "Aliases"])) {
            args.extend(["--alias".to_string(), alias]);
        }
        args.extend([network.to_string(), container.to_string()]);
        args
    }

    #[cfg(test)]
    fn build_network_disconnect_args(
        network: &str,
        container: &str,
        payload: &Value,
    ) -> Vec<String> {
        let mut args = vec!["network".to_string(), "disconnect".to_string()];
        if bool_value(payload.get("Force")) {
            args.push("--force".to_string());
        }
        args.extend([network.to_string(), container.to_string()]);
        args
    }

    fn string_array(value: Option<&Value>) -> Vec<String> {
        match value {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| optional_string_value(Some(item)))
                .collect(),
            Some(Value::String(text)) if !text.trim().is_empty() => {
                vec![text.trim().to_string()]
            }
            _ => Vec::new(),
        }
    }

    fn run_ctr(config: &Config, args: Vec<String>) -> Result<Output, Value> {
        let output = run_ctr_allow_failure(config, args.clone())?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(json!({
                "exitCode": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
                "ctr": config.ctr,
                "args": args,
            }))
        }
    }

    fn run_ctr_allow_failure(config: &Config, args: Vec<String>) -> Result<Output, Value> {
        let mut command = Command::new(&config.ctr);
        command
            .arg("--address")
            .arg(&config.containerd_socket)
            .arg("--namespace")
            .arg(&config.namespace)
            .args(&args);

        command.output().map_err(|error| {
            json!({
                "error": error.to_string(),
                "ctr": config.ctr,
                "args": args,
            })
        })
    }

    fn ctr_output_value(output: &Output) -> Value {
        json!({
            "exitCode": output.status.code(),
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
        })
    }

    fn spawn_ctr_runner(
        state: AdapterState,
        id: String,
        pending: PendingContainer,
    ) -> Result<(), Value> {
        let mut run_pending = pending.clone();
        configure_buildkit_proxy_worker(&mut run_pending, runtime_http_proxy_url().as_deref())?;
        run_pending
            .mounts
            .extend(ensure_container_system_mounts_for_state(
                &state,
                &run_pending,
            )?);
        ensure_pending_mount_sources(&run_pending)?;
        let image = ensure_image_for_run(&state.config, &pending)?;
        let attachment = prepare_pending_network(&state.config, &run_pending)?;
        let create_args = build_ctr_container_create_args_with_netns(
            &run_pending,
            &image,
            attachment
                .as_ref()
                .map(|attachment| attachment.netns_path.as_path()),
        );
        if let Err(error) = run_ctr(&state.config, create_args.clone()) {
            cleanup_pending_network(&state.config, &pending, attachment.as_ref());
            cleanup_containerd_pending_artifacts(&state.config, &run_pending);
            return Err(json!({
                "error": error,
                "ctr": state.config.ctr,
                "args": create_args,
            }));
        }
        if let Err(error) = patch_containerd_snapshot_compat(&state.config, &run_pending) {
            let _ = append_log_bytes(
                &pending.log_path,
                format!("CrateBay compatibility patch skipped: {error}\n").as_bytes(),
            );
        }

        let start_args = build_ctr_task_start_args(&run_pending);
        let mut command = Command::new(&state.config.ctr);
        command
            .arg("--address")
            .arg(&state.config.containerd_socket)
            .arg("--namespace")
            .arg(&state.config.namespace)
            .args(&start_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            cleanup_pending_network(&state.config, &pending, attachment.as_ref());
            cleanup_containerd_pending_artifacts(&state.config, &run_pending);
            json!({
                "error": error.to_string(),
                "ctr": state.config.ctr,
                "args": start_args,
            })
        })?;
        let stdout_reader = child
            .stdout
            .take()
            .map(|reader| spawn_container_log_reader(reader, pending.log_path.clone()));
        let stderr_reader = child
            .stderr
            .take()
            .map(|reader| spawn_container_log_reader(reader, pending.log_path.clone()));
        if let Some(attachment) = attachment.as_ref() {
            update_pending_network_attachment(
                &state,
                &id,
                &pending.name,
                pending.network.clone(),
                Some(attachment),
            );
            if let Some(network) = pending.network.as_deref() {
                refresh_network_hosts_for_network(&state, network);
            }
        }
        let thread_state = state.clone();

        thread::spawn(move || {
            let exit_code = match child.wait() {
                Ok(status) => status.code().map(i64::from).unwrap_or(126),
                Err(error) => {
                    let _ = append_log_bytes(
                        &pending.log_path,
                        format!("ctr run failed: {error}\n").as_bytes(),
                    );
                    126
                }
            };
            if let Some(handle) = stdout_reader {
                let _ = handle.join();
            }
            if let Some(handle) = stderr_reader {
                let _ = handle.join();
            }
            cleanup_pending_network(&thread_state.config, &pending, attachment.as_ref());
            cleanup_containerd_pending_artifacts(&thread_state.config, &pending);
            mark_pending_exit_code(&thread_state, &id, &pending.name, exit_code);
            if let Some(network) = pending.network.as_deref() {
                refresh_network_hosts_for_network(&thread_state, network);
            }
        });

        Ok(())
    }

    fn ensure_image_for_run(config: &Config, pending: &PendingContainer) -> Result<String, Value> {
        match run_ctr(
            config,
            vec!["images".to_string(), "list".to_string(), "-q".to_string()],
        ) {
            Ok(output) => {
                if let Some(existing) = select_containerd_image_ref(&output.stdout, &pending.image)
                {
                    return Ok(existing);
                }
            }
            Err(error) => {
                append_log_bytes(
                    &pending.log_path,
                    format!("CrateBay image lookup failed, pulling image: {error}\n").as_bytes(),
                )
                .ok();
            }
        }

        append_log_bytes(
            &pending.log_path,
            format!("CrateBay pulling image {}\n", pending.image).as_bytes(),
        )
        .ok();
        if pending.no_pull {
            return Err(json!({
                "image": pending.image,
                "error": "image not found locally and noPull is enabled",
            }));
        }
        match pull_image_with_engine(config, &pending.image, &pending.registry_mirrors) {
            Ok(result) => {
                append_log_bytes(
                    &pending.log_path,
                    format!(
                        "CrateBay image ready: {} ({}){}\n",
                        result.image_ref,
                        result.backend,
                        result
                            .mirror
                            .as_ref()
                            .map(|mirror| format!(" via {mirror}"))
                            .unwrap_or_default()
                    )
                    .as_bytes(),
                )
                .ok();
                Ok(result.image_ref)
            }
            Err(error) => Err(json!({
                "image": pending.image,
                "error": error,
            })),
        }
    }

    fn select_containerd_image_ref(bytes: &[u8], image: &str) -> Option<String> {
        select_containerd_image_ref_from_refs(String::from_utf8_lossy(bytes).lines(), image)
    }

    fn image_refs_equivalent(found: &str, wanted: &str) -> bool {
        let found = found.split_once('@').map(|(name, _)| name).unwrap_or(found);
        let wanted = wanted
            .split_once('@')
            .map(|(name, _)| name)
            .unwrap_or(wanted);
        found == wanted
    }

    fn spawn_container_log_reader<R>(mut reader: R, log_path: PathBuf) -> thread::JoinHandle<()>
    where
        R: Read + Send + 'static,
    {
        thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(bytes_read) => {
                        let _ = append_log_bytes(&log_path, &buffer[..bytes_read]);
                    }
                    Err(_) => break,
                }
            }
        })
    }

    fn append_log_bytes(log_path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        file.write_all(bytes)
    }

    #[cfg(test)]
    fn build_ctr_run_args(pending: &PendingContainer, image: &str) -> Vec<String> {
        build_ctr_run_args_with_netns(pending, image, None)
    }

    fn build_ctr_container_create_args(pending: &PendingContainer, image: &str) -> Vec<String> {
        build_ctr_container_create_args_with_netns(pending, image, None)
    }

    fn build_ctr_container_create_args_with_netns(
        pending: &PendingContainer,
        image: &str,
        netns_path: Option<&Path>,
    ) -> Vec<String> {
        let mut args = vec![
            "container".to_string(),
            "create".to_string(),
            "--no-pivot".to_string(),
        ];
        if let Some(netns_path) = netns_path {
            args.extend([
                "--with-ns".to_string(),
                format!("network:{}", netns_path.display()),
            ]);
        } else {
            args.extend([
                "--with-ns".to_string(),
                "network:/proc/1/ns/net".to_string(),
            ]);
        }
        for env in ctr_run_envs(&pending.env, runtime_http_proxy_url().as_deref()) {
            args.extend(["--env".to_string(), env.clone()]);
        }
        for mount in &pending.mounts {
            args.extend(["--mount".to_string(), ctr_mount_arg(mount)]);
        }
        if pending.privileged {
            args.push("--privileged".to_string());
        }
        if let Some(working_dir) = pending.working_dir.as_ref() {
            args.extend(["--cwd".to_string(), working_dir.clone()]);
        }
        args.extend([image.to_string(), containerd_task_name(pending).to_string()]);
        args.extend(pending.command.clone());
        args
    }

    fn build_ctr_task_start_args(pending: &PendingContainer) -> Vec<String> {
        vec![
            "task".to_string(),
            "start".to_string(),
            "--no-pivot".to_string(),
            containerd_task_name(pending).to_string(),
        ]
    }

    #[cfg(test)]
    fn build_ctr_run_args_with_netns(
        pending: &PendingContainer,
        image: &str,
        netns_path: Option<&Path>,
    ) -> Vec<String> {
        let mut args = vec!["run".to_string(), "--no-pivot".to_string()];
        if let Some(wrapper) = container_runc_wrapper_path() {
            args.extend(["--runc-binary".to_string(), wrapper]);
        }
        if let Some(netns_path) = netns_path {
            args.extend([
                "--with-ns".to_string(),
                format!("network:{}", netns_path.display()),
            ]);
        } else {
            args.extend([
                "--with-ns".to_string(),
                "network:/proc/1/ns/net".to_string(),
            ]);
        }
        for env in ctr_run_envs(&pending.env, runtime_http_proxy_url().as_deref()) {
            args.extend(["--env".to_string(), env.clone()]);
        }
        for mount in &pending.mounts {
            args.extend(["--mount".to_string(), ctr_mount_arg(mount)]);
        }
        if pending.privileged {
            args.push("--privileged".to_string());
        }
        if let Some(working_dir) = pending.working_dir.as_ref() {
            args.extend(["--cwd".to_string(), working_dir.clone()]);
        }
        args.extend([image.to_string(), containerd_task_name(pending).to_string()]);
        args.extend(pending.command.clone());
        args
    }

    fn ctr_run_envs(container_env: &[String], runtime_proxy_url: Option<&str>) -> Vec<String> {
        let mut envs = container_env.to_vec();
        let Some(proxy_url) = runtime_proxy_url.and_then(normalize_http_proxy_url) else {
            return envs;
        };

        push_env_if_missing(&mut envs, "HTTP_PROXY", &proxy_url);
        push_env_if_missing(&mut envs, "HTTPS_PROXY", &proxy_url);
        push_env_if_missing(&mut envs, "http_proxy", &proxy_url);
        push_env_if_missing(&mut envs, "https_proxy", &proxy_url);
        push_env_if_missing(&mut envs, "NO_PROXY", DEFAULT_NO_PROXY);
        push_env_if_missing(&mut envs, "no_proxy", DEFAULT_NO_PROXY);
        envs
    }

    fn patch_containerd_snapshot_compat(
        config: &Config,
        pending: &PendingContainer,
    ) -> Result<Vec<PathBuf>, Value> {
        let key = containerd_task_name(pending);
        let target = engine_temp_root()
            .join("snapshot-compat")
            .join(safe_network_file_name(key));
        let _ = run_host_command("umount", &[target.display().to_string()]);
        let _ = fs::remove_dir_all(&target);
        fs::create_dir_all(&target).map_err(|error| {
            json!({
                "backend": "containerd",
                "snapshot": key,
                "error": format!("create snapshot compat mountpoint: {error}"),
            })
        })?;

        let mounts = run_ctr(
            config,
            vec![
                "snapshots".to_string(),
                "mounts".to_string(),
                target.display().to_string(),
                key.to_string(),
            ],
        )?;
        let mount_command = String::from_utf8_lossy(&mounts.stdout)
            .lines()
            .next()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                json!({
                    "backend": "containerd",
                    "snapshot": key,
                    "error": "containerd did not return a snapshot mount command",
                })
            })?;

        let mount_output = run_host_shell(&mount_command).map_err(|error| {
            json!({
                "backend": "containerd",
                "snapshot": key,
                "command": mount_command,
                "error": error.to_string(),
            })
        })?;
        if !mount_output.status.success() {
            let _ = fs::remove_dir_all(&target);
            return Err(json!({
                "backend": "containerd",
                "snapshot": key,
                "command": mount_command,
                "exitCode": mount_output.status.code(),
                "stdout": String::from_utf8_lossy(&mount_output.stdout),
                "stderr": String::from_utf8_lossy(&mount_output.stderr),
            }));
        }

        let patched = patch_legacy_python36_ctypes(&target).map_err(|error| {
            json!({
                "backend": "containerd",
                "snapshot": key,
                "mountpoint": target.display().to_string(),
                "error": format!("patch legacy Python _ctypes: {error}"),
            })
        });
        let unmount_output = run_host_command("umount", &[target.display().to_string()]);
        let _ = fs::remove_dir_all(&target);
        if let Ok(output) = unmount_output {
            if !output.status.success() {
                return Err(json!({
                    "backend": "containerd",
                    "snapshot": key,
                    "error": "unmount snapshot compat mountpoint failed",
                    "stdout": String::from_utf8_lossy(&output.stdout),
                    "stderr": String::from_utf8_lossy(&output.stderr),
                }));
            }
        }
        patched
    }

    fn run_host_shell(command: &str) -> io::Result<Output> {
        Command::new("sh").arg("-lc").arg(command).output()
    }

    fn run_host_command(program: &str, args: &[String]) -> io::Result<Output> {
        Command::new(program).args(args).output()
    }

    fn configure_buildkit_proxy_worker(
        pending: &mut PendingContainer,
        runtime_proxy_url: Option<&str>,
    ) -> Result<(), Value> {
        if !is_buildkit_image_ref(&pending.image) {
            return Ok(());
        }
        if pending
            .command
            .iter()
            .any(|arg| arg == "--oci-worker-binary" || arg.starts_with("--oci-worker-binary="))
        {
            return Ok(());
        }

        if let Some(proxy_url) = runtime_proxy_url.and_then(normalize_http_proxy_url) {
            push_env_if_missing(&mut pending.env, "HTTP_PROXY", &proxy_url);
            push_env_if_missing(&mut pending.env, "HTTPS_PROXY", &proxy_url);
            push_env_if_missing(&mut pending.env, "http_proxy", &proxy_url);
            push_env_if_missing(&mut pending.env, "https_proxy", &proxy_url);
            push_env_if_missing(&mut pending.env, "NO_PROXY", DEFAULT_NO_PROXY);
            push_env_if_missing(&mut pending.env, "no_proxy", DEFAULT_NO_PROXY);
        }
        push_env_if_missing(&mut pending.env, "CRATEBAY_REAL_RUNC", BUILDKIT_RUNC_PATH);

        let wrapper_path = buildkit_runc_wrapper_source()?;
        let wrapper_target = "/usr/local/bin/cratebay-runc-wrapper";
        if !pending
            .mounts
            .iter()
            .any(|mount| mount.target == wrapper_target)
        {
            pending.mounts.push(CtrMount {
                source: wrapper_path.display().to_string(),
                target: wrapper_target.to_string(),
                readonly: true,
            });
        }
        pending
            .command
            .push(format!("--oci-worker-binary={wrapper_target}"));
        Ok(())
    }

    #[cfg(test)]
    fn container_runc_wrapper_path() -> Option<String> {
        if std::env::var("CRATEBAY_DISABLE_RUNC_WRAPPER")
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
        {
            return None;
        }
        Some(
            std::env::var("CRATEBAY_RUNC_WRAPPER_PATH")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_CRATEBAY_RUNC_WRAPPER_PATH.to_string()),
        )
    }

    fn is_buildkit_image_ref(image: &str) -> bool {
        let image = image
            .split_once('@')
            .map(|(name, _)| name)
            .unwrap_or(image)
            .rsplit_once(':')
            .map(|(name, _)| name)
            .unwrap_or(image);
        image == "moby/buildkit"
            || image == "docker.io/moby/buildkit"
            || image == "registry-1.docker.io/moby/buildkit"
    }

    fn buildkit_runc_wrapper_source() -> Result<PathBuf, Value> {
        let path = std::env::var("CRATEBAY_RUNC_WRAPPER_PATH")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CRATEBAY_RUNC_WRAPPER_PATH));
        if path.exists() {
            Ok(path)
        } else if cfg!(test) {
            std::env::current_exe().map_err(|error| {
                json!({
                    "backend": "containerd",
                    "error": format!("resolve test runc wrapper source: {error}"),
                })
            })
        } else {
            Err(json!({
                "backend": "containerd",
                "error": format!("CrateBay runc wrapper binary is missing: {}", path.display()),
            }))
        }
    }

    fn push_env_if_missing(envs: &mut Vec<String>, key: &str, value: &str) {
        if envs.iter().any(|env| env_key(env) == key) {
            return;
        }
        envs.push(format!("{key}={value}"));
    }

    fn env_key(env: &str) -> &str {
        env.split_once('=').map(|(key, _)| key).unwrap_or(env)
    }

    fn runtime_http_proxy_url() -> Option<String> {
        std::env::var("CRATEBAY_HTTP_PROXY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                fs::read_to_string("/proc/cmdline")
                    .ok()
                    .and_then(|cmdline| cmdline_value(&cmdline, "cratebay_http_proxy"))
            })
            .and_then(|value| normalize_http_proxy_url(&value))
    }

    fn normalize_http_proxy_url(value: &str) -> Option<String> {
        let trimmed = value.trim().trim_matches('"').trim_matches('\'').trim();
        if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
            return None;
        }
        let without_trailing_slash = trimmed.trim_end_matches('/');
        if without_trailing_slash.starts_with("http://")
            || without_trailing_slash.starts_with("https://")
        {
            Some(without_trailing_slash.to_string())
        } else {
            Some(format!("http://{without_trailing_slash}"))
        }
    }

    fn cmdline_value(cmdline: &str, key: &str) -> Option<String> {
        let prefix = format!("{key}=");
        cmdline
            .split_whitespace()
            .find_map(|part| part.strip_prefix(&prefix))
            .map(|value| value.trim_matches('"').trim_matches('\'').to_string())
            .filter(|value| !value.trim().is_empty())
    }

    fn prepare_pending_network(
        config: &Config,
        pending: &PendingContainer,
    ) -> Result<Option<PendingNetworkAttachment>, Value> {
        let Some(network) = pending.network.as_ref() else {
            return Ok(None);
        };
        let network_value = managed_network_value_by_id(network).ok_or_else(|| {
            json!({
                "network": network,
                "container": pending.name,
                "error": "network is not managed by CrateBay CNI; create it with CrateBayNativeCreateNetwork or CrateBayNativeCreatePod first",
            })
        })?;
        let attachment = pending_network_attachment(pending, network);
        fs::create_dir_all(netns_root()).map_err(|error| {
            json!({
                "path": netns_root().display().to_string(),
                "error": error.to_string(),
            })
        })?;
        let _ = run_ip_allow_failure(vec![
            "netns".to_string(),
            "delete".to_string(),
            attachment.netns_name.clone(),
        ]);
        run_ip(vec![
            "netns".to_string(),
            "add".to_string(),
            attachment.netns_name.clone(),
        ])?;
        ensure_netns_path_exists(&attachment, "after ip netns add")?;
        if let Err(error) = run_ip(vec![
            "netns".to_string(),
            "exec".to_string(),
            attachment.netns_name.clone(),
            "ip".to_string(),
            "link".to_string(),
            "set".to_string(),
            "lo".to_string(),
            "up".to_string(),
        ])
        .and_then(|_| run_cni_chain(config, "ADD", pending, &attachment, &network_value))
        {
            cleanup_pending_network(config, pending, Some(&attachment));
            return Err(error);
        }
        ensure_netns_path_exists(&attachment, "after CNI ADD")?;
        Ok(Some(attachment))
    }

    fn cleanup_pending_network(
        config: &Config,
        pending: &PendingContainer,
        attachment: Option<&PendingNetworkAttachment>,
    ) {
        let Some(attachment) = attachment else {
            return;
        };
        if let Some(network_value) = managed_network_value_by_id(&attachment.network) {
            let _ = run_cni_chain(config, "DEL", pending, attachment, &network_value);
        }
        let _ = run_ip_allow_failure(vec![
            "netns".to_string(),
            "delete".to_string(),
            attachment.netns_name.clone(),
        ]);
    }

    fn ensure_netns_path_exists(
        attachment: &PendingNetworkAttachment,
        phase: &str,
    ) -> Result<(), Value> {
        if attachment.netns_path.exists() {
            return Ok(());
        }
        Err(json!({
            "network": attachment.network,
            "netns": attachment.netns_name,
            "path": attachment.netns_path.display().to_string(),
            "phase": phase,
            "error": "network namespace path was not created",
        }))
    }

    fn pending_network_attachment(
        pending: &PendingContainer,
        network: &str,
    ) -> PendingNetworkAttachment {
        let raw = format!("{network}-{}", pending.name);
        let prefix = safe_network_file_name(&raw)
            .chars()
            .take(30)
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        let hash = short_hash_hex(&raw);
        let netns_name = if prefix.is_empty() {
            format!("cb-{hash}")
        } else {
            format!("cb-{prefix}-{hash}")
        };
        PendingNetworkAttachment {
            network: network.to_string(),
            netns_path: netns_path(&netns_name),
            netns_name,
        }
    }

    fn short_hash_hex(value: &str) -> String {
        let digest = Sha256::digest(value.as_bytes());
        digest
            .iter()
            .take(6)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    }

    fn generated_container_id(name: &str) -> String {
        container_id_from_seed(&format!("{}:{}", name, now_millis()))
    }

    fn migrated_container_id(raw_id: &str, name: &str) -> String {
        container_id_from_seed(&format!("legacy:{raw_id}:{name}"))
    }

    fn container_id_from_seed(seed: &str) -> String {
        let digest = Sha256::digest(seed.as_bytes());
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    }

    fn looks_like_docker_container_id(id: &str) -> bool {
        id.len() >= 12 && id.chars().all(|ch| ch.is_ascii_hexdigit())
    }

    fn running_network_attachment(
        pending: &PendingContainer,
        network: &str,
    ) -> Option<PendingNetworkAttachment> {
        match (pending.netns_name.as_ref(), pending.netns_path.as_ref()) {
            (Some(netns_name), Some(netns_path)) => Some(PendingNetworkAttachment {
                network: network.to_string(),
                netns_name: netns_name.clone(),
                netns_path: netns_path.clone(),
            }),
            _ if pending.network.is_some() => Some(pending_network_attachment(pending, network)),
            _ => None,
        }
    }

    fn run_cni_chain(
        _config: &Config,
        command: &str,
        pending: &PendingContainer,
        attachment: &PendingNetworkAttachment,
        network_value: &Value,
    ) -> Result<(), Value> {
        let conflist = cni_conflist_for_network(network_value)?;
        let plugins = conflist
            .get("plugins")
            .or_else(|| conflist.get("Plugins"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if plugins.is_empty() {
            return Err(json!({
                "network": attachment.network,
                "error": "CNI conflist has no plugins",
            }));
        }

        if command == "ADD" {
            let mut prev_result = None;
            for plugin in plugins {
                let plugin_type = cni_plugin_type(&plugin)?;
                let input =
                    cni_plugin_input(&conflist, &plugin, prev_result.as_ref(), &pending.ports);
                let output = run_cni_plugin(command, pending, attachment, &plugin_type, &input)?;
                prev_result = parse_json_body(&output.stdout)
                    .as_object()
                    .filter(|object| !object.is_empty())
                    .map(|_| parse_json_body(&output.stdout));
            }
            return Ok(());
        }

        for plugin in plugins.into_iter().rev() {
            let plugin_type = cni_plugin_type(&plugin)?;
            let input = cni_plugin_input(&conflist, &plugin, None, &pending.ports);
            run_cni_plugin(command, pending, attachment, &plugin_type, &input)?;
        }
        Ok(())
    }

    fn cni_conflist_for_network(network_value: &Value) -> Result<Value, Value> {
        let name = string_value(network_value.get("Name"));
        if let Some(path) = nested_string(network_value, &["CrateBay", "cniConfig"]) {
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                    return Ok(value);
                }
            }
        }
        if name.is_empty() {
            return Err(json!({ "error": "managed network is missing a name" }));
        }
        Ok(managed_network_cni_config(&name, network_value))
    }

    fn cni_plugin_type(plugin: &Value) -> Result<String, Value> {
        let plugin_type = string_value(plugin.get("type").or_else(|| plugin.get("Type")));
        if plugin_type.is_empty() || !valid_cni_plugin_name(&plugin_type) {
            return Err(json!({
                "plugin": plugin,
                "error": "invalid CNI plugin type",
            }));
        }
        Ok(plugin_type)
    }

    fn cni_plugin_input(
        conflist: &Value,
        plugin: &Value,
        prev_result: Option<&Value>,
        ports: &[CniPortMapping],
    ) -> Value {
        let mut input = plugin.as_object().cloned().unwrap_or_default();
        input.insert(
            "cniVersion".to_string(),
            conflist
                .get("cniVersion")
                .or_else(|| conflist.get("CNIVersion"))
                .cloned()
                .unwrap_or_else(|| json!("1.0.0")),
        );
        input.insert(
            "name".to_string(),
            conflist
                .get("name")
                .or_else(|| conflist.get("Name"))
                .cloned()
                .unwrap_or_else(|| json!("cratebay")),
        );
        if let Some(prev_result) = prev_result {
            input.insert("prevResult".to_string(), prev_result.clone());
        }
        if !ports.is_empty() {
            let runtime_config = input
                .entry("runtimeConfig".to_string())
                .or_insert_with(|| json!({}));
            if let Some(runtime_config) = runtime_config.as_object_mut() {
                runtime_config.insert("portMappings".to_string(), cni_port_mappings_value(ports));
            }
        }
        Value::Object(input)
    }

    fn cni_port_mappings_value(ports: &[CniPortMapping]) -> Value {
        Value::Array(
            ports
                .iter()
                .map(|port| {
                    let mut value = serde_json::Map::new();
                    value.insert("hostPort".to_string(), json!(port.host_port));
                    value.insert("containerPort".to_string(), json!(port.container_port));
                    value.insert("protocol".to_string(), json!(port.protocol));
                    if let Some(host_ip) = port.host_ip.as_ref() {
                        value.insert("hostIP".to_string(), json!(host_ip));
                    }
                    Value::Object(value)
                })
                .collect(),
        )
    }

    fn run_cni_plugin(
        command: &str,
        pending: &PendingContainer,
        attachment: &PendingNetworkAttachment,
        plugin_type: &str,
        input: &Value,
    ) -> Result<Output, Value> {
        let plugin_path = cni_plugin_path(plugin_type)?;
        let input_bytes = serde_json::to_vec(input).map_err(|error| {
            json!({
                "plugin": plugin_type,
                "error": error.to_string(),
            })
        })?;
        let mut child = Command::new(&plugin_path)
            .env("CNI_COMMAND", command)
            .env("CNI_CONTAINERID", containerd_task_name(pending))
            .env("CNI_NETNS", &attachment.netns_path)
            .env("CNI_IFNAME", "eth0")
            .env("CNI_PATH", cni_path_env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                json!({
                    "plugin": plugin_type,
                    "path": plugin_path.display().to_string(),
                    "command": command,
                    "error": error.to_string(),
                })
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&input_bytes).map_err(|error| {
                json!({
                    "plugin": plugin_type,
                    "path": plugin_path.display().to_string(),
                    "command": command,
                    "error": error.to_string(),
                })
            })?;
        }
        let output = wait_child_output(
            child,
            CNI_PLUGIN_TIMEOUT,
            json!({
                "plugin": plugin_type,
                "path": plugin_path.display().to_string(),
                "command": command,
            }),
        )?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(json!({
                "plugin": plugin_type,
                "path": plugin_path.display().to_string(),
                "command": command,
                "exitCode": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
                "input": input,
            }))
        }
    }

    fn run_ip(args: Vec<String>) -> Result<Output, Value> {
        let output = run_ip_allow_failure(args.clone())?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(json!({
                "ip": ip_binary(),
                "args": args,
                "exitCode": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
            }))
        }
    }

    fn run_ip_allow_failure(args: Vec<String>) -> Result<Output, Value> {
        let ip = ip_binary();
        let child = Command::new(&ip)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                json!({
                    "ip": ip,
                    "args": args,
                    "error": error.to_string(),
                })
            })?;
        wait_child_output(
            child,
            IP_COMMAND_TIMEOUT,
            json!({
                "ip": ip,
                "args": args,
            }),
        )
    }

    fn wait_child_output(
        mut child: std::process::Child,
        timeout: Duration,
        context: Value,
    ) -> Result<Output, Value> {
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(context_with_error(
                        context,
                        format!("command timed out after {}s", timeout.as_secs()),
                    ));
                }
                Err(error) => return Err(context_with_error(context, error.to_string())),
            }
        };

        let mut stdout = Vec::new();
        if let Some(mut pipe) = child.stdout.take() {
            let _ = pipe.read_to_end(&mut stdout);
        }

        let mut stderr = Vec::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_end(&mut stderr);
        }

        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }

    fn context_with_error(mut context: Value, error: String) -> Value {
        if let Some(object) = context.as_object_mut() {
            object.insert("error".to_string(), json!(error));
            context
        } else {
            json!({
                "error": error,
                "context": context,
            })
        }
    }

    fn ip_binary() -> String {
        if let Some(configured) = std::env::var("CRATEBAY_IP")
            .ok()
            .filter(|value| !value.trim().is_empty())
        {
            return configured;
        }

        ["/sbin/ip", "/usr/sbin/ip", "/usr/bin/ip", "/bin/ip"]
            .into_iter()
            .find(|candidate| Path::new(candidate).exists())
            .unwrap_or("ip")
            .to_string()
    }

    fn cni_plugin_path(plugin_type: &str) -> Result<PathBuf, Value> {
        for path in cni_path_entries() {
            let candidate = path.join(plugin_type);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        Err(json!({
            "plugin": plugin_type,
            "cniPath": cni_path_env(),
            "error": "CNI plugin was not found",
        }))
    }

    fn cni_path_entries() -> Vec<PathBuf> {
        std::env::var("CRATEBAY_CNI_PATH")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| std::env::split_paths(&value).collect())
            .unwrap_or_else(|| std::env::split_paths(&cni_default_path()).collect::<Vec<PathBuf>>())
    }

    fn cni_path_env() -> String {
        std::env::var("CRATEBAY_CNI_PATH")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(cni_default_path)
    }

    fn cni_default_path() -> String {
        "/opt/cni/bin:/usr/libexec/cni:/usr/lib/cni:/usr/local/lib/cni".to_string()
    }

    fn valid_cni_plugin_name(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    }

    fn netns_root() -> PathBuf {
        PathBuf::from("/var/run/netns")
    }

    fn netns_path(name: &str) -> PathBuf {
        netns_root().join(name)
    }

    fn ctr_image_for_run(image: &str) -> String {
        ctr_image_candidates(image)
            .into_iter()
            .last()
            .unwrap_or_else(|| image.to_string())
    }

    fn ctr_image_candidates(image: &str) -> Vec<String> {
        let trimmed = image.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let mut candidates = vec![trimmed.to_string()];
        let name_part = trimmed
            .split_once('@')
            .map(|(name, _)| name)
            .unwrap_or(trimmed);
        if !image_name_has_tag(name_part) {
            let tagged = if let Some((name, digest)) = trimmed.split_once('@') {
                format!("{name}:latest@{digest}")
            } else {
                format!("{trimmed}:latest")
            };
            candidates.push(tagged);
        }
        let has_slash = name_part.contains('/');
        let first = name_part.split('/').next().unwrap_or_default();
        let has_registry =
            has_slash && (first.contains('.') || first.contains(':') || first == "localhost");
        if !has_registry {
            let normalized = if name_part.contains('/') {
                format!("docker.io/{trimmed}")
            } else {
                format!("docker.io/library/{trimmed}")
            };
            if !candidates.iter().any(|candidate| candidate == &normalized) {
                candidates.push(normalized);
            }
            if !image_name_has_tag(name_part) {
                let normalized_latest = if name_part.contains('/') {
                    format!("docker.io/{name_part}:latest")
                } else {
                    format!("docker.io/library/{name_part}:latest")
                };
                if !candidates
                    .iter()
                    .any(|candidate| candidate == &normalized_latest)
                {
                    candidates.push(normalized_latest);
                }
            }
        }
        candidates
    }

    fn image_name_has_tag(name_part: &str) -> bool {
        name_part
            .rsplit('/')
            .next()
            .map(|tail| tail.contains(':'))
            .unwrap_or(false)
    }

    fn ctr_image_pull_candidates(image: &str) -> Vec<String> {
        let mut candidates = ctr_image_candidates(image);
        candidates.reverse();
        candidates
    }

    fn ctr_image_pull_args(image_ref: &str) -> Result<Vec<String>, Value> {
        let mut args = vec!["images".to_string(), "pull".to_string()];
        if let Some(registry_host) = loopback_registry_host(image_ref) {
            let hosts_dir = write_plain_http_hosts_dir(&registry_host)?;
            args.push("--hosts-dir".to_string());
            args.push(hosts_dir.display().to_string());
        }
        args.push(image_ref.to_string());
        Ok(args)
    }

    fn loopback_registry_host(image_ref: &str) -> Option<String> {
        let name_part = image_ref
            .trim()
            .split_once('@')
            .map(|(name, _)| name)
            .unwrap_or_else(|| image_ref.trim());
        let first = name_part.split('/').next()?;
        let host = first
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']').map(|(host, _)| host))
            .or_else(|| first.split_once(':').map(|(host, _)| host))
            .unwrap_or(first);
        let is_loopback = host == "localhost"
            || host
                .parse::<IpAddr>()
                .map(|addr| addr.is_loopback())
                .unwrap_or(false);
        is_loopback.then(|| first.to_string())
    }

    fn write_plain_http_hosts_dir(registry_host: &str) -> Result<PathBuf, Value> {
        let content = format!(
            "server = \"http://{registry_host}\"\n\n[host.\"http://{registry_host}\"]\n  capabilities = [\"pull\", \"resolve\"]\n"
        );
        let hosts_root = engine_temp_root().join("containerd-hosts");
        for namespace in containerd_hosts_namespaces(registry_host) {
            let root = hosts_root.join(namespace);
            fs::create_dir_all(&root).map_err(|error| {
                json!({
                    "error": error.to_string(),
                    "path": root.display().to_string(),
                    "registry": registry_host,
                })
            })?;
            let hosts_file = root.join("hosts.toml");
            fs::write(&hosts_file, &content).map_err(|error| {
                json!({
                    "error": error.to_string(),
                    "path": hosts_file.display().to_string(),
                    "registry": registry_host,
                })
            })?;
        }
        Ok(hosts_root)
    }

    fn containerd_hosts_namespaces(registry_host: &str) -> Vec<String> {
        let escaped_host = registry_host.replace(['/', '\\'], "_");
        let mut namespaces = Vec::new();
        if let Some((host, port)) = registry_host.rsplit_once(':') {
            if !host.is_empty() && !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit()) {
                namespaces.push(format!("{host}_{port}_").replace(['/', '\\'], "_"));
            }
        }
        namespaces.push(escaped_host);
        namespaces.sort();
        namespaces.dedup();
        namespaces
    }

    fn rewrite_image_for_registry_mirror(image: &str, mirror: &str) -> String {
        let image = image.trim();
        let mirror = normalize_registry_mirror(mirror);
        if image.is_empty() || mirror.is_empty() {
            return image.to_string();
        }

        let name_part = image.split_once('@').map(|(name, _)| name).unwrap_or(image);
        if let Some(first) = name_part.split('/').next() {
            if name_part.contains('/')
                && (first.contains('.') || first.contains(':') || first == "localhost")
            {
                return image.to_string();
            }
        }

        if name_part.contains('/') {
            format!("{mirror}/{image}")
        } else {
            format!("{mirror}/library/{image}")
        }
    }

    fn pending_mounts(host_config: Option<&Value>) -> Vec<CtrMount> {
        let binds = host_config.and_then(|value| value.get("Binds"));
        string_array(binds)
            .into_iter()
            .filter_map(|spec| parse_bind_mount_spec(&spec))
            .collect()
    }

    fn parse_bind_mount_spec(spec: &str) -> Option<CtrMount> {
        let parts = spec.split(':').collect::<Vec<_>>();
        let (source, target, option_parts) = match parts.as_slice() {
            [source, target] => (*source, *target, &[][..]),
            [source, target, options @ ..] => (*source, *target, options),
            _ => return None,
        };
        let source = source.trim();
        let target = target.trim();
        if source.is_empty() || target.is_empty() {
            return None;
        }

        let readonly = option_parts.contains(&"ro");
        let source = if source.starts_with('/') {
            source.to_string()
        } else {
            volume_data_path(source).display().to_string()
        };
        Some(CtrMount {
            source,
            target: target.to_string(),
            readonly,
        })
    }

    fn ctr_mount_arg(mount: &CtrMount) -> String {
        let access = if mount.readonly { "ro" } else { "rw" };
        format!(
            "type=bind,src={},dst={},options=rbind:{}",
            mount.source, mount.target, access
        )
    }

    fn ensure_pending_mount_sources(pending: &PendingContainer) -> Result<(), Value> {
        for mount in &pending.mounts {
            if mount
                .source
                .starts_with(volume_root().to_string_lossy().as_ref())
            {
                fs::create_dir_all(&mount.source).map_err(|error| {
                    json!({
                        "error": error.to_string(),
                        "source": mount.source,
                    })
                })?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn ensure_container_system_mounts(pending: &PendingContainer) -> Result<Vec<CtrMount>, Value> {
        ensure_container_system_mounts_with_peers(pending, pending_container_records())
    }

    fn ensure_container_system_mounts_for_state(
        state: &AdapterState,
        pending: &PendingContainer,
    ) -> Result<Vec<CtrMount>, Value> {
        ensure_container_system_mounts_with_peers(pending, unique_pending_containers(state))
    }

    fn ensure_container_system_mounts_with_peers(
        pending: &PendingContainer,
        peers: Vec<PendingContainer>,
    ) -> Result<Vec<CtrMount>, Value> {
        let root = container_system_container_root(&pending.name);
        fs::create_dir_all(&root).map_err(|error| {
            json!({
                "container": pending.name,
                "path": root.display().to_string(),
                "error": error.to_string(),
            })
        })?;

        let resolv_path = root.join("resolv.conf");
        let hosts_path = root.join("hosts");
        fs::write(&resolv_path, container_resolv_conf()).map_err(|error| {
            json!({
                "container": pending.name,
                "path": resolv_path.display().to_string(),
                "error": error.to_string(),
            })
        })?;
        fs::write(&hosts_path, container_hosts(pending, &peers)).map_err(|error| {
            json!({
                "container": pending.name,
                "path": hosts_path.display().to_string(),
                "error": error.to_string(),
            })
        })?;

        let mut mounts = Vec::new();
        if !pending
            .mounts
            .iter()
            .any(|mount| mount.target == "/etc/resolv.conf")
        {
            mounts.push(CtrMount {
                source: resolv_path.display().to_string(),
                target: "/etc/resolv.conf".to_string(),
                readonly: true,
            });
        }
        if !pending
            .mounts
            .iter()
            .any(|mount| mount.target == "/etc/hosts")
        {
            mounts.push(CtrMount {
                source: hosts_path.display().to_string(),
                target: "/etc/hosts".to_string(),
                readonly: true,
            });
        }
        if let Some(cgroup_mount) =
            container_cgroup_mount(pending, Path::new(CGROUP_MOUNT).exists())
        {
            mounts.push(cgroup_mount);
        }
        Ok(mounts)
    }

    fn container_cgroup_mount(pending: &PendingContainer, source_exists: bool) -> Option<CtrMount> {
        if !source_exists
            || pending
                .mounts
                .iter()
                .any(|mount| mount.target == CGROUP_MOUNT)
        {
            return None;
        }
        Some(CtrMount {
            source: CGROUP_MOUNT.to_string(),
            target: CGROUP_MOUNT.to_string(),
            readonly: !pending.privileged,
        })
    }

    fn container_resolv_conf() -> String {
        fs::read_to_string("/etc/resolv.conf")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "nameserver 1.1.1.1\nnameserver 8.8.8.8\n".to_string())
    }

    fn container_hosts(pending: &PendingContainer, peers: &[PendingContainer]) -> String {
        let mut hosts = format!(
            "127.0.0.1 localhost\n::1 localhost\n127.0.1.1 {}\n",
            pending.name
        );
        let Some(network) = pending.network.as_deref() else {
            return hosts;
        };

        for peer in peers {
            if peer.name == pending.name
                || peer.network.as_deref() != Some(network)
                || !peer.started_with_ctr
                || peer.exit_code.is_some()
            {
                continue;
            }
            let Some((ip, _prefix)) = pending_ipv4_address(peer) else {
                continue;
            };
            let aliases = container_host_aliases(peer);
            if !aliases.is_empty() {
                hosts.push_str(&format!("{} {}\n", ip, aliases.join(" ")));
            }
        }
        hosts
    }

    fn container_host_aliases(pending: &PendingContainer) -> Vec<String> {
        let mut aliases = Vec::new();
        push_host_alias(&mut aliases, &pending.name);
        for alias in &pending.aliases {
            push_host_alias(&mut aliases, alias);
        }
        aliases
    }

    fn pending_ipv4_address(pending: &PendingContainer) -> Option<(String, u8)> {
        let netns_name = pending.netns_name.as_ref()?;
        let output = run_ip_allow_failure(vec![
            "netns".to_string(),
            "exec".to_string(),
            netns_name.clone(),
            "ip".to_string(),
            "-4".to_string(),
            "-o".to_string(),
            "addr".to_string(),
            "show".to_string(),
            "dev".to_string(),
            "eth0".to_string(),
        ])
        .ok()?;
        if !output.status.success() {
            return None;
        }
        parse_ip_addr_show(&String::from_utf8_lossy(&output.stdout))
    }

    fn parse_ip_addr_show(output: &str) -> Option<(String, u8)> {
        for line in output.lines() {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            for (idx, part) in parts.iter().enumerate() {
                if *part != "inet" {
                    continue;
                }
                let cidr = parts.get(idx + 1)?;
                let (ip, prefix) = cidr.split_once('/').unwrap_or((cidr, "32"));
                let prefix = prefix.parse::<u8>().ok().unwrap_or(32);
                if !ip.trim().is_empty() {
                    return Some((ip.to_string(), prefix));
                }
            }
        }
        None
    }

    fn refresh_network_hosts_for_network(state: &AdapterState, network: &str) {
        let peers = unique_pending_containers(state);
        for pending in &peers {
            if pending.network.as_deref() != Some(network) {
                continue;
            }
            let hosts_path = container_system_container_root(&pending.name).join("hosts");
            if hosts_path.exists() {
                let _ = fs::write(hosts_path, container_hosts(pending, &peers));
            }
        }
    }

    fn container_system_root() -> PathBuf {
        if let Some(root) = std::env::var("CRATEBAY_CONTAINER_SYSTEM_ROOT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
        {
            return root;
        }
        #[cfg(test)]
        {
            std::env::temp_dir()
                .join("cratebay-engine-adapter-tests")
                .join(test_thread_id())
                .join("system")
        }
        #[cfg(not(test))]
        {
            PathBuf::from("/var/lib/cratebay-engine/container-system")
        }
    }

    fn container_system_container_root(name: &str) -> PathBuf {
        container_system_root().join(safe_network_file_name(name))
    }

    fn cleanup_containerd_pending_artifacts(config: &Config, pending: &PendingContainer) {
        let mut names = Vec::new();
        for name in [
            containerd_task_name(pending),
            pending.name.as_str(),
            pending.id.as_str(),
        ] {
            if !name.is_empty() && !names.iter().any(|existing| existing == name) {
                names.push(name.to_string());
            }
        }
        for name in names {
            cleanup_containerd_name_artifacts(config, &name);
        }
    }

    fn cleanup_containerd_name_artifacts(config: &Config, name: &str) {
        let _ = fs::remove_dir_all(pending_archive_container_root(name));
        let _ = fs::remove_dir_all(container_system_container_root(name));
        for args in containerd_cleanup_args(name) {
            let _ = run_ctr_allow_failure(config, args);
        }
    }

    fn containerd_cleanup_args(name: &str) -> Vec<Vec<String>> {
        vec![
            vec![
                "tasks".to_string(),
                "kill".to_string(),
                "--signal".to_string(),
                "KILL".to_string(),
                name.to_string(),
            ],
            vec![
                "tasks".to_string(),
                "rm".to_string(),
                "--force".to_string(),
                name.to_string(),
            ],
            vec!["containers".to_string(), "rm".to_string(), name.to_string()],
            vec!["snapshots".to_string(), "rm".to_string(), name.to_string()],
        ]
    }

    fn archive_target_path(query: &HashMap<String, String>) -> Option<String> {
        let mut path = query
            .get("path")
            .or_else(|| query.get("Path"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())?;
        if !path.starts_with('/') {
            path = format!("/{path}");
        }
        Some(path)
    }

    fn pending_archive_root() -> PathBuf {
        if let Some(root) = std::env::var("CRATEBAY_CONTAINER_ARCHIVE_ROOT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
        {
            return root;
        }
        #[cfg(test)]
        {
            std::env::temp_dir()
                .join("cratebay-engine-adapter-tests")
                .join(test_thread_id())
                .join("archives")
        }
        #[cfg(not(test))]
        {
            PathBuf::from("/var/lib/cratebay-engine/container-archives")
        }
    }

    fn pending_archive_container_root(name: &str) -> PathBuf {
        pending_archive_root().join(safe_network_file_name(name))
    }

    fn pending_archive_stage_path(name: &str, target: &str) -> PathBuf {
        let target = target
            .trim_start_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .map(safe_network_file_name)
            .collect::<Vec<_>>()
            .join("__");
        pending_archive_container_root(name).join(if target.is_empty() {
            "root".to_string()
        } else {
            target
        })
    }

    fn pending_archive_mounts(stage_dir: &Path, target: &str) -> Vec<CtrMount> {
        if !archive_target_should_mount_entries(target) {
            return vec![CtrMount {
                source: stage_dir.display().to_string(),
                target: target.to_string(),
                readonly: false,
            }];
        }

        let mut mounts = Vec::new();
        if let Ok(entries) = fs::read_dir(stage_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.is_empty() || name == "." || name == ".." {
                    continue;
                }
                mounts.push(CtrMount {
                    source: entry.path().display().to_string(),
                    target: join_container_path(target, &name),
                    readonly: false,
                });
            }
        }
        mounts
    }

    fn archive_target_should_mount_entries(target: &str) -> bool {
        matches!(target, "/" | "/etc" | "/run" | "/var" | "/usr" | "/tmp")
    }

    fn join_container_path(base: &str, child: &str) -> String {
        let base = base.trim_end_matches('/');
        if base.is_empty() {
            format!("/{child}")
        } else {
            format!("{base}/{child}")
        }
    }

    fn pending_container(state: &AdapterState, id: &str) -> Option<PendingContainer> {
        let id = normalize_container_ref(id);
        if let Some(pending) = state.pending_containers.lock().ok().and_then(|containers| {
            containers.get(&id).cloned().or_else(|| {
                containers
                    .values()
                    .find(|pending| pending_container_ref_matches(pending, &id))
                    .cloned()
            })
        }) {
            return Some(pending);
        }
        let pending = read_pending_container_record(&id)?;
        cache_pending_container(state, &pending.id, &pending.name, pending.clone());
        Some(pending)
    }

    fn refresh_pending_task_state(
        state: &AdapterState,
        mut pending: PendingContainer,
    ) -> PendingContainer {
        if pending.started_with_ctr && pending.exit_code.is_none() {
            if let Ok(false) = containerd_task_exists(&state.config, &pending) {
                let exit_code = wait_for_pending_exit_code_with_timeout(
                    state,
                    &pending.id,
                    &pending.name,
                    Duration::from_millis(500),
                )
                .unwrap_or(LOST_TASK_EXIT_CODE);
                if exit_code == LOST_TASK_EXIT_CODE {
                    append_log_bytes(
                        &pending.log_path,
                        b"CrateBay task is missing from containerd; marking container exited so it can be restarted.\n",
                    )
                    .ok();
                }
                pending.exit_code = Some(exit_code);
                cache_pending_container(state, &pending.id, &pending.name, pending.clone());
                let _ = write_pending_container_record(&pending);
            }
        }
        pending
    }

    fn reset_pending_runtime_state_for_start(
        state: &AdapterState,
        mut pending: PendingContainer,
    ) -> PendingContainer {
        if pending.started_with_ctr {
            cleanup_stored_pending_network(&state.config, &pending);
            cleanup_containerd_pending_artifacts(&state.config, &pending);
            pending.started_with_ctr = false;
            pending.exit_code = None;
            pending.netns_name = None;
            pending.netns_path = None;
            cache_pending_container(state, &pending.id, &pending.name, pending.clone());
            let _ = write_pending_container_record(&pending);
        }
        pending
    }

    fn cleanup_stored_pending_network(config: &Config, pending: &PendingContainer) {
        let Some(network) = pending.network.as_deref() else {
            return;
        };
        if let Some(attachment) = running_network_attachment(pending, network) {
            cleanup_pending_network(config, pending, Some(&attachment));
        }
    }

    fn containerd_task_exists(config: &Config, pending: &PendingContainer) -> Result<bool, Value> {
        let started_at = Instant::now();
        loop {
            match containerd_task_exists_once(config, pending) {
                Ok(true) => return Ok(true),
                Ok(false) if started_at.elapsed() < TASK_DISCOVERY_GRACE => {
                    thread::sleep(Duration::from_millis(100));
                }
                Ok(false) => return Ok(false),
                Err(error) => return Err(error),
            }
        }
    }

    fn containerd_task_exists_once(
        config: &Config,
        pending: &PendingContainer,
    ) -> Result<bool, Value> {
        if containerd_task_runtime_dirs(config, pending)
            .iter()
            .any(|path| path.is_dir())
        {
            return Ok(true);
        }

        let output = run_ctr(
            config,
            vec!["tasks".to_string(), "list".to_string(), "-q".to_string()],
        )?;
        if task_list_contains(
            &output.stdout,
            &[
                containerd_task_name(pending),
                pending.name.as_str(),
                pending.id.as_str(),
            ],
        ) {
            return Ok(true);
        }

        let output = run_ctr(config, vec!["tasks".to_string(), "list".to_string()])?;
        Ok(task_list_contains(
            &output.stdout,
            &[
                containerd_task_name(pending),
                pending.name.as_str(),
                pending.id.as_str(),
            ],
        ))
    }

    fn task_list_contains(bytes: &[u8], names: &[&str]) -> bool {
        String::from_utf8_lossy(bytes)
            .lines()
            .map(str::trim)
            .any(|line| {
                names
                    .iter()
                    .any(|name| line == *name || line.split_whitespace().any(|part| part == *name))
            })
    }

    fn cache_pending_container(
        state: &AdapterState,
        id: &str,
        name: &str,
        pending: PendingContainer,
    ) {
        if let Ok(mut containers) = state.pending_containers.lock() {
            let exact_id = normalize_container_ref(id);
            let exact_name = normalize_container_ref(name);
            containers.insert(exact_id.clone(), pending.clone());
            containers.insert(exact_name.clone(), pending.clone());

            let mut alias_keys = pending_container_lookup_keys(&pending);
            alias_keys.sort();
            alias_keys.dedup();
            for key in alias_keys {
                if key == exact_id || key == exact_name {
                    continue;
                }
                let occupied_by_other = containers
                    .get(&key)
                    .map(|existing| existing.id != pending.id && existing.name != pending.name)
                    .unwrap_or(false);
                if !occupied_by_other {
                    containers.insert(key, pending.clone());
                }
            }
        }
    }

    fn mark_pending_started_with_ctr(state: &AdapterState, id: &str, name: &str) {
        update_pending_container(state, id, name, |pending| {
            pending.started_with_ctr = true;
            pending.exit_code = None;
        });
    }

    #[cfg(test)]
    fn dedupe_docker_items_by_name(items: &mut Vec<Value>) {
        let mut seen = HashSet::new();
        items.retain(|item| {
            let key = item
                .get("Name")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .or_else(|| item.get("Id").and_then(Value::as_str))
                .unwrap_or_default()
                .to_string();
            key.is_empty() || seen.insert(key)
        });
    }

    fn mark_pending_exit_code(state: &AdapterState, id: &str, name: &str, exit_code: i64) {
        update_pending_container(state, id, name, |pending| {
            pending.exit_code = Some(exit_code);
        });
    }

    fn update_pending_network(
        state: &AdapterState,
        id: &str,
        name: &str,
        network: Option<String>,
        aliases: Vec<String>,
    ) {
        update_pending_container(state, id, name, |pending| {
            pending.network = network.clone();
            pending.aliases.clear();
            let pending_name = pending.name.clone();
            push_host_alias(&mut pending.aliases, &pending_name);
            for alias in &aliases {
                push_host_alias(&mut pending.aliases, alias);
            }
        });
    }

    fn update_pending_network_attachment(
        state: &AdapterState,
        id: &str,
        name: &str,
        network: Option<String>,
        attachment: Option<&PendingNetworkAttachment>,
    ) {
        update_pending_container(state, id, name, |pending| {
            pending.network = network.clone();
            if let Some(attachment) = attachment {
                pending.netns_name = Some(attachment.netns_name.clone());
                pending.netns_path = Some(attachment.netns_path.clone());
            }
        });
    }

    fn update_pending_container<F>(state: &AdapterState, id: &str, name: &str, mut update: F)
    where
        F: FnMut(&mut PendingContainer),
    {
        if let Some(mut pending) =
            pending_container(state, id).or_else(|| pending_container(state, name))
        {
            update(&mut pending);
            cache_pending_container(state, &pending.id, &pending.name, pending.clone());
            let _ = write_pending_container_record(&pending);
        }
    }

    fn unique_pending_containers(state: &AdapterState) -> Vec<PendingContainer> {
        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        let cached = state
            .pending_containers
            .lock()
            .map(|containers| containers.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for pending in cached {
            let pending = refresh_pending_task_state(state, pending);
            if seen.insert(pending.id.clone()) {
                unique.push(pending);
            }
        }
        for pending in pending_container_records() {
            let pending = refresh_pending_task_state(state, pending);
            if seen.insert(pending.id.clone()) {
                unique.push(pending);
            }
        }
        unique
    }

    fn pending_state(pending: &PendingContainer) -> (&'static str, String) {
        if pending.started_with_ctr && pending.exit_code.is_none() {
            ("running", "Up".to_string())
        } else if let Some(code) = pending.exit_code {
            ("exited", format!("Exited ({code})"))
        } else {
            ("created", "Created".to_string())
        }
    }

    fn shim_task_summary(pending: &PendingContainer) -> Value {
        let (state, status) = pending_state(pending);
        json!({
            "api": "cratebay.shim.task.v1",
            "id": pending.id,
            "name": pending.name,
            "image": pending.image,
            "state": state,
            "status": status,
            "backend": "containerd",
            "manager": "cratebay-containerd-shim",
            "containerdTask": containerd_task_name(pending),
            "startedWithCtr": pending.started_with_ctr,
            "exitCode": pending.exit_code,
            "network": pending.network,
            "aliases": pending.aliases,
            "labels": pending.labels,
            "netnsName": pending.netns_name,
            "netnsPath": pending.netns_path.as_ref().map(|path| path.display().to_string()),
            "ports": pending.ports.iter().map(|port| json!({
                "hostIP": port.host_ip,
                "hostPort": port.host_port,
                "containerPort": port.container_port,
                "protocol": port.protocol,
            })).collect::<Vec<_>>(),
            "logPath": pending.log_path.display().to_string(),
            "managedBy": "cratebay",
        })
    }

    fn pending_summary_value(pending: &PendingContainer) -> Value {
        let (state, status) = pending_state(pending);
        let image_id = pending_image_id(pending);
        json!({
            "Id": pending.id,
            "Names": [format!("/{}", pending.name)],
            "Image": pending.image,
            "ImageID": image_id,
            "Command": pending.command.join(" "),
            "Created": pending.created_at,
            "Ports": [],
            "Mounts": pending_mounts_value(pending),
            "Labels": pending_container_labels(pending),
            "NetworkSettings": {
                "Networks": pending_networks_value(pending),
            },
            "State": state,
            "Status": status,
        })
    }

    fn pending_container_labels(pending: &PendingContainer) -> Value {
        let mut labels = pending.labels.clone();
        strip_completed_compose_replace_label(&pending.name, &pending.runtime_id, &mut labels);
        labels.insert("com.cratebay.managed".to_string(), json!("true"));
        labels.insert("com.cratebay.backend".to_string(), json!("containerd"));
        Value::Object(labels)
    }

    fn strip_completed_compose_replace_label(
        name: &str,
        runtime_id: &str,
        labels: &mut serde_json::Map<String, Value>,
    ) {
        if !runtime_id.is_empty() && name != runtime_id {
            labels.remove("com.docker.compose.replace");
        }
    }

    fn pending_inspect_value(pending: &PendingContainer) -> Value {
        let (state, status) = pending_state(pending);
        let image_id = pending_image_id(pending);
        json!({
            "Id": pending.id,
            "Name": format!("/{}", pending.name),
            "Image": image_id,
            "Created": chrono_like_timestamp(pending.created_at),
            "State": {
                "Status": state,
                "Running": state == "running",
                "ExitCode": pending.exit_code.unwrap_or(0),
            },
            "Config": {
                "Image": pending.image,
                "Cmd": pending.command,
                "Env": pending.env,
                "WorkingDir": pending.working_dir.clone().unwrap_or_default(),
                "Labels": pending_container_labels(pending),
            },
            "HostConfig": {
                "NetworkMode": pending.network.clone().unwrap_or_else(|| "default".to_string()),
                "Privileged": pending.privileged,
            },
            "NetworkSettings": {
                "Networks": pending_networks_value(pending),
            },
            "Mounts": pending_mounts_value(pending),
            "Status": status,
        })
    }

    fn pending_image_id(pending: &PendingContainer) -> String {
        pending
            .labels
            .get("com.docker.compose.image")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| pending.image.clone())
    }

    fn pending_mounts_value(pending: &PendingContainer) -> Value {
        json!(pending
            .mounts
            .iter()
            .map(|mount| {
                if let Some(name) = volume_name_from_data_path(&mount.source) {
                    json!({
                        "Type": "volume",
                        "Name": name,
                        "Source": mount.source,
                        "Destination": mount.target,
                        "Driver": "local",
                        "Mode": "",
                        "RW": !mount.readonly,
                        "Propagation": "",
                    })
                } else {
                    json!({
                        "Type": "bind",
                        "Source": mount.source,
                        "Destination": mount.target,
                        "Mode": "",
                        "RW": !mount.readonly,
                        "Propagation": "",
                    })
                }
            })
            .collect::<Vec<_>>())
    }

    fn pending_networks_value(pending: &PendingContainer) -> Value {
        pending
            .network
            .as_ref()
            .map(|network| {
                let (ip, prefix) =
                    pending_ipv4_address(pending).unwrap_or_else(|| (String::new(), 0));
                let ipv4_address = if ip.is_empty() {
                    String::new()
                } else {
                    format!("{ip}/{prefix}")
                };
                json!({
                    network: {
                        "NetworkID": network,
                        "EndpointID": pending.id,
                        "Aliases": container_host_aliases(pending),
                        "IPAddress": ip,
                        "IPPrefixLen": prefix,
                        "IPv4Address": ipv4_address,
                    }
                })
            })
            .unwrap_or_else(|| json!({}))
    }

    fn dedupe_native_items_by_name(items: &mut Vec<Value>) {
        let mut seen = HashSet::new();
        items.retain(|item| {
            let key = item
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .or_else(|| item.get("id").and_then(Value::as_str))
                .unwrap_or_default()
                .to_string();
            key.is_empty() || seen.insert(key)
        });
    }

    fn remove_pending_container(state: &AdapterState, id: &str, name: &str) {
        let id = normalize_container_ref(id);
        let name = normalize_container_ref(name);
        if let Ok(mut containers) = state.pending_containers.lock() {
            containers.retain(|key, pending| {
                key != &id
                    && key != &name
                    && !pending_container_identity_matches(pending, &id)
                    && !pending_container_identity_matches(pending, &name)
            });
        }
        let _ = remove_pending_container_record(&id);
        if id != name {
            let _ = remove_pending_container_record(&name);
        }
    }

    fn normalize_container_ref(id: &str) -> String {
        id.trim().trim_start_matches('/').to_string()
    }

    fn pending_container_lookup_keys(pending: &PendingContainer) -> Vec<String> {
        let mut keys = Vec::new();
        push_pending_lookup_key(&mut keys, &pending.id);
        push_pending_lookup_key(&mut keys, &pending.name);
        for alias in &pending.aliases {
            push_pending_lookup_key(&mut keys, alias);
        }
        keys
    }

    fn push_pending_lookup_key(keys: &mut Vec<String>, key: &str) {
        let key = normalize_container_ref(key);
        if !key.is_empty() && !keys.iter().any(|existing| existing == &key) {
            keys.push(key);
        }
    }

    fn pending_container_ref_matches(pending: &PendingContainer, id: &str) -> bool {
        let id = normalize_container_ref(id);
        if id.is_empty() {
            return false;
        }
        pending_container_lookup_keys(pending)
            .iter()
            .any(|key| key == &id || docker_container_id_prefix_matches(key, &id))
    }

    fn pending_container_identity_matches(pending: &PendingContainer, id: &str) -> bool {
        let id = normalize_container_ref(id);
        !id.is_empty()
            && (pending.id == id
                || pending.name == id
                || docker_container_id_prefix_matches(&pending.id, &id))
    }

    fn docker_container_id_prefix_matches(full_id: &str, prefix: &str) -> bool {
        prefix.len() >= 4
            && prefix.len() < full_id.len()
            && looks_like_docker_container_id(full_id)
            && prefix.chars().all(|ch| ch.is_ascii_hexdigit())
            && full_id.starts_with(prefix)
    }

    fn pending_container_registry_root() -> PathBuf {
        if let Some(root) = std::env::var("CRATEBAY_CONTAINER_REGISTRY_ROOT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
        {
            return root;
        }
        #[cfg(test)]
        {
            std::env::temp_dir()
                .join("cratebay-engine-adapter-tests")
                .join(test_thread_id())
                .join("containers")
        }
        #[cfg(not(test))]
        {
            PathBuf::from("/var/lib/cratebay-engine/containers")
        }
    }

    fn pending_container_registry_path(name: &str) -> PathBuf {
        pending_container_registry_root().join(format!("{name}.json"))
    }

    fn write_pending_container_record(pending: &PendingContainer) -> Result<(), Value> {
        if !valid_local_name(&pending.name) {
            return Err(json!({
                "name": pending.name,
                "error": "invalid container name",
            }));
        }
        fs::create_dir_all(pending_container_registry_root()).map_err(|error| {
            json!({
                "path": pending_container_registry_root().display().to_string(),
                "error": error.to_string(),
            })
        })?;
        fs::write(
            pending_container_registry_path(&pending.name),
            serde_json::to_vec_pretty(&pending_container_record_value(pending))
                .unwrap_or_else(|_| b"{}".to_vec()),
        )
        .map_err(|error| {
            json!({
                "path": pending_container_registry_path(&pending.name).display().to_string(),
                "error": error.to_string(),
            })
        })
    }

    fn read_pending_container_record(id: &str) -> Option<PendingContainer> {
        let id = normalize_container_ref(id);
        if valid_local_name(&id) {
            if let Some(pending) = fs::read(pending_container_registry_path(&id))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|value| pending_container_from_record_value(&value))
            {
                return Some(pending);
            }
        }
        pending_container_records()
            .into_iter()
            .find(|pending| pending_container_ref_matches(pending, &id))
    }

    fn pending_container_records() -> Vec<PendingContainer> {
        let Ok(entries) = fs::read_dir(pending_container_registry_root()) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| fs::read(entry.path()).ok())
            .filter_map(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .filter_map(|value| pending_container_from_record_value(&value))
            .collect()
    }

    fn remove_pending_container_record(id: &str) -> Result<(), Value> {
        let id = normalize_container_ref(id);
        if valid_local_name(&id) {
            match fs::remove_file(pending_container_registry_path(&id)) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(json!({
                        "path": pending_container_registry_path(&id).display().to_string(),
                        "error": error.to_string(),
                    }));
                }
            }
        }
        for pending in pending_container_records() {
            if pending_container_identity_matches(&pending, &id) {
                let _ = fs::remove_file(pending_container_registry_path(&pending.name));
            }
        }
        Ok(())
    }

    fn pending_container_record_value(pending: &PendingContainer) -> Value {
        json!({
            "api": "cratebay.container.record.v1",
            "id": pending.id,
            "name": pending.name,
            "createdAt": pending.created_at,
            "runtimeId": pending.runtime_id,
            "image": pending.image,
            "command": pending.command,
            "env": pending.env,
            "workingDir": pending.working_dir,
            "mounts": pending.mounts.iter().map(|mount| json!({
                "source": mount.source,
                "target": mount.target,
                "readonly": mount.readonly,
            })).collect::<Vec<_>>(),
            "network": pending.network,
            "aliases": pending.aliases,
            "labels": pending.labels,
            "netnsName": pending.netns_name,
            "netnsPath": pending.netns_path.as_ref().map(|path| path.display().to_string()),
            "ports": pending.ports.iter().map(|port| json!({
                "hostIP": port.host_ip,
                "hostPort": port.host_port,
                "containerPort": port.container_port,
                "protocol": port.protocol,
            })).collect::<Vec<_>>(),
            "logPath": pending.log_path.display().to_string(),
            "noPull": pending.no_pull,
            "registryMirrors": pending.registry_mirrors,
            "privileged": pending.privileged,
            "startedWithCtr": pending.started_with_ctr,
            "exitCode": pending.exit_code,
            "managedBy": "cratebay",
        })
    }

    fn pending_container_from_record_value(value: &Value) -> Option<PendingContainer> {
        let raw_id = optional_string_value(value.get("id").or_else(|| value.get("Id")))?;
        let name = optional_string_value(value.get("name").or_else(|| value.get("Name")))?;
        let image = optional_string_value(value.get("image").or_else(|| value.get("Image")))?;
        if !valid_local_name(&name) {
            return None;
        }
        let id = if looks_like_docker_container_id(&raw_id) {
            raw_id.clone()
        } else {
            migrated_container_id(&raw_id, &name)
        };
        let runtime_id = optional_string_value(
            value
                .get("runtimeId")
                .or_else(|| value.get("runtime_id"))
                .or_else(|| value.get("TaskName")),
        )
        .unwrap_or_else(|| {
            if looks_like_docker_container_id(&raw_id) {
                name.clone()
            } else {
                raw_id.clone()
            }
        });
        let mut labels = object_map_or_empty(value.get("labels").or_else(|| value.get("Labels")));
        strip_completed_compose_replace_label(&name, &runtime_id, &mut labels);
        Some(PendingContainer {
            id,
            name: name.clone(),
            created_at: timestamp_seconds(
                value
                    .get("createdAt")
                    .or_else(|| value.get("created_at"))
                    .or_else(|| value.get("Created"))
                    .or_else(|| value.get("created")),
            )
            .unwrap_or_else(now_seconds),
            runtime_id,
            image,
            command: string_array(value.get("command").or_else(|| value.get("Command"))),
            env: string_array(value.get("env").or_else(|| value.get("Env"))),
            working_dir: optional_string_value(
                value
                    .get("workingDir")
                    .or_else(|| value.get("working_dir"))
                    .or_else(|| value.get("WorkingDir")),
            ),
            mounts: pending_record_mounts(value.get("mounts").or_else(|| value.get("Mounts"))),
            network: optional_string_value(value.get("network").or_else(|| value.get("Network"))),
            aliases: string_array(value.get("aliases").or_else(|| value.get("Aliases"))),
            labels,
            netns_name: optional_string_value(
                value
                    .get("netnsName")
                    .or_else(|| value.get("netns_name"))
                    .or_else(|| value.get("NetnsName")),
            ),
            netns_path: optional_string_value(
                value
                    .get("netnsPath")
                    .or_else(|| value.get("netns_path"))
                    .or_else(|| value.get("NetnsPath")),
            )
            .map(PathBuf::from),
            ports: pending_record_ports(value.get("ports").or_else(|| value.get("Ports"))),
            log_path: optional_string_value(value.get("logPath").or_else(|| value.get("LogPath")))
                .map(PathBuf::from)
                .unwrap_or_else(|| container_log_path(&name)),
            no_pull: bool_value(value.get("noPull").or_else(|| value.get("no_pull"))),
            registry_mirrors: normalize_registry_mirrors(
                value
                    .get("registryMirrors")
                    .or_else(|| value.get("registry_mirrors"))
                    .or_else(|| value.get("CrateBayRegistryMirrors")),
            ),
            privileged: bool_value(
                value
                    .get("privileged")
                    .or_else(|| value.get("Privileged"))
                    .or_else(|| {
                        value
                            .get("HostConfig")
                            .and_then(|host| host.get("Privileged"))
                    }),
            ),
            started_with_ctr: bool_value(
                value
                    .get("startedWithCtr")
                    .or_else(|| value.get("started_with_ctr")),
            ),
            exit_code: numeric_i64(value.get("exitCode").or_else(|| value.get("exit_code"))),
        })
    }

    fn pending_record_mounts(value: Option<&Value>) -> Vec<CtrMount> {
        let Some(Value::Array(items)) = value else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| {
                Some(CtrMount {
                    source: optional_string_value(
                        item.get("source").or_else(|| item.get("Source")),
                    )?,
                    target: optional_string_value(
                        item.get("target").or_else(|| item.get("Target")),
                    )?,
                    readonly: bool_value(item.get("readonly").or_else(|| item.get("Readonly"))),
                })
            })
            .collect()
    }

    fn pending_record_ports(value: Option<&Value>) -> Vec<CniPortMapping> {
        let Some(Value::Array(items)) = value else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| {
                let host_port = numeric_i64(item.get("hostPort").or_else(|| item.get("HostPort")))
                    .and_then(|value| u16::try_from(value).ok())?;
                let container_port = numeric_i64(
                    item.get("containerPort")
                        .or_else(|| item.get("ContainerPort")),
                )
                .and_then(|value| u16::try_from(value).ok())?;
                Some(CniPortMapping {
                    host_ip: optional_string_value(
                        item.get("hostIP")
                            .or_else(|| item.get("hostIp"))
                            .or_else(|| item.get("HostIp")),
                    ),
                    host_port,
                    container_port,
                    protocol: optional_string_value(
                        item.get("protocol").or_else(|| item.get("Protocol")),
                    )
                    .unwrap_or_else(|| "tcp".to_string()),
                })
            })
            .collect()
    }

    fn container_log_path(name: &str) -> PathBuf {
        let safe_name = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>();
        PathBuf::from("/run/cratebay/container-logs").join(format!("{safe_name}.log"))
    }

    fn volume_root() -> PathBuf {
        overridable_root(
            "CRATEBAY_VOLUME_REGISTRY_ROOT",
            "/var/lib/cratebay-engine/volumes",
            "volumes",
        )
    }

    fn volume_path(name: &str) -> PathBuf {
        volume_root().join(name)
    }

    fn volume_data_path(name: &str) -> PathBuf {
        volume_path(name).join("_data")
    }

    fn volume_name_from_data_path(source: &str) -> Option<String> {
        let relative = Path::new(source).strip_prefix(volume_root()).ok()?;
        let parts = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [name, data] if data == "_data" && valid_local_name(name) => Some(name.clone()),
            _ => None,
        }
    }

    fn volume_metadata_path(name: &str) -> PathBuf {
        volume_path(name).join("metadata.json")
    }

    fn directory_size_bytes(path: &Path) -> u64 {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return 0;
        };
        if metadata.is_file() {
            return metadata.len();
        }
        if !metadata.is_dir() {
            return 0;
        }

        fs::read_dir(path)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .map(|entry| directory_size_bytes(&entry.path()))
            .sum()
    }

    fn file_size_bytes(path: &Path) -> u64 {
        fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    }

    fn volume_value(name: &str) -> Value {
        let metadata = fs::read(volume_metadata_path(name))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or_else(|| json!({}));
        json!({
            "Name": name,
            "Driver": optional_string_value(metadata.get("Driver"))
                .unwrap_or_else(|| "local".to_string()),
            "Mountpoint": volume_data_path(name).display().to_string(),
            "CreatedAt": optional_string_value(metadata.get("CreatedAt"))
                .unwrap_or_else(chrono_like_now),
            "Labels": object_or_empty(metadata.get("Labels")),
            "Options": object_or_empty(metadata.get("Options")),
            "Scope": "local",
        })
    }

    fn network_root() -> PathBuf {
        overridable_root(
            "CRATEBAY_NETWORK_REGISTRY_ROOT",
            "/var/lib/cratebay-engine/networks",
            "networks",
        )
    }

    fn network_registry_path(name: &str) -> PathBuf {
        network_root().join(format!("{name}.json"))
    }

    fn cni_config_root() -> PathBuf {
        overridable_root("CRATEBAY_CNI_CONFIG_ROOT", "/etc/cni/net.d", "cni-netd")
    }

    fn cni_config_path(name: &str) -> PathBuf {
        cni_config_root().join(format!(
            "20-cratebay-{}.conflist",
            safe_network_file_name(name)
        ))
    }

    fn overridable_root(env_name: &str, default_path: &str, test_name: &str) -> PathBuf {
        if let Some(root) = std::env::var(env_name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
        {
            return root;
        }
        #[cfg(test)]
        {
            let _ = default_path;
            std::env::temp_dir()
                .join("cratebay-engine-adapter-tests")
                .join(test_thread_id())
                .join(test_name)
        }
        #[cfg(not(test))]
        {
            let _ = test_name;
            PathBuf::from(default_path)
        }
    }

    #[cfg(test)]
    fn test_thread_id() -> String {
        format!("{:?}", thread::current().id())
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>()
    }

    fn managed_network_values() -> Vec<Value> {
        let Ok(entries) = fs::read_dir(network_root()) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| fs::read(entry.path()).ok())
            .filter_map(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .collect()
    }

    fn managed_network_value_by_id(id: &str) -> Option<Value> {
        if !valid_local_name(id) {
            return None;
        }
        fs::read(network_registry_path(id))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    }

    fn managed_network_value(name: &str, payload: &Value) -> Value {
        let mut labels = object_or_empty(payload.get("Labels"));
        if let Some(labels) = labels.as_object_mut() {
            labels.insert("com.cratebay.managed".to_string(), json!("true"));
        }
        json!({
            "Id": name,
            "Name": name,
            "Driver": optional_string_value(payload.get("Driver")).unwrap_or_else(|| "bridge".to_string()),
            "Created": chrono_like_now(),
            "Scope": "local",
            "Internal": bool_value(payload.get("Internal")),
            "EnableIPv6": bool_value(payload.get("EnableIPv6")),
            "Labels": labels,
            "Options": object_or_empty(payload.get("Options")),
            "IPAM": object_or_empty(payload.get("IPAM")),
            "Containers": {},
            "CrateBay": {
                "backend": "cratebay-cni",
                "cniConfig": cni_config_path(name).display().to_string(),
            },
        })
    }

    fn managed_network_cni_config(name: &str, payload: &Value) -> Value {
        json!({
            "cniVersion": "1.0.0",
            "name": name,
            "plugins": [
                {
                    "type": "bridge",
                    "bridge": bridge_name_for_network(name),
                    "isGateway": true,
                    "ipMasq": !bool_value(payload.get("Internal")),
                    "ipam": managed_network_cni_ipam(name, payload),
                },
                {
                    "type": "portmap",
                    "capabilities": { "portMappings": true },
                }
            ]
        })
    }

    fn managed_network_cni_ipam(name: &str, payload: &Value) -> Value {
        if let Some(subnet) = payload
            .get("IPAM")
            .and_then(|ipam| ipam.get("Config"))
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("Subnet"))
            .and_then(Value::as_str)
            .filter(|subnet| !subnet.trim().is_empty())
        {
            return json!({
                "type": "host-local",
                "ranges": [[{ "subnet": subnet }]],
                "routes": [{ "dst": "0.0.0.0/0" }],
            });
        }
        json!({
            "type": "host-local",
            "ranges": [[{ "subnet": default_network_subnet(name) }]],
            "routes": [{ "dst": "0.0.0.0/0" }],
        })
    }

    fn default_network_subnet(name: &str) -> String {
        let bucket = name
            .bytes()
            .fold(0u8, |acc, byte| acc.wrapping_add(byte))
            .max(2);
        format!("10.88.{bucket}.0/24")
    }

    fn bridge_name_for_network(name: &str) -> String {
        format!(
            "cb{}",
            safe_network_file_name(name)
                .chars()
                .take(13)
                .collect::<String>()
        )
    }

    fn safe_network_file_name(name: &str) -> String {
        name.chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect()
    }

    fn valid_local_name(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    }

    #[cfg(test)]
    fn parse_exit_code(bytes: &[u8]) -> Option<i64> {
        String::from_utf8_lossy(bytes)
            .split(|ch: char| !ch.is_ascii_digit())
            .rfind(|part| !part.is_empty())
            .and_then(|part| part.parse::<i64>().ok())
    }

    fn exec_exit_code(output: &Output) -> i64 {
        exec_exit_code_from_parts(output.status.code(), &output.stdout, &output.stderr)
    }

    fn exec_exit_code_from_parts(status_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> i64 {
        parse_prefixed_exit_code(stderr, "exit code")
            .or_else(|| parse_prefixed_exit_code(stdout, "exit code"))
            .or_else(|| status_code.map(i64::from))
            .unwrap_or(126)
    }

    fn parse_prefixed_exit_code(bytes: &[u8], prefix: &str) -> Option<i64> {
        let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
        let (_, tail) = text.rsplit_once(prefix)?;
        tail.split(|ch: char| !ch.is_ascii_digit())
            .find(|part| !part.is_empty())
            .and_then(|part| part.parse::<i64>().ok())
    }

    fn empty_response(status: u16) -> HttpResponse {
        HttpResponse {
            status,
            reason: reason_phrase(status),
            content_type: "text/plain; charset=utf-8",
            upgrade: false,
            body: Vec::new(),
        }
    }

    fn managed_container_not_found(message: &str, id: &str) -> HttpResponse {
        error_response(
            404,
            message,
            json!({
                "container": id,
                "backend": "containerd",
                "error": "container is not managed by CrateBay",
            }),
        )
    }

    fn query_bool(query: &HashMap<String, String>, key: &str) -> bool {
        query
            .get(key)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }

    fn tail_lines(text: &str, tail: usize) -> String {
        let lines = text.lines().collect::<Vec<_>>();
        let start = lines.len().saturating_sub(tail);
        let mut result = lines[start..].join("\n");
        if !result.is_empty() && text.ends_with('\n') {
            result.push('\n');
        }
        result
    }

    fn docker_stream_response(stdout: Vec<u8>, stderr: Vec<u8>) -> HttpResponse {
        let mut body = Vec::new();
        append_docker_stream_frame(&mut body, 1, &stdout);
        append_docker_stream_frame(&mut body, 2, &stderr);

        HttpResponse {
            status: 200,
            reason: "OK",
            content_type: "application/vnd.docker.raw-stream",
            upgrade: false,
            body,
        }
    }

    fn docker_hijack_response(stdout: Vec<u8>, stderr: Vec<u8>) -> HttpResponse {
        let mut response = docker_stream_response(stdout, stderr);
        response.status = 101;
        response.reason = "UPGRADED";
        response.upgrade = true;
        response
    }

    fn json_stream_response(output: &Output) -> HttpResponse {
        let mut lines = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if !line.trim().is_empty() {
                lines.push(json!({ "stream": format!("{line}\n") }));
            }
        }
        for line in String::from_utf8_lossy(&output.stderr).lines() {
            if !line.trim().is_empty() {
                lines.push(json!({ "stream": format!("{line}\n") }));
            }
        }
        if lines.is_empty() {
            lines.push(json!({ "stream": "Done\n" }));
        }

        let body = lines
            .into_iter()
            .filter_map(|line| serde_json::to_string(&line).ok())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        HttpResponse {
            status: 200,
            reason: "OK",
            content_type: "application/json",
            upgrade: false,
            body: body.into_bytes(),
        }
    }

    fn append_docker_stream_frame(body: &mut Vec<u8>, stream: u8, payload: &[u8]) {
        if payload.is_empty() {
            return;
        }

        body.push(stream);
        body.extend_from_slice(&[0, 0, 0]);
        body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        body.extend_from_slice(payload);
    }

    fn chrono_like_now() -> String {
        chrono_like_timestamp(now_seconds())
    }

    fn chrono_like_timestamp(seconds: i64) -> String {
        let (year, month, day, hour, minute, second) = unix_seconds_to_utc(seconds);
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }

    fn unix_seconds_to_utc(seconds: i64) -> (i32, u32, u32, u32, u32, u32) {
        let days = seconds.div_euclid(86_400);
        let seconds_of_day = seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        (
            year,
            month,
            day,
            (seconds_of_day / 3600) as u32,
            ((seconds_of_day % 3600) / 60) as u32,
            (seconds_of_day % 60) as u32,
        )
    }

    fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
        let z = days_since_epoch + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_prime = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
        let month = month_prime + if month_prime < 10 { 3 } else { -9 };
        year += if month <= 2 { 1 } else { 0 };
        (year as i32, month as u32, day as u32)
    }

    fn parse_chrono_like_timestamp(text: &str) -> Option<i64> {
        let parts = text
            .split(|ch: char| !ch.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 6 {
            return None;
        }
        let year = parts[0].parse::<i32>().ok()?;
        let month = parts[1].parse::<u32>().ok()?;
        let day = parts[2].parse::<u32>().ok()?;
        let hour = parts[3].parse::<u32>().ok()?;
        let minute = parts[4].parse::<u32>().ok()?;
        let second = parts[5].parse::<u32>().ok()?;
        if !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        Some(
            days_from_civil(year, month, day) * 86_400
                + i64::from(hour) * 3_600
                + i64::from(minute) * 60
                + i64::from(second),
        )
    }

    fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
        let year = i64::from(year) - i64::from(month <= 2);
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month = i64::from(month);
        let month_prime = month + if month > 2 { -3 } else { 9 };
        let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    fn normalize_image_inspect(value: Value, fallback_id: &str) -> Value {
        let repository = string_field(&value, &["Repository", "RepositoryName", "Name"])
            .unwrap_or_else(|| "<none>".to_string());
        let tag = string_field(&value, &["Tag"]).unwrap_or_else(|| "latest".to_string());
        let repo_tags = match string_array(value.get("RepoTags")) {
            tags if !tags.is_empty() => tags,
            _ => repo_tags_from_parts(&repository, &tag),
        };
        let id = string_field(&value, &["Id", "ID", "ImageID", "Digest"])
            .unwrap_or_else(|| fallback_id.to_string());
        let size = size_bytes(value.get("Size"));
        let labels = object_or_empty(
            nested_value(&value, &["Config", "Labels"]).or_else(|| value.get("Labels")),
        );

        json!({
            "Id": normalize_image_id(&id),
            "RepoTags": repo_tags,
            "RepoDigests": string_array(value.get("RepoDigests")),
            "Parent": string_field(&value, &["Parent"]).unwrap_or_default(),
            "Comment": "",
            "Created": string_field(&value, &["Created", "CreatedAt"]).unwrap_or_else(chrono_like_now),
            "Container": "",
            "ContainerConfig": object_or_empty(value.get("ContainerConfig")),
            "DockerVersion": "cratebay-containerd",
            "Author": string_field(&value, &["Author"]).unwrap_or_default(),
            "Config": {
                "Labels": labels,
            },
            "Architecture": string_field(&value, &["Architecture"]).unwrap_or_else(|| std::env::consts::ARCH.to_string()),
            "Os": string_field(&value, &["Os", "OS"]).unwrap_or_else(|| "linux".to_string()),
            "Size": size,
            "VirtualSize": size,
            "GraphDriver": {
                "Name": "containerd",
                "Data": {},
            },
            "RootFS": value
                .get("RootFS")
                .cloned()
                .unwrap_or_else(|| json!({ "Type": "layers", "Layers": [] })),
            "Metadata": {
                "LastTagTime": "0001-01-01T00:00:00Z",
            },
        })
    }

    fn inspect_network_value(_config: &Config, id: &str) -> Result<Value, Value> {
        if let Some(network) = managed_network_value_by_id(id) {
            return Ok(network);
        }
        Err(json!({
            "id": id,
            "backend": "cratebay-cni",
            "error": "network is not managed by CrateBay",
        }))
    }

    fn normalize_network_inspect(value: Value, fallback_id: &str) -> Value {
        let name = string_field(&value, &["Name", "NAME", "name"])
            .unwrap_or_else(|| fallback_id.to_string());
        let id = string_field(&value, &["Id", "ID", "id"]).unwrap_or_else(|| name.clone());
        let driver = string_field(&value, &["Driver", "DRIVER", "driver"])
            .or_else(|| first_plugin_string(&value, "type"))
            .unwrap_or_else(|| "bridge".to_string());
        let ipam = value
            .get("IPAM")
            .cloned()
            .or_else(|| cni_ipam_value(&value))
            .unwrap_or_else(|| json!({ "Driver": "default", "Config": [], "Options": {} }));

        json!({
            "Name": name,
            "Id": id,
            "Created": string_field(&value, &["Created", "CreatedAt", "created"]).unwrap_or_else(chrono_like_now),
            "Scope": string_field(&value, &["Scope", "scope"]).unwrap_or_else(|| "local".to_string()),
            "Driver": driver,
            "EnableIPv6": bool_value(value.get("EnableIPv6").or_else(|| value.get("enable_ipv6"))),
            "IPAM": ipam,
            "Internal": bool_value(value.get("Internal").or_else(|| value.get("internal"))),
            "Attachable": value
                .get("Attachable")
                .or_else(|| value.get("attachable"))
                .map(|value| bool_value(Some(value)))
                .unwrap_or(true),
            "Ingress": bool_value(value.get("Ingress").or_else(|| value.get("ingress"))),
            "ConfigFrom": value
                .get("ConfigFrom")
                .cloned()
                .unwrap_or_else(|| json!({ "Network": "" })),
            "ConfigOnly": bool_value(value.get("ConfigOnly").or_else(|| value.get("config_only"))),
            "Containers": object_or_empty(value.get("Containers").or_else(|| value.get("containers"))),
            "Options": object_or_empty(value.get("Options").or_else(|| value.get("options"))),
            "Labels": object_or_empty(value.get("Labels").or_else(|| value.get("labels"))),
        })
    }

    fn normalize_network_inspect_with_pending(
        state: &AdapterState,
        value: Value,
        fallback_id: &str,
    ) -> Value {
        let mut network = normalize_network_inspect(value, fallback_id);
        let network_name =
            string_field(&network, &["Name"]).unwrap_or_else(|| fallback_id.to_string());
        let Some(containers) = network.get_mut("Containers").and_then(Value::as_object_mut) else {
            return network;
        };

        for pending in unique_pending_containers(state) {
            if !pending.started_with_ctr
                || pending.network.as_deref() != Some(network_name.as_str())
            {
                continue;
            }
            let (ip, prefix) = pending_ipv4_address(&pending).unwrap_or_else(|| (String::new(), 0));
            let ipv4_address = if ip.is_empty() {
                String::new()
            } else {
                format!("{ip}/{prefix}")
            };
            containers.insert(
                pending.id.clone(),
                json!({
                    "Name": pending.name,
                    "EndpointID": "",
                    "MacAddress": "",
                    "IPv4Address": ipv4_address,
                    "IPv6Address": "",
                }),
            );
        }

        network
    }

    fn first_plugin_string(value: &Value, key: &str) -> Option<String> {
        value
            .get("plugins")
            .or_else(|| value.get("Plugins"))
            .and_then(Value::as_array)
            .and_then(|plugins| plugins.first())
            .and_then(|plugin| nested_string(plugin, &[key]))
    }

    fn cni_ipam_value(value: &Value) -> Option<Value> {
        let ipam = value
            .get("ipam")
            .or_else(|| value.get("IPAM"))
            .or_else(|| {
                value
                    .get("plugins")
                    .or_else(|| value.get("Plugins"))
                    .and_then(Value::as_array)
                    .and_then(|plugins| plugins.iter().find_map(|plugin| plugin.get("ipam")))
            })?;

        let mut config = serde_json::Map::new();
        if let Some(subnet) =
            nested_string(ipam, &["subnet"]).or_else(|| nested_string(ipam, &["Subnet"]))
        {
            config.insert("Subnet".to_string(), Value::String(subnet));
        }
        if let Some(gateway) =
            nested_string(ipam, &["gateway"]).or_else(|| nested_string(ipam, &["Gateway"]))
        {
            config.insert("Gateway".to_string(), Value::String(gateway));
        }
        if let Some(range) =
            nested_string(ipam, &["range"]).or_else(|| nested_string(ipam, &["IPRange"]))
        {
            config.insert("IPRange".to_string(), Value::String(range));
        }

        Some(json!({
            "Driver": nested_string(ipam, &["type"]).unwrap_or_else(|| "host-local".to_string()),
            "Config": if config.is_empty() {
                Vec::<Value>::new()
            } else {
                vec![Value::Object(config)]
            },
            "Options": {},
        }))
    }

    fn parse_filters(raw: Option<&String>) -> HashMap<String, Vec<String>> {
        let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
            return HashMap::new();
        };
        let Ok(Value::Object(filters)) = serde_json::from_str::<Value>(raw) else {
            return HashMap::new();
        };

        filters
            .into_iter()
            .map(|(key, value)| (key, string_array(Some(&value))))
            .collect()
    }

    fn network_matches_filters(network: &Value, filters: &HashMap<String, Vec<String>>) -> bool {
        filters.iter().all(|(key, expected)| {
            if expected.is_empty() {
                return true;
            }

            match key.as_str() {
                "label" => expected
                    .iter()
                    .all(|filter| network_label_matches(network, filter)),
                "name" => {
                    let name = nested_string(network, &["Name"]).unwrap_or_default();
                    expected.iter().any(|filter| name.contains(filter))
                }
                "id" => {
                    let id = nested_string(network, &["Id"]).unwrap_or_default();
                    expected.iter().any(|filter| id.contains(filter))
                }
                "driver" => {
                    let driver = nested_string(network, &["Driver"]).unwrap_or_default();
                    expected.contains(&driver)
                }
                "scope" => {
                    let scope = nested_string(network, &["Scope"]).unwrap_or_default();
                    expected.contains(&scope)
                }
                "type" => true,
                _ => true,
            }
        })
    }

    fn network_label_matches(network: &Value, filter: &str) -> bool {
        let Some(labels) = network.get("Labels").and_then(Value::as_object) else {
            return false;
        };
        match filter.split_once('=') {
            Some((key, value)) => labels
                .get(key)
                .and_then(Value::as_str)
                .map(|found| found == value)
                .unwrap_or(false),
            None => labels.contains_key(filter),
        }
    }

    fn normalize_image_id(id: &str) -> String {
        if id.is_empty() || id.starts_with("sha256:") {
            id.to_string()
        } else {
            format!("sha256:{id}")
        }
    }

    fn repo_tags_from_parts(repository: &str, tag: &str) -> Vec<String> {
        if repository.is_empty() || repository == "<none>" {
            return Vec::new();
        }

        if tag.is_empty() || tag == "<none>" {
            vec![repository.to_string()]
        } else {
            vec![format!("{repository}:{tag}")]
        }
    }

    #[cfg(test)]
    fn docker_state_from_status(status: &str) -> String {
        let normalized = status.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return "created".to_string();
        }
        if normalized == "running" || normalized.contains("up ") || normalized.starts_with("up") {
            "running".to_string()
        } else if normalized.contains("paused") {
            "paused".to_string()
        } else if normalized.contains("restarting") {
            "restarting".to_string()
        } else if normalized.contains("removing") {
            "removing".to_string()
        } else if normalized.contains("dead") {
            "dead".to_string()
        } else if normalized.contains("exited") || normalized.contains("stopped") {
            "exited".to_string()
        } else if normalized.contains("created") {
            "created".to_string()
        } else {
            normalized
        }
    }

    fn integer_string(value: Option<&Value>) -> Option<String> {
        numeric_i64(value).map(|value| value.to_string())
    }

    fn numeric_i64(value: Option<&Value>) -> Option<i64> {
        match value? {
            Value::Number(number) => number
                .as_i64()
                .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
                .or_else(|| number.as_f64().map(|value| value as i64)),
            Value::String(text) => text.trim().parse::<i64>().ok(),
            _ => None,
        }
    }

    fn timestamp_seconds(value: Option<&Value>) -> Option<i64> {
        numeric_i64(value).or_else(|| {
            optional_string_value(value).and_then(|text| parse_chrono_like_timestamp(&text))
        })
    }

    fn numeric_f64(value: Option<&Value>) -> Option<f64> {
        match value? {
            Value::Number(number) => number
                .as_f64()
                .or_else(|| number.as_i64().map(|value| value as f64))
                .or_else(|| number.as_u64().map(|value| value as f64)),
            Value::String(text) => text.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    fn size_bytes(value: Option<&Value>) -> i64 {
        match value {
            Some(Value::Number(_)) => numeric_i64(value).unwrap_or_default(),
            Some(Value::String(text)) => parse_human_size(text).unwrap_or_default(),
            _ => 0,
        }
    }

    fn parse_human_size(raw: &str) -> Option<i64> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut number = String::new();
        let mut unit = String::new();
        for ch in trimmed.chars() {
            if ch.is_ascii_digit() || ch == '.' {
                number.push(ch);
            } else if !ch.is_whitespace() {
                unit.push(ch.to_ascii_lowercase());
            }
        }

        let value = number.parse::<f64>().ok()?;
        let multiplier = match unit.as_str() {
            "" | "b" | "byte" | "bytes" => 1.0,
            "k" | "kb" | "kib" => 1024.0,
            "m" | "mb" | "mib" => 1024.0 * 1024.0,
            "g" | "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
            "t" | "tb" | "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            _ => 1.0,
        };
        Some((value * multiplier) as i64)
    }

    fn nested_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
        let mut current = value;
        for key in path {
            current = current.get(*key)?;
        }
        Some(current)
    }

    fn nested_string(value: &Value, path: &[&str]) -> Option<String> {
        nested_value(value, path).and_then(|value| optional_string_value(Some(value)))
    }

    fn object_or_empty(value: Option<&Value>) -> Value {
        match value {
            Some(Value::Object(_)) => value.cloned().unwrap_or_else(|| json!({})),
            _ => json!({}),
        }
    }

    fn object_map_or_empty(value: Option<&Value>) -> serde_json::Map<String, Value> {
        match value {
            Some(Value::Object(object)) => object.clone(),
            _ => serde_json::Map::new(),
        }
    }

    fn string_field(value: &Value, names: &[&str]) -> Option<String> {
        names.iter().find_map(|name| {
            value.get(name).and_then(|field| match field {
                Value::String(text) if !text.is_empty() => Some(text.clone()),
                _ => None,
            })
        })
    }

    fn read_kernel_version() -> String {
        fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn text_response(status: u16, reason: &'static str, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            reason,
            content_type: "text/plain; charset=utf-8",
            upgrade: false,
            body: body.as_bytes().to_vec(),
        }
    }

    fn json_response(status: u16, body: Value) -> HttpResponse {
        HttpResponse {
            status,
            reason: reason_phrase(status),
            content_type: "application/json",
            upgrade: false,
            body: serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec()),
        }
    }

    fn error_response(status: u16, message: &str, details: Value) -> HttpResponse {
        json_response(status, json!({ "message": message, "details": details }))
    }

    fn reason_phrase(status: u16) -> &'static str {
        match status {
            101 => "UPGRADED",
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            400 => "Bad Request",
            404 => "Not Found",
            409 => "Conflict",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            _ => "OK",
        }
    }

    fn write_response(stream: &mut UnixStream, response: HttpResponse) -> Result<(), String> {
        let headers = if response.upgrade {
            format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nConnection: Upgrade\r\nUpgrade: tcp\r\n\r\n",
                response.status, response.reason, response.content_type
            )
        } else {
            format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.status,
                response.reason,
                response.content_type,
                response.body.len()
            )
        };
        stream
            .write_all(headers.as_bytes())
            .and_then(|_| stream.write_all(&response.body))
            .and_then(|_| stream.flush())
            .map_err(|error| format!("write response: {error}"))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            append_log_bytes, apply_log_tail, archive_target_path, attach_stream_flags,
            build_ctr_run_args, build_ctr_run_args_with_netns, build_network_connect_args,
            build_network_disconnect_args, cmdline_value, cni_plugin_input,
            cni_port_mappings_value, configure_buildkit_proxy_worker, connect_network,
            container_action, container_cgroup_mount, container_rootfs_candidates,
            containerd_cleanup_args, containerd_hosts_namespaces, containerd_task_name,
            cratebay_container_terminal_action, cratebay_engine_payload,
            cratebay_substrate_payload, create_container, create_managed_network, create_volume,
            ctr_image_export_args, ctr_image_pull_args, ctr_image_pull_candidates,
            ctr_image_summary, ctr_mount_arg, ctr_run_envs, decode_http_chunked_body,
            dedupe_docker_items_by_name, dedupe_native_items_by_name, disconnect_network,
            docker_archive_config, docker_state_from_status, docker_stats_value,
            docker_stream_response, ensure_container_system_mounts, exec_ctr_args,
            extend_limited_bytes, image_action, image_ref_matches, image_ref_parts,
            image_refs_equivalent, inspect_container, inspect_cratebay_pod, inspect_volume,
            ip_binary, is_cratebay_pod_network, list_containers, list_cratebay_pods,
            list_cratebay_shim_tasks, looks_like_docker_container_id, managed_network_cni_config,
            managed_network_value, mark_pending_started_with_ctr, merged_container_command,
            native_attach_container_to_pod, native_create_request,
            native_detach_container_from_pod, native_image_inspect_payload, native_image_ref,
            native_inspect_network, native_inspect_volume, native_network_create_request,
            native_pack_container_image_payload, native_pod_create_request, native_port_binding,
            native_reap_shim_task, native_remove_image, native_remove_network, native_remove_pod,
            native_remove_volume, native_stats_value, native_storage_gc,
            native_volume_create_request, native_wait_container, network_action, network_id_path,
            network_matches_filters, normalize_docker_path, normalize_docker_request_path,
            normalize_http_proxy_url, normalize_network_inspect, normalize_registry_mirrors,
            oci_architecture, open_terminal_pty, parse_bind_mount_spec, parse_containerd_metrics,
            parse_exit_code, parse_filters, parse_human_size, parse_ip_addr_show,
            parse_registry_image_ref, patch_legacy_python36_ctypes_file, pending_archive_mounts,
            pending_archive_stage_path, pending_container, pending_container_from_record_value,
            pending_container_record_value, pending_container_registry_path,
            pending_from_create_payload, pending_inspect_value, pending_network_attachment,
            pending_port_mappings, pending_summary_value, pty_window_size, put_container_archive,
            query_bool_or, read_request, relax_runc_seccomp_default, remove_container,
            rename_container, reset_pending_runtime_state_for_start,
            rewrite_image_for_registry_mirror, rootfs_archive_skip, running_network_attachment,
            select_containerd_image_ref, select_containerd_image_ref_from_refs,
            set_pty_window_size, spawn_container_log_reader, stop_container,
            store_pending_container, task_list_contains, temp_archive_path, terminal_ctr_args,
            terminal_session_id, terminal_size_from_payload, truncate_bytes, unique_task_id,
            unix_seconds_to_utc, wait_container, AdapterState, CniPortMapping, Config,
            ContainerCommitResult, ContainerRuntimeMetrics, CtrMount, ExecRecord, PendingContainer,
            DEFAULT_ENGINE_ADAPTER_SOCKET, ENGINE_TMP_ROOT,
        };
        use serde_json::json;
        use std::collections::HashMap;
        use std::fs;
        use std::io::{Cursor, Write};
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixStream;
        use std::path::PathBuf;
        use std::sync::{Arc, Mutex, OnceLock};
        use std::thread;

        fn env_lock() -> std::sync::MutexGuard<'static, ()> {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
        }

        fn clear_adapter_socket_env() {
            std::env::remove_var("CRATEBAY_ENGINE_ADAPTER_SOCKET");
            std::env::remove_var("CRATEBAY_DOCKER_ADAPTER_SOCKET");
        }

        fn test_state_without_external_frontend() -> AdapterState {
            let _ = std::fs::remove_dir_all(super::pending_container_registry_root());
            let _ = std::fs::remove_dir_all(super::network_root());
            let _ = std::fs::remove_dir_all(super::cni_config_root());
            test_state_without_external_frontend_preserving_registry()
        }

        fn test_state_without_external_frontend_preserving_registry() -> AdapterState {
            AdapterState {
                config: Config {
                    socket: PathBuf::from("/run/cratebay/engine.sock"),
                    containerd_socket: PathBuf::from("/run/containerd/containerd.sock"),
                    namespace: "cratebay-test".to_string(),
                    ctr: "ctr".to_string(),
                },
                execs: Arc::new(Mutex::new(HashMap::new())),
                terminals: Arc::new(Mutex::new(HashMap::new())),
                metrics: Arc::new(Mutex::new(HashMap::new())),
                pending_containers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        #[test]
        fn default_adapter_socket_is_engine_sock() {
            assert_eq!(DEFAULT_ENGINE_ADAPTER_SOCKET, "/run/cratebay/engine.sock");
        }

        #[test]
        fn engine_adapter_socket_env_takes_priority_over_legacy_alias() {
            let _guard = env_lock();
            clear_adapter_socket_env();
            std::env::set_var(
                "CRATEBAY_DOCKER_ADAPTER_SOCKET",
                "/run/cratebay/docker.sock",
            );
            std::env::set_var(
                "CRATEBAY_ENGINE_ADAPTER_SOCKET",
                "/run/cratebay/engine.sock",
            );

            assert_eq!(
                super::env_adapter_socket_path(),
                Some(PathBuf::from("/run/cratebay/engine.sock"))
            );

            clear_adapter_socket_env();
        }

        #[test]
        fn legacy_adapter_socket_env_is_still_supported() {
            let _guard = env_lock();
            clear_adapter_socket_env();
            std::env::set_var(
                "CRATEBAY_DOCKER_ADAPTER_SOCKET",
                "/run/cratebay/docker.sock",
            );

            assert_eq!(
                super::env_adapter_socket_path(),
                Some(PathBuf::from("/run/cratebay/docker.sock"))
            );

            clear_adapter_socket_env();
        }

        #[test]
        fn read_request_spools_native_image_import_body() {
            let (mut client, mut server) = UnixStream::pair().expect("unix stream pair");
            let body = b"cratebay-large-image-body".repeat(128);
            let expected = body.clone();
            let writer = thread::spawn(move || {
                let header = format!(
                    "POST /cratebay/images/import HTTP/1.1\r\nHost: cratebay\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                client.write_all(header.as_bytes()).expect("write header");
                client.write_all(&body).expect("write body");
            });

            let request = read_request(&mut server).expect("read request");
            writer.join().expect("writer thread");

            assert_eq!(request.method, "POST");
            assert_eq!(request.path, "/cratebay/images/import");
            assert!(request.body.is_empty());
            let spool_path = request
                .body_spool_path
                .as_ref()
                .expect("spooled import body path");
            assert_eq!(
                std::fs::read(spool_path).expect("read spooled body"),
                expected
            );
            let _ = std::fs::remove_file(spool_path);
        }

        #[test]
        fn read_request_decodes_chunked_body() {
            let (mut client, mut server) = UnixStream::pair().expect("unix stream pair");
            let writer = thread::spawn(move || {
                client
                    .write_all(
                        b"PUT /containers/demo/archive?path=/etc/buildkit HTTP/1.1\r\n\
Host: cratebay\r\n\
Transfer-Encoding: chunked\r\n\r\n\
5\r\nhello\r\n\
6\r\n world\r\n\
0\r\n\r\n",
                    )
                    .expect("write chunked request");
            });

            let request = read_request(&mut server).expect("read request");
            writer.join().expect("writer thread");

            assert_eq!(request.method, "PUT");
            assert_eq!(request.path, "/containers/demo/archive?path=/etc/buildkit");
            assert_eq!(request.body, b"hello world");
            assert!(request.body_spool_path.is_none());
        }

        #[test]
        fn image_archive_temp_files_use_engine_storage_root() {
            assert_eq!(ENGINE_TMP_ROOT, "/var/lib/cratebay-engine/tmp");

            let archive_path = temp_archive_path("cratebay-test-archive");
            assert!(
                archive_path
                    .parent()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    == Some("tmp"),
                "archive temp path should live under the Engine tmp root: {}",
                archive_path.display()
            );
        }

        #[test]
        fn ip_binary_respects_cratebay_override() {
            let previous = std::env::var_os("CRATEBAY_IP");
            std::env::set_var("CRATEBAY_IP", "/custom/iproute2/ip");

            assert_eq!(ip_binary(), "/custom/iproute2/ip");

            if let Some(value) = previous {
                std::env::set_var("CRATEBAY_IP", value);
            } else {
                std::env::remove_var("CRATEBAY_IP");
            }
        }

        #[test]
        fn rootfs_candidates_follow_containerd_task_layout() {
            let state = test_state_without_external_frontend();
            let (_, payload, _) = native_create_request(&json!({
                "name": "sandbox-demo",
                "image": "alpine:3.20",
                "command": "sleep 60",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload("sandbox-demo", &payload);

            assert_eq!(
                container_rootfs_candidates(&state.config, &pending),
                vec![
                    PathBuf::from(
                        "/run/containerd/io.containerd.runtime.v2.task/cratebay-test/sandbox-demo/rootfs",
                    ),
                    PathBuf::from(format!(
                        "/run/containerd/io.containerd.runtime.v2.task/cratebay-test/{}/rootfs",
                        pending.id
                    )),
                ]
            );
        }

        #[test]
        fn rootfs_archive_skips_runtime_pseudo_filesystems() {
            for top in ["dev", "proc", "run", "sys"] {
                assert!(rootfs_archive_skip(&PathBuf::from(top).join("anything")));
            }
            assert!(!rootfs_archive_skip(&PathBuf::from("usr/bin/tool")));
            assert!(!rootfs_archive_skip(&PathBuf::from("tmp/output.txt")));
        }

        #[test]
        fn docker_archive_config_records_real_container_command_and_layer() {
            let (_, payload, _) = native_create_request(&json!({
                "name": "pack-demo",
                "image": "alpine:3.20",
                "entrypoint": "/bin/sh",
                "command": "-lc echo packed",
                "env": ["A=B"],
                "workingDir": "/workspace",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload("pack-demo", &payload);
            let config = docker_archive_config(&pending, "2026-01-01T00:00:00Z", "abc123");

            assert_eq!(config["architecture"], oci_architecture());
            assert_eq!(config["config"]["Env"][0], "A=B");
            assert_eq!(config["config"]["WorkingDir"], "/workspace");
            assert_eq!(config["config"]["Cmd"][0], "/bin/sh");
            assert_eq!(config["rootfs"]["diff_ids"][0], "sha256:abc123");
            assert_eq!(
                config["history"][0]["created_by"],
                "cratebay engine pack-container"
            );
        }

        #[test]
        fn strips_docker_api_version_prefix() {
            assert_eq!(normalize_docker_path("/v1.44/_ping"), "/_ping");
            assert_eq!(
                normalize_docker_path("/v1.44/containers/json?all=true"),
                "/containers/json"
            );
        }

        #[test]
        fn keeps_unversioned_paths() {
            assert_eq!(normalize_docker_path("/_ping"), "/_ping");
            assert_eq!(
                normalize_docker_path("/containers/json?all=true"),
                "/containers/json"
            );
        }

        #[test]
        fn cratebay_engine_payload_identifies_containerd_backend_and_compat_layer() {
            let payload = cratebay_engine_payload(&Config {
                socket: PathBuf::from("/run/cratebay/engine.sock"),
                containerd_socket: PathBuf::from("/run/containerd/containerd.sock"),
                namespace: "cratebay-test".to_string(),
                ctr: "ctr".to_string(),
            });

            assert_eq!(payload["name"], "CrateBay Engine");
            assert_eq!(payload["kind"], "cratebay-containerd");
            assert_eq!(payload["backend"]["runtime"], "containerd");
            assert_eq!(payload["backend"]["namespace"], "cratebay-test");
            assert_eq!(payload["adapter"]["api"], "cratebay.engine.v1");
            assert_eq!(payload["compatibility"]["dockerCompatible"], true);
            assert_eq!(payload["compatibility"]["dockerApiVersion"], "1.44");
        }

        #[test]
        fn substrate_payload_identifies_cratebay_owned_managers() {
            let state = test_state_without_external_frontend();
            let payload = cratebay_substrate_payload(&state);

            assert_eq!(payload["api"], "cratebay.substrate.v1");
            assert_eq!(payload["engine"], "CrateBay Engine");
            assert_eq!(payload["daemon"]["docker"], "none");
            assert_eq!(payload["compatibility"]["dockerDaemon"], false);
            assert_eq!(payload["shim"]["manager"], "cratebay-containerd-shim");
            assert_eq!(payload["shim"]["backend"], "containerd task service");
            assert_eq!(payload["network"]["manager"], "cratebay-cni");
            assert_eq!(payload["network"]["stack"], "CNI");
            assert_eq!(payload["storage"]["manager"], "cratebay-storage");
            assert_eq!(payload["storage"]["gc"]["dryRunDefault"], true);
        }

        #[test]
        fn storage_gc_dry_runs_then_prunes_exited_metadata_when_applied() {
            let state = test_state_without_external_frontend();
            let log_path = std::env::temp_dir()
                .join("cratebay-engine-adapter-tests")
                .join(super::test_thread_id())
                .join("gc-demo.log");
            std::fs::create_dir_all(log_path.parent().expect("log parent")).expect("log dir");
            std::fs::write(&log_path, b"finished\n").expect("log file");
            let id = super::container_id_from_seed("gc-demo");
            let pending = PendingContainer {
                id: id.clone(),
                name: "gc-demo".to_string(),
                created_at: 1_780_000_000,
                runtime_id: "gc-demo".to_string(),
                image: "alpine:3.20".to_string(),
                command: vec!["true".to_string()],
                env: vec![],
                working_dir: None,
                mounts: vec![],
                network: None,
                aliases: vec![],
                labels: serde_json::Map::new(),
                netns_name: None,
                netns_path: None,
                ports: vec![],
                log_path: log_path.clone(),
                no_pull: false,
                registry_mirrors: vec![],
                privileged: false,
                started_with_ctr: true,
                exit_code: Some(0),
            };
            store_pending_container(&state, &id, "gc-demo", pending).expect("store pending");

            let dry_run = native_storage_gc(&state, br#"{}"#);
            let dry_run_payload: serde_json::Value =
                serde_json::from_slice(&dry_run.body).expect("dry-run payload");
            assert_eq!(dry_run_payload["dryRun"], true);
            assert_eq!(dry_run_payload["candidateCount"], 1);
            assert!(log_path.exists());
            assert!(pending_container(&state, "gc-demo").is_some());

            let applied = native_storage_gc(&state, br#"{ "apply": true }"#);
            let applied_payload: serde_json::Value =
                serde_json::from_slice(&applied.body).expect("applied payload");
            assert_eq!(applied_payload["applied"], true);
            assert_eq!(applied_payload["removed"].as_array().map(Vec::len), Some(1));
            assert!(!log_path.exists());
            assert!(pending_container(&state, "gc-demo").is_none());
        }

        #[test]
        fn native_network_and_volume_inspect_expose_cratebay_managers() {
            let state = test_state_without_external_frontend();
            let (_, network_payload) = native_network_create_request(&json!({
                "name": "inspect-net",
                "ipam": { "Config": [{ "Subnet": "10.99.1.0/24" }] }
            }))
            .expect("network payload");
            create_managed_network(&state.config, &network_payload).expect("managed network");

            let network = native_inspect_network(&state, "inspect-net".to_string());
            let network_payload: serde_json::Value =
                serde_json::from_slice(&network.body).expect("network inspect payload");
            assert_eq!(network_payload["api"], "cratebay.network.inspect.v1");
            assert_eq!(network_payload["backend"], "cratebay-cni");
            assert_eq!(network_payload["item"]["managedBy"], "cratebay");
            assert_eq!(
                network_payload["inspect"]["IPAM"]["Config"][0]["Subnet"],
                "10.99.1.0/24"
            );

            std::fs::create_dir_all(super::volume_data_path("inspect-volume")).expect("volume dir");
            std::fs::write(
                super::volume_data_path("inspect-volume").join("payload.txt"),
                b"cratebay",
            )
            .expect("volume payload");
            let volume = native_inspect_volume("inspect-volume".to_string());
            let volume_payload: serde_json::Value =
                serde_json::from_slice(&volume.body).expect("volume inspect payload");
            assert_eq!(volume_payload["api"], "cratebay.volume.inspect.v1");
            assert_eq!(volume_payload["backend"], "cratebay-storage");
            assert_eq!(volume_payload["item"]["managedBy"], "cratebay");
            assert_eq!(volume_payload["item"]["sizeBytes"], 8);
        }

        #[test]
        fn shim_tasks_list_and_reap_exited_task_metadata() {
            let state = test_state_without_external_frontend();
            let log_path = std::env::temp_dir()
                .join("cratebay-engine-adapter-tests")
                .join(super::test_thread_id())
                .join("shim-reap-demo.log");
            std::fs::create_dir_all(log_path.parent().expect("log parent")).expect("log dir");
            std::fs::write(&log_path, b"done\n").expect("log file");
            let pending = PendingContainer {
                id: "shim-reap-id".to_string(),
                name: "shim-reap-demo".to_string(),
                created_at: 1_780_000_000,
                runtime_id: "shim-reap-demo".to_string(),
                image: "alpine:3.20".to_string(),
                command: vec!["true".to_string()],
                env: vec![],
                working_dir: None,
                mounts: vec![],
                network: Some("inspect-net".to_string()),
                aliases: vec![],
                labels: serde_json::Map::new(),
                netns_name: Some("cb-inspect-net-shim-reap-demo".to_string()),
                netns_path: Some(PathBuf::from(
                    "/var/run/netns/cb-inspect-net-shim-reap-demo",
                )),
                ports: vec![CniPortMapping {
                    host_ip: Some("127.0.0.1".to_string()),
                    host_port: 18080,
                    container_port: 80,
                    protocol: "tcp".to_string(),
                }],
                log_path: log_path.clone(),
                no_pull: false,
                registry_mirrors: vec![],
                privileged: false,
                started_with_ctr: true,
                exit_code: Some(0),
            };
            store_pending_container(&state, "shim-reap-id", "shim-reap-demo", pending)
                .expect("store pending");

            let listed = list_cratebay_shim_tasks(&state);
            let listed_payload: serde_json::Value =
                serde_json::from_slice(&listed.body).expect("shim list payload");
            assert_eq!(listed_payload["api"], "cratebay.shim.tasks.v1");
            assert_eq!(
                listed_payload["items"][0]["manager"],
                "cratebay-containerd-shim"
            );
            assert_eq!(listed_payload["items"][0]["state"], "exited");
            assert_eq!(listed_payload["items"][0]["ports"][0]["hostPort"], 18080);

            let dry_run = native_reap_shim_task(&state, "shim-reap-demo".to_string(), br#"{}"#);
            let dry_run_payload: serde_json::Value =
                serde_json::from_slice(&dry_run.body).expect("reap dry-run payload");
            assert_eq!(dry_run_payload["dryRun"], true);
            assert!(pending_container(&state, "shim-reap-demo").is_some());

            let applied = native_reap_shim_task(
                &state,
                "shim-reap-demo".to_string(),
                br#"{ "apply": true }"#,
            );
            let applied_payload: serde_json::Value =
                serde_json::from_slice(&applied.body).expect("reap applied payload");
            assert_eq!(applied_payload["applied"], true);
            assert!(pending_container(&state, "shim-reap-demo").is_none());
            assert!(!log_path.exists());
        }

        #[test]
        fn parses_query_and_percent_decodes_path_ids() {
            let (path, query) =
                normalize_docker_request_path("/v1.44/containers/my%20box/logs?tail=20&stderr=1");
            assert_eq!(path, "/containers/my%20box/logs");
            assert_eq!(query.get("tail").map(String::as_str), Some("20"));
            assert_eq!(container_action(&path, "logs").as_deref(), Some("my box"));
        }

        #[test]
        fn recognizes_image_paths_with_encoded_ids() {
            assert_eq!(
                image_action("/images/alpine%3A3.20/json", "json").as_deref(),
                Some("alpine:3.20")
            );
            assert_eq!(
                image_action(
                    "/images/sha256:docker.io/library/vpc_network_devcontainer-app:latest/json",
                    "json"
                )
                .as_deref(),
                Some("sha256:docker.io/library/vpc_network_devcontainer-app:latest")
            );
        }

        #[test]
        fn recognizes_native_terminal_paths_with_encoded_ids() {
            assert_eq!(
                cratebay_container_terminal_action(
                    "/cratebay/containers/my%20box/terminal/open",
                    "open"
                )
                .as_deref(),
                Some("my box")
            );
            assert!(cratebay_container_terminal_action(
                "/cratebay/containers/my%20box/terminal/exec",
                "open"
            )
            .is_none());
        }

        #[test]
        fn terminal_session_id_accepts_camel_and_snake_case() {
            assert_eq!(
                terminal_session_id(&json!({ "sessionId": "tty-1" })).as_deref(),
                Some("tty-1")
            );
            assert_eq!(
                terminal_session_id(&json!({ "session_id": "tty-2" })).as_deref(),
                Some("tty-2")
            );
            assert!(terminal_session_id(&json!({ "sessionId": "" })).is_none());
        }

        #[test]
        fn generated_task_ids_are_unique_with_same_timestamp_prefix() {
            let first = unique_task_id("cratebay-exec");
            let second = unique_task_id("cratebay-exec");

            assert!(first.starts_with("cratebay-exec-"));
            assert!(second.starts_with("cratebay-exec-"));
            assert_ne!(first, second);
        }

        #[test]
        fn recognizes_network_paths() {
            assert_eq!(
                network_action("/networks/demo-pod/connect", "connect").as_deref(),
                Some("demo-pod")
            );
            assert_eq!(
                network_id_path("/networks/demo-pod").as_deref(),
                Some("demo-pod")
            );
            assert!(network_id_path("/networks/create").is_none());
        }

        #[test]
        fn pending_container_records_compose_network_aliases() {
            let payload = json!({
                "Image": "mysql:8.0",
                "Labels": {
                    "com.docker.compose.project": "vpc_network_devcontainer",
                    "com.docker.compose.service": "mysql",
                    "com.docker.compose.image": "sha256:docker.io/library/mysql:8.0"
                },
                "HostConfig": {
                    "NetworkMode": "vpc_network_devcontainer_vpc-network"
                },
                "NetworkingConfig": {
                    "EndpointsConfig": {
                        "vpc_network_devcontainer_vpc-network": {
                            "Aliases": [
                                "mysql",
                                "vpc_network_devcontainer-mysql-1",
                                "bad alias"
                            ]
                        }
                    }
                }
            });

            let pending = pending_from_create_payload("vpc_network_devcontainer-mysql-1", &payload);
            let summary = pending_summary_value(&pending);
            let inspect = pending_inspect_value(&pending);
            let record = pending_container_record_value(&pending);
            let restored =
                pending_container_from_record_value(&record).expect("restored pending container");

            assert_eq!(
                pending.network.as_deref(),
                Some("vpc_network_devcontainer_vpc-network")
            );
            assert_eq!(
                pending.labels["com.docker.compose.project"],
                "vpc_network_devcontainer"
            );
            assert_eq!(pending.labels["com.docker.compose.service"], "mysql");
            assert_eq!(summary["ImageID"], "sha256:docker.io/library/mysql:8.0");
            assert_eq!(inspect["Image"], "sha256:docker.io/library/mysql:8.0");
            assert_eq!(summary["Labels"]["com.docker.compose.service"], "mysql");
            assert_eq!(summary["Labels"]["com.cratebay.backend"], "containerd");
            assert_eq!(
                summary["NetworkSettings"]["Networks"]["vpc_network_devcontainer_vpc-network"]
                    ["NetworkID"],
                "vpc_network_devcontainer_vpc-network"
            );
            assert_eq!(
                inspect["Config"]["Labels"]["com.docker.compose.service"],
                "mysql"
            );
            assert_eq!(restored.labels["com.docker.compose.service"], "mysql");
            assert!(pending.aliases.iter().any(|alias| alias == "mysql"));
            assert!(pending
                .aliases
                .iter()
                .any(|alias| alias == "vpc_network_devcontainer-mysql-1"));
            assert!(!pending.aliases.iter().any(|alias| alias == "bad alias"));
        }

        #[test]
        fn pending_container_records_round_trip_registry_mirrors() {
            let (_, payload, _) = native_create_request(&json!({
                "name": "mirror-demo",
                "image": "alpine:3.20",
                "registryMirrors": [
                    " https://mirror.example.com/ ",
                    "http://mirror-2.example.com",
                    " "
                ]
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload("mirror-demo", &payload);
            let record = pending_container_record_value(&pending);
            let restored =
                pending_container_from_record_value(&record).expect("restored pending container");

            assert_eq!(
                pending.registry_mirrors,
                vec!["mirror.example.com", "mirror-2.example.com"]
            );
            assert_eq!(record["registryMirrors"][0], "mirror.example.com");
            assert_eq!(restored.registry_mirrors, pending.registry_mirrors);
        }

        #[test]
        fn parses_cni_ip_addr_output() {
            assert_eq!(
                parse_ip_addr_show(
                    "2: eth0@if6: <BROADCAST> mtu 1500 qdisc noqueue state UP group default \\    inet 10.88.242.4/24 brd 10.88.242.255 scope global eth0\n"
                ),
                Some(("10.88.242.4".to_string(), 24))
            );
        }

        #[test]
        fn frames_docker_raw_stream_output() {
            let response = docker_stream_response(b"out\n".to_vec(), b"err\n".to_vec());
            assert_eq!(response.content_type, "application/vnd.docker.raw-stream");
            assert_eq!(&response.body[..8], &[1, 0, 0, 0, 0, 0, 0, 4]);
            assert_eq!(&response.body[8..12], b"out\n");
            assert_eq!(&response.body[12..20], &[2, 0, 0, 0, 0, 0, 0, 4]);
            assert_eq!(&response.body[20..], b"err\n");
        }

        #[test]
        fn maps_runtime_status_to_docker_state() {
            assert_eq!(docker_state_from_status("Up 3 seconds"), "running");
            assert_eq!(
                docker_state_from_status("Exited (0) 2 minutes ago"),
                "exited"
            );
            assert_eq!(docker_state_from_status("Created"), "created");
        }

        #[test]
        fn native_create_request_maps_agent_shape_to_engine_payload() {
            let (name, payload, auto_start) = native_create_request(&json!({
                "name": "sandbox-demo",
                "image": "alpine:latest",
                "command": "sleep 60",
                "env": ["A=1"],
                "volume": ["/tmp:/tmp:ro"],
                "publish": ["8080:80/tcp"],
                "network": "none",
                "readOnly": true,
                "noPull": true,
                "noStart": true,
                "cpu": 1.5,
                "memory": 256
            }))
            .expect("native create payload");

            assert_eq!(name, "sandbox-demo");
            assert_eq!(payload["Image"], "alpine:latest");
            assert_eq!(payload["Cmd"][0], "/bin/sh");
            assert_eq!(payload["Cmd"][2], "sleep 60");
            assert_eq!(payload["Env"][0], "A=1");
            assert_eq!(payload["HostConfig"]["Binds"][0], "/tmp:/tmp:ro");
            assert_eq!(
                payload["HostConfig"]["PortBindings"]["80/tcp"][0]["HostPort"],
                "8080"
            );
            assert_eq!(payload["HostConfig"]["NetworkMode"], "none");
            assert_eq!(payload["HostConfig"]["ReadonlyRootfs"], true);
            assert_eq!(payload["HostConfig"]["Memory"], 268435456);
            assert_eq!(payload["HostConfig"]["NanoCpus"], 1500000000);
            assert_eq!(payload["CrateBayNoPull"], true);
            assert!(!auto_start);
        }

        #[test]
        fn merged_container_command_uses_image_entrypoint_with_create_cmd() {
            let image_config = json!({
                "config": {
                    "Entrypoint": ["buildkitd"],
                    "Cmd": ["--debug"]
                }
            });
            let payload = json!({
                "Image": "moby/buildkit:buildx-stable-1",
                "Cmd": ["--allow-insecure-entitlement=network.host"]
            });

            assert_eq!(
                merged_container_command(&payload, Some(&image_config)),
                vec![
                    "buildkitd".to_string(),
                    "--allow-insecure-entitlement=network.host".to_string()
                ]
            );
        }

        #[test]
        fn merged_container_command_treats_null_entrypoint_as_image_default() {
            let image_config = json!({
                "config": {
                    "Entrypoint": ["/usr/bin/buildkitd-entrypoint"]
                }
            });
            let payload = json!({
                "Image": "moby/buildkit:buildx-stable-1",
                "Entrypoint": null,
                "Cmd": ["--allow-insecure-entitlement=network.host"]
            });

            assert_eq!(
                merged_container_command(&payload, Some(&image_config)),
                vec![
                    "/usr/bin/buildkitd-entrypoint".to_string(),
                    "--allow-insecure-entitlement=network.host".to_string()
                ]
            );
        }

        #[test]
        fn merged_container_command_uses_image_defaults_when_create_payload_omits_cmd() {
            let image_config = json!({
                "config": {
                    "Entrypoint": ["/usr/bin/demo"],
                    "Cmd": ["serve"]
                }
            });
            let payload = json!({ "Image": "demo:latest" });

            assert_eq!(
                merged_container_command(&payload, Some(&image_config)),
                vec!["/usr/bin/demo".to_string(), "serve".to_string()]
            );
        }

        #[test]
        fn merged_container_command_allows_explicit_entrypoint_override() {
            let image_config = json!({
                "config": {
                    "Entrypoint": ["ignored"],
                    "Cmd": ["ignored-cmd"]
                }
            });
            let payload = json!({
                "Image": "demo:latest",
                "Entrypoint": ["/bin/sh", "-c"],
                "Cmd": ["echo ok"]
            });

            assert_eq!(
                merged_container_command(&payload, Some(&image_config)),
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "echo ok".to_string()
                ]
            );
        }

        #[test]
        fn native_create_request_maps_pod_to_network_mode() {
            let (_, payload, _) = native_create_request(&json!({
                "name": "sandbox-demo",
                "image": "alpine:latest",
                "command": "sleep 60",
                "pod": "demo-pod",
                "network": null,
            }))
            .expect("native create payload");

            assert_eq!(payload["HostConfig"]["NetworkMode"], "demo-pod");
        }

        #[test]
        fn native_create_payload_builds_containerd_pending_container() {
            let (name, payload, auto_start) = native_create_request(&json!({
                "name": "sandbox-demo",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "env": ["A=1"],
                "network": "pod-demo",
                "workingDir": "/workspace",
            }))
            .expect("native create payload");

            let pending = pending_from_create_payload(&name, &payload);
            let summary = pending_summary_value(&pending);
            let inspect = pending_inspect_value(&pending);

            assert!(auto_start);
            assert!(looks_like_docker_container_id(&pending.id));
            assert_eq!(pending.name, "sandbox-demo");
            assert_eq!(pending.runtime_id, "sandbox-demo");
            assert_eq!(pending.image, "alpine:3.20");
            assert_eq!(pending.command, vec!["/bin/sh", "-c", "sleep 60"]);
            assert_eq!(pending.network.as_deref(), Some("pod-demo"));
            assert!(!pending.no_pull);
            assert_eq!(summary["State"], "created");
            assert_eq!(summary["Labels"]["com.cratebay.backend"], "containerd");
            assert_eq!(inspect["Config"]["Image"], "alpine:3.20");
            assert_eq!(inspect["HostConfig"]["NetworkMode"], "pod-demo");
        }

        #[test]
        fn privileged_host_config_maps_to_ctr_run_flag() {
            let mut query = HashMap::new();
            query.insert("name".to_string(), "privileged-demo".to_string());
            let state = test_state_without_external_frontend();
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "moby/buildkit:buildx-stable-1",
                    "HostConfig": {
                        "Privileged": true
                    }
                }"#,
            );
            let pending = pending_container(&state, "privileged-demo").expect("pending container");
            let run_args = build_ctr_run_args(&pending, "moby/buildkit:buildx-stable-1");

            assert_eq!(create.status, 201);
            assert!(pending.privileged);
            assert!(run_args.iter().any(|arg| arg == "--privileged"));
            assert_eq!(
                pending_inspect_value(&pending)["HostConfig"]["Privileged"],
                true
            );
        }

        #[test]
        fn container_system_mounts_inject_dns_and_hosts_files() {
            let (_, payload, _) = native_create_request(&json!({
                "name": "system-files-demo",
                "image": "alpine:3.20",
                "command": ["true"],
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload("system-files-demo", &payload);

            let mounts = ensure_container_system_mounts(&pending).expect("system mounts");
            let resolv = mounts
                .iter()
                .find(|mount| mount.target == "/etc/resolv.conf")
                .expect("resolv mount");
            let hosts = mounts
                .iter()
                .find(|mount| mount.target == "/etc/hosts")
                .expect("hosts mount");

            assert!(resolv.readonly);
            assert!(hosts.readonly);
            assert!(fs::read_to_string(&resolv.source)
                .expect("resolv file")
                .contains("nameserver"));
            assert!(fs::read_to_string(&hosts.source)
                .expect("hosts file")
                .contains("system-files-demo"));
        }

        #[test]
        fn cgroup_system_mount_matches_container_privilege() {
            let (_, payload, _) = native_create_request(&json!({
                "name": "cgroup-files-demo",
                "image": "alpine:3.20",
                "command": ["true"],
            }))
            .expect("native create payload");
            let mut pending = pending_from_create_payload("cgroup-files-demo", &payload);

            let readonly = container_cgroup_mount(&pending, true).expect("cgroup mount");
            assert_eq!(readonly.target, "/sys/fs/cgroup");
            assert!(readonly.readonly);

            pending.privileged = true;
            let writable = container_cgroup_mount(&pending, true).expect("cgroup mount");
            assert!(!writable.readonly);

            pending.mounts.push(CtrMount {
                source: "/custom/cgroup".to_string(),
                target: "/sys/fs/cgroup".to_string(),
                readonly: false,
            });
            assert!(container_cgroup_mount(&pending, true).is_none());
        }

        #[test]
        fn native_wait_container_exposes_cratebay_contract() {
            let state = test_state_without_external_frontend();
            let (name, payload, _) = native_create_request(&json!({
                "name": "wait-demo",
                "image": "alpine:3.20",
                "command": ["true"],
            }))
            .expect("native create payload");
            let mut pending = pending_from_create_payload(&name, &payload);
            pending.started_with_ctr = true;
            pending.exit_code = Some(7);
            let id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &id, &pending_name, pending)
                .expect("store pending container");

            let response = native_wait_container(&state, "wait-demo".to_string(), br#"{}"#);
            let body: serde_json::Value =
                serde_json::from_slice(&response.body).expect("wait response json");

            assert_eq!(response.status, 200);
            assert_eq!(body["api"], "cratebay.container.wait.v1");
            assert_eq!(body["id"], "wait-demo");
            assert_eq!(body["backend"], "containerd");
            assert_eq!(body["exitCode"], 7);
            assert_eq!(body["timedOut"], false);
        }

        #[test]
        fn compat_create_container_stores_pending_without_external_frontend() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "compat-demo".to_string());

            let response = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"],
                    "Env": ["A=1"],
                    "WorkingDir": "/workspace",
                    "HostConfig": {
                        "NetworkMode": "demo-pod",
                        "Binds": ["workspace-cache:/workspace:ro"],
                        "PortBindings": {
                            "80/tcp": [{ "HostPort": "8080" }]
                        }
                    }
                }"#,
            );
            let body: serde_json::Value =
                serde_json::from_slice(&response.body).expect("create response json");
            let pending = pending_container(&state, "compat-demo").expect("pending container");

            assert_eq!(response.status, 201);
            let created_id = body["Id"].as_str().expect("docker id");
            assert!(looks_like_docker_container_id(created_id));
            assert_eq!(body["CrateBay"]["backend"], "containerd-pending");
            assert_eq!(pending.id, created_id);
            assert_eq!(pending.name, "compat-demo");
            assert_eq!(pending.runtime_id, "compat-demo");
            assert_eq!(pending.image, "alpine:3.20");
            assert_eq!(pending.command, vec!["sleep", "60"]);
            assert_eq!(pending.env, vec!["A=1"]);
            assert_eq!(pending.working_dir.as_deref(), Some("/workspace"));
            assert_eq!(pending.network.as_deref(), Some("demo-pod"));
            assert_eq!(pending.mounts.len(), 1);
            assert_eq!(pending.ports.len(), 1);
            assert_eq!(pending.ports[0].host_port, 8080);
            assert_eq!(pending.ports[0].container_port, 80);
        }

        #[test]
        fn compat_list_and_inspect_include_pending_without_external_frontend() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "compat-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"]
                }"#,
            );
            assert_eq!(create.status, 201);

            let listed = list_containers(&state).expect("container list");
            let inspected = inspect_container(&state, "compat-demo".to_string());
            let inspect_body: serde_json::Value =
                serde_json::from_slice(&inspected.body).expect("inspect response json");

            let listed_id = listed[0]["Id"].as_str().expect("listed id");
            assert!(looks_like_docker_container_id(listed_id));
            assert_eq!(listed[0]["State"], "created");
            assert!(listed[0]["Created"].as_i64().unwrap_or_default() > 0);
            assert_eq!(listed[0]["Labels"]["com.cratebay.backend"], "containerd");
            assert_eq!(inspected.status, 200);
            assert_eq!(inspect_body["Id"], listed_id);
            assert!(inspect_body["Created"]
                .as_str()
                .filter(|created| created.contains('T') && created.ends_with('Z'))
                .is_some());
            assert_eq!(inspect_body["Config"]["Image"], "alpine:3.20");
            assert_eq!(inspect_body["State"]["Status"], "created");
        }

        #[test]
        fn put_archive_stages_files_and_mounts_pending_container() {
            let state = test_state_without_external_frontend();
            let mut create_query = HashMap::new();
            create_query.insert("name".to_string(), "archive-demo".to_string());
            let create = create_container(
                &state,
                &create_query,
                br#"{
                    "Image": "moby/buildkit:buildx-stable-1"
                }"#,
            );
            assert_eq!(create.status, 201);

            let mut archive_bytes = Vec::new();
            {
                let mut builder = tar::Builder::new(&mut archive_bytes);
                let mut header = tar::Header::new_gnu();
                let contents = b"[worker.oci]\n";
                header.set_path("buildkit/buildkitd.toml").unwrap();
                header.set_size(contents.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append(&header, Cursor::new(contents.as_slice()))
                    .unwrap();
                builder.finish().unwrap();
            }
            let mut archive_query = HashMap::new();
            archive_query.insert("path".to_string(), "/etc".to_string());

            let response = put_container_archive(
                &state,
                "archive-demo".to_string(),
                &archive_query,
                &archive_bytes,
            );
            let pending = pending_container(&state, "archive-demo").expect("pending container");
            let stage = pending_archive_stage_path("archive-demo", "/etc");
            let archive_mounts = pending_archive_mounts(&stage, "/etc");

            assert_eq!(response.status, 200);
            assert_eq!(archive_target_path(&archive_query).as_deref(), Some("/etc"));
            assert_eq!(
                fs::read_to_string(stage.join("buildkit").join("buildkitd.toml")).unwrap(),
                "[worker.oci]\n"
            );
            assert_eq!(archive_mounts.len(), 1);
            assert_eq!(archive_mounts[0].target, "/etc/buildkit");
            assert!(pending.mounts.iter().any(|mount| {
                mount.target == "/etc/buildkit"
                    && mount.source == stage.join("buildkit").display().to_string()
            }));
            assert!(!pending.mounts.iter().any(|mount| mount.target == "/etc"));
        }

        #[test]
        fn broad_archive_target_with_no_entries_does_not_overlay_system_dir() {
            let stage = std::env::temp_dir()
                .join("cratebay-engine-adapter-tests")
                .join(super::test_thread_id())
                .join("empty-archive-stage");
            std::fs::create_dir_all(&stage).expect("stage dir");

            assert!(pending_archive_mounts(&stage, "/etc").is_empty());
        }

        #[test]
        fn pending_container_registry_round_trips_runtime_state() {
            let (name, payload, _) = native_create_request(&json!({
                "name": "registry-demo",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "env": ["A=1"],
                "network": "demo-pod",
                "volume": ["workspace-cache:/workspace:ro"],
                "publish": ["8080:80/tcp"],
                "noPull": true,
            }))
            .expect("native create payload");
            let mut pending = pending_from_create_payload(&name, &payload);
            pending.started_with_ctr = true;
            pending.exit_code = Some(0);

            let value = pending_container_record_value(&pending);
            let restored =
                pending_container_from_record_value(&value).expect("restored pending container");

            assert_eq!(value["api"], "cratebay.container.record.v1");
            assert_eq!(value["managedBy"], "cratebay");
            assert_eq!(restored.id, pending.id);
            assert_eq!(restored.name, "registry-demo");
            assert_eq!(restored.runtime_id, "registry-demo");
            assert_eq!(restored.image, "alpine:3.20");
            assert_eq!(restored.network.as_deref(), Some("demo-pod"));
            assert_eq!(restored.mounts.len(), 1);
            assert_eq!(restored.ports.len(), 1);
            assert!(restored.no_pull);
            assert!(restored.started_with_ctr);
            assert_eq!(restored.exit_code, Some(0));
        }

        #[test]
        fn compat_list_loads_pending_container_registry_after_restart() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "persisted-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"]
                }"#,
            );
            assert_eq!(create.status, 201);

            let restarted_state = test_state_without_external_frontend_preserving_registry();
            let listed = list_containers(&restarted_state).expect("container list");
            let inspected = inspect_container(&restarted_state, "persisted-demo".to_string());

            assert!(listed[0]["Id"]
                .as_str()
                .filter(|id| looks_like_docker_container_id(id))
                .is_some());
            assert_eq!(listed[0]["State"], "created");
            assert_eq!(inspected.status, 200);
            assert!(pending_container(&restarted_state, "persisted-demo").is_some());
        }

        #[test]
        fn compat_rename_exited_pending_moves_registry_name() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "rename-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"]
                }"#,
            );
            assert_eq!(create.status, 201);
            let mut pending = pending_container(&state, "rename-demo").expect("pending container");
            let docker_id = pending.id.clone();
            pending.started_with_ctr = true;
            pending.exit_code = Some(0);
            let pending_name = pending.name.clone();
            store_pending_container(&state, &docker_id, &pending_name, pending)
                .expect("store exited pending");

            let mut rename_query = HashMap::new();
            rename_query.insert("name".to_string(), "project__rename-demo".to_string());
            let renamed = rename_container(&state, "rename-demo".to_string(), &rename_query);
            let by_id = pending_container(&state, &docker_id).expect("renamed by id");
            let by_name =
                pending_container(&state, "project__rename-demo").expect("renamed by name");

            assert_eq!(renamed.status, 204);
            assert_eq!(by_id.id, docker_id);
            assert_eq!(by_id.name, "project__rename-demo");
            assert_eq!(by_name.id, by_id.id);
            assert!(std::fs::metadata(pending_container_registry_path("rename-demo")).is_err());
            assert!(
                std::fs::metadata(pending_container_registry_path("project__rename-demo")).is_ok()
            );
        }

        #[test]
        fn compat_rename_running_pending_keeps_containerd_task_id() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "project__project-service-1".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"],
                    "Labels": {
                        "com.docker.compose.replace": "service-1"
                    }
                }"#,
            );
            assert_eq!(create.status, 201);
            let created_body: serde_json::Value =
                serde_json::from_slice(&create.body).expect("create response json");
            let docker_id = created_body["Id"].as_str().expect("docker id").to_string();
            mark_pending_started_with_ctr(
                &state,
                "project__project-service-1",
                "project__project-service-1",
            );

            let mut rename_query = HashMap::new();
            rename_query.insert("name".to_string(), "project-service-1".to_string());
            let renamed = rename_container(
                &state,
                "project__project-service-1".to_string(),
                &rename_query,
            );
            let pending =
                pending_container(&state, "project-service-1").expect("renamed by display name");

            assert_eq!(renamed.status, 204);
            assert_eq!(pending.id, docker_id);
            assert_eq!(pending.name, "project-service-1");
            assert_eq!(containerd_task_name(&pending), "project__project-service-1");
            assert!(pending.labels.get("com.docker.compose.replace").is_none());
            assert!(pending_container(&state, "project__project-service-1").is_none());
        }

        #[test]
        fn compat_remove_unstarted_pending_deletes_registry_without_external_frontend() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "remove-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"]
                }"#,
            );
            assert_eq!(create.status, 201);
            assert!(pending_container(&state, "remove-demo").is_some());

            let remove_query = HashMap::new();
            let removed = remove_container(&state, "remove-demo".to_string(), &remove_query);

            assert_eq!(removed.status, 204);
            assert!(pending_container(&state, "remove-demo").is_none());
        }

        #[test]
        fn compat_remove_by_short_id_deletes_pending() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "remove-short-id-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"]
                }"#,
            );
            let created_body: serde_json::Value =
                serde_json::from_slice(&create.body).expect("create response json");
            let full_id = created_body["Id"].as_str().expect("docker id");
            let short_id = full_id.chars().take(12).collect::<String>();

            let remove_query = HashMap::new();
            let removed = remove_container(&state, short_id, &remove_query);

            assert_eq!(removed.status, 204);
            assert!(pending_container(&state, full_id).is_none());
            assert!(pending_container(&state, "remove-short-id-demo").is_none());
        }

        #[test]
        fn compat_remove_running_pending_requires_force() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "remove-running-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"]
                }"#,
            );
            assert_eq!(create.status, 201);
            mark_pending_started_with_ctr(&state, "remove-running-demo", "remove-running-demo");

            let remove_query = HashMap::new();
            let removed =
                remove_container(&state, "remove-running-demo".to_string(), &remove_query);

            assert_eq!(removed.status, 409);
            assert!(pending_container(&state, "remove-running-demo").is_some());
        }

        #[test]
        fn compat_force_remove_running_pending_deletes_registry() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "force-remove-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"]
                }"#,
            );
            assert_eq!(create.status, 201);
            mark_pending_started_with_ctr(&state, "force-remove-demo", "force-remove-demo");

            let mut remove_query = HashMap::new();
            remove_query.insert("force".to_string(), "true".to_string());
            let removed = remove_container(&state, "force-remove-demo".to_string(), &remove_query);

            assert_eq!(removed.status, 204);
            assert!(pending_container(&state, "force-remove-demo").is_none());
        }

        #[test]
        fn pending_container_resolves_compose_display_name_alias() {
            let state = test_state_without_external_frontend();
            let mut pending = pending_from_create_payload(
                "project__project-service-1",
                &json!({
                    "Image": "alpine:3.20",
                    "Cmd": ["true"],
                    "Labels": {
                        "com.docker.compose.project": "project",
                        "com.docker.compose.service": "service"
                    }
                }),
            );
            pending.name = "project-service-1".to_string();
            pending.aliases.push("project-service-1".to_string());
            let id = pending.id.clone();
            let name = pending.name.clone();
            store_pending_container(&state, &id, &name, pending).expect("store pending");

            let by_display_name =
                pending_container(&state, "project-service-1").expect("display name resolves");
            let by_docker_name =
                pending_container(&state, "/project-service-1").expect("docker name resolves");

            assert_eq!(by_display_name.id, id);
            assert_eq!(by_display_name.runtime_id, "project__project-service-1");
            assert_eq!(by_docker_name.name, "project-service-1");
        }

        #[test]
        fn compat_remove_by_compose_display_name_deletes_renamed_pending() {
            let state = test_state_without_external_frontend();
            let mut pending = pending_from_create_payload(
                "project__project-db-1",
                &json!({
                    "Image": "mysql:8.0",
                    "Cmd": ["true"],
                    "Labels": {
                        "com.docker.compose.project": "project",
                        "com.docker.compose.service": "db"
                    }
                }),
            );
            pending.name = "project-db-1".to_string();
            pending.aliases.push("project-db-1".to_string());
            let id = pending.id.clone();
            let name = pending.name.clone();
            store_pending_container(&state, &id, &name, pending).expect("store pending");

            let remove_query = HashMap::new();
            let removed = remove_container(&state, "project-db-1".to_string(), &remove_query);

            assert_eq!(removed.status, 204);
            assert!(pending_container(&state, "project-db-1").is_none());
            assert!(pending_container(&state, "project__project-db-1").is_none());
        }

        #[test]
        fn compat_remove_old_compose_container_keeps_replacement_alias() {
            let state = test_state_without_external_frontend();
            let mut old_query = HashMap::new();
            old_query.insert("name".to_string(), "project-db-1".to_string());
            let old = create_container(
                &state,
                &old_query,
                br#"{
                    "Image": "mysql:8.0",
                    "NetworkingConfig": {
                        "EndpointsConfig": {
                            "net": {
                                "Aliases": ["project-db-1", "db"]
                            }
                        }
                    }
                }"#,
            );
            assert_eq!(old.status, 201);

            let mut replacement_query = HashMap::new();
            replacement_query.insert("name".to_string(), "project__project-db-1".to_string());
            let replacement = create_container(
                &state,
                &replacement_query,
                br#"{
                    "Image": "mysql:8.0",
                    "NetworkingConfig": {
                        "EndpointsConfig": {
                            "net": {
                                "Aliases": ["project-db-1", "project__project-db-1", "db"]
                            }
                        }
                    }
                }"#,
            );
            assert_eq!(replacement.status, 201);

            let remove_query = HashMap::new();
            let removed = remove_container(&state, "project-db-1".to_string(), &remove_query);
            let mut rename_query = HashMap::new();
            rename_query.insert("name".to_string(), "project-db-1".to_string());
            let renamed =
                rename_container(&state, "project__project-db-1".to_string(), &rename_query);
            let pending = pending_container(&state, "project-db-1").expect("replacement renamed");

            assert_eq!(removed.status, 204);
            assert_eq!(renamed.status, 204);
            assert_eq!(pending.name, "project-db-1");
            assert_eq!(pending.runtime_id, "project__project-db-1");
            assert!(pending_container(&state, "project__project-db-1").is_none());
        }

        #[test]
        fn compat_restart_reset_clears_exited_runtime_state_and_netns() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "restart-reset-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"]
                }"#,
            );
            assert_eq!(create.status, 201);

            let mut pending =
                pending_container(&state, "restart-reset-demo").expect("pending container");
            pending.network = Some("restart-net".to_string());
            pending.netns_name = Some("cb-restart-net-restart-reset-demo".to_string());
            pending.netns_path = Some(PathBuf::from(
                "/var/run/netns/cb-restart-net-restart-reset-demo",
            ));
            pending.started_with_ctr = true;
            pending.exit_code = Some(1);
            store_pending_container(
                &state,
                "restart-reset-demo",
                "restart-reset-demo",
                pending.clone(),
            )
            .expect("store started pending");

            let reset = reset_pending_runtime_state_for_start(&state, pending);
            let restored =
                pending_container(&state, "restart-reset-demo").expect("restored pending");

            assert!(!reset.started_with_ctr);
            assert_eq!(reset.exit_code, None);
            assert_eq!(reset.netns_name, None);
            assert_eq!(reset.netns_path, None);
            assert!(!restored.started_with_ctr);
            assert_eq!(restored.exit_code, None);
            assert_eq!(restored.netns_name, None);
            assert_eq!(restored.netns_path, None);
        }

        #[test]
        fn containerd_cleanup_removes_tasks_containers_and_snapshots() {
            assert_eq!(
                containerd_cleanup_args("cleanup-demo"),
                vec![
                    vec!["tasks", "kill", "--signal", "KILL", "cleanup-demo"],
                    vec!["tasks", "rm", "--force", "cleanup-demo"],
                    vec!["containers", "rm", "cleanup-demo"],
                    vec!["snapshots", "rm", "cleanup-demo"],
                ]
            );
        }

        #[test]
        fn compat_stop_and_wait_unstarted_pending_do_not_call_external_frontend() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "created-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"]
                }"#,
            );
            assert_eq!(create.status, 201);

            let stop_query = HashMap::new();
            let stopped = stop_container(&state, "created-demo".to_string(), &stop_query);
            let waited = wait_container(&state, "created-demo".to_string());
            let waited_body: serde_json::Value =
                serde_json::from_slice(&waited.body).expect("wait response");

            assert_eq!(stopped.status, 204);
            assert_eq!(waited.status, 200);
            assert_eq!(waited_body["StatusCode"], 0);
        }

        #[test]
        fn network_connect_updates_unstarted_pending_container_registry() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "network-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"]
                }"#,
            );
            assert_eq!(create.status, 201);
            let (_, payload) = native_network_create_request(&json!({
                "name": "demo-pod",
                "driver": "bridge"
            }))
            .expect("network payload");
            create_managed_network(&state.config, &payload).expect("managed network");

            let connected = connect_network(
                &state,
                "demo-pod".to_string(),
                br#"{ "Container": "network-demo" }"#,
            );
            let pending = pending_container(&state, "network-demo").expect("pending container");
            let restarted = test_state_without_external_frontend_preserving_registry();
            let restored =
                pending_container(&restarted, "network-demo").expect("restored pending container");

            assert_eq!(connected.status, 200);
            assert_eq!(pending.network.as_deref(), Some("demo-pod"));
            assert_eq!(restored.network.as_deref(), Some("demo-pod"));
        }

        #[test]
        fn network_disconnect_updates_unstarted_pending_container_registry() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "network-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["true"],
                    "HostConfig": { "NetworkMode": "demo-pod" }
                }"#,
            );
            assert_eq!(create.status, 201);

            let disconnected = disconnect_network(
                &state,
                "demo-pod".to_string(),
                br#"{ "Container": "network-demo" }"#,
            );
            let pending = pending_container(&state, "network-demo").expect("pending container");

            assert_eq!(disconnected.status, 200);
            assert_eq!(pending.network, None);
        }

        #[test]
        fn network_connect_running_pending_requires_managed_network() {
            let state = test_state_without_external_frontend();
            let mut query = HashMap::new();
            query.insert("name".to_string(), "running-network-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"]
                }"#,
            );
            assert_eq!(create.status, 201);
            mark_pending_started_with_ctr(&state, "running-network-demo", "running-network-demo");

            let connected = connect_network(
                &state,
                "demo-pod".to_string(),
                br#"{ "Container": "running-network-demo" }"#,
            );
            let pending =
                pending_container(&state, "running-network-demo").expect("pending container");

            assert_eq!(connected.status, 404);
            assert_eq!(pending.network, None);
        }

        #[test]
        fn running_network_attachment_reuses_stored_namespace_after_detach() {
            let (_, payload, _) = native_create_request(&json!({
                "name": "running-network-demo",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "network": "demo-pod",
            }))
            .expect("native create payload");
            let mut pending = pending_from_create_payload("running-network-demo", &payload);
            pending.started_with_ctr = true;
            pending.network = None;
            pending.netns_name = Some("cb-demo-pod-running-network-demo".to_string());
            pending.netns_path = Some(PathBuf::from(
                "/var/run/netns/cb-demo-pod-running-network-demo",
            ));

            let attachment =
                running_network_attachment(&pending, "demo-pod").expect("stored netns");

            assert_eq!(attachment.network, "demo-pod");
            assert_eq!(attachment.netns_name, "cb-demo-pod-running-network-demo");
            assert_eq!(
                attachment.netns_path,
                PathBuf::from("/var/run/netns/cb-demo-pod-running-network-demo")
            );
        }

        #[test]
        fn task_list_parser_matches_container_name_or_id() {
            let output = b"TASK PID STATUS\nsandbox-a 101 RUNNING\nregistry-demo\n";

            assert!(task_list_contains(output, &["registry-demo", "other-id"]));
            assert!(task_list_contains(output, &["other-name", "sandbox-a"]));
            assert!(!task_list_contains(output, &["missing"]));
        }

        #[test]
        fn parses_containerd_task_metrics_into_runtime_stats() {
            let metrics = parse_containerd_metrics(
                br#"
                cpuacct.usage 123456789
                memory.usage_in_bytes 67108864
                memory.limit_in_bytes 268435456
                "#,
            );
            let cgroup_v2 = parse_containerd_metrics(
                br#"
                usage_usec 98765
                memory.current 33554432
                memory.max 134217728
                "#,
            );

            assert_eq!(metrics.cpu_total, 123456789);
            assert_eq!(metrics.memory_usage, 67108864);
            assert_eq!(metrics.memory_limit, 268435456);
            assert_eq!(cgroup_v2.cpu_total, 98765000);
            assert_eq!(cgroup_v2.memory_usage, 33554432);
            assert_eq!(cgroup_v2.memory_limit, 134217728);
        }

        #[test]
        fn docker_stats_value_uses_previous_containerd_snapshot() {
            let state = test_state_without_external_frontend();
            let (name, payload, _) = native_create_request(&json!({
                "name": "stats-demo",
                "image": "alpine:3.20",
                "command": "sleep 60",
            }))
            .expect("native create payload");
            let mut pending = pending_from_create_payload(&name, &payload);
            pending.started_with_ctr = true;

            let first = docker_stats_value(
                &state,
                &pending,
                ContainerRuntimeMetrics {
                    cpu_total: 1000,
                    memory_usage: 1024,
                    memory_limit: 4096,
                },
                "containerd",
            );
            let second = docker_stats_value(
                &state,
                &pending,
                ContainerRuntimeMetrics {
                    cpu_total: 2500,
                    memory_usage: 2048,
                    memory_limit: 4096,
                },
                "containerd",
            );

            assert_eq!(first["cpu_stats"]["cpu_usage"]["total_usage"], 1000);
            assert_eq!(first["precpu_stats"]["cpu_usage"]["total_usage"], 0);
            assert_eq!(second["cpu_stats"]["cpu_usage"]["total_usage"], 2500);
            assert_eq!(second["precpu_stats"]["cpu_usage"]["total_usage"], 1000);
            assert_eq!(second["memory_stats"]["usage"], 2048);
            assert_eq!(second["memory_stats"]["limit"], 4096);
            assert_eq!(second["CrateBay"]["backend"], "containerd");
        }

        #[test]
        fn native_stats_value_exposes_cratebay_contract() {
            let state = test_state_without_external_frontend();
            let (name, payload, _) = native_create_request(&json!({
                "name": "native-stats-demo",
                "image": "alpine:3.20",
                "command": "sleep 60",
            }))
            .expect("native create payload");
            let mut pending = pending_from_create_payload(&name, &payload);
            pending.started_with_ctr = true;

            let first = native_stats_value(
                &state,
                &pending,
                ContainerRuntimeMetrics {
                    cpu_total: 1000,
                    memory_usage: 1048576,
                    memory_limit: 4194304,
                },
                "containerd",
            );
            let second = native_stats_value(
                &state,
                &pending,
                ContainerRuntimeMetrics {
                    cpu_total: 4000,
                    memory_usage: 2097152,
                    memory_limit: 4194304,
                },
                "containerd",
            );

            assert_eq!(first["api"], "cratebay.container.stats.v1");
            assert_eq!(first["id"], pending.id);
            assert_eq!(first["managedBy"], "cratebay");
            assert_eq!(first["backend"], "containerd");
            assert_eq!(second["cpu"]["totalUsage"], 4000);
            assert_eq!(second["cpu"]["previousTotalUsage"], 1000);
            assert_eq!(second["memory"]["usedMb"], 2.0);
            assert_eq!(second["memory"]["limitMb"], 4.0);
            assert_eq!(second["memory"]["percent"], 50.0);
        }

        #[test]
        fn container_log_reader_appends_output_without_waiting_for_exit_collection() {
            let path = std::env::temp_dir().join(format!(
                "cratebay-engine-log-{:?}.log",
                std::thread::current().id()
            ));
            let _ = std::fs::remove_file(&path);

            let handle =
                spawn_container_log_reader(Cursor::new(b"hello\nworld\n".to_vec()), path.clone());
            handle.join().expect("log reader thread");
            append_log_bytes(&path, b"again\n").expect("append more logs");

            assert_eq!(
                std::fs::read_to_string(&path).expect("container log"),
                "hello\nworld\nagain\n"
            );
            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn pending_log_tail_limits_returned_lines() {
            assert_eq!(
                String::from_utf8(apply_log_tail(
                    b"one\ntwo\nthree\n".to_vec(),
                    Some(&"2".to_string())
                ))
                .expect("tail utf8"),
                "two\nthree\n"
            );
            assert_eq!(
                apply_log_tail(b"one\ntwo\n".to_vec(), Some(&"all".to_string())),
                b"one\ntwo\n"
            );
        }

        #[test]
        fn attach_query_flags_follow_docker_defaults() {
            let defaults = HashMap::new();
            assert_eq!(attach_stream_flags(&defaults), (true, true));
            assert!(query_bool_or(&defaults, "stream", true));

            let explicit = HashMap::from([
                ("stdout".to_string(), "0".to_string()),
                ("stderr".to_string(), "true".to_string()),
                ("stream".to_string(), "false".to_string()),
            ]);
            assert_eq!(attach_stream_flags(&explicit), (false, true));
            assert!(!query_bool_or(&explicit, "stream", true));
        }

        #[test]
        fn bind_specs_map_to_ctr_mounts() {
            let host_mount =
                parse_bind_mount_spec("/tmp/work:/workspace:ro").expect("absolute bind mount");
            assert_eq!(host_mount.source, "/tmp/work");
            assert_eq!(host_mount.target, "/workspace");
            assert!(host_mount.readonly);
            assert_eq!(
                ctr_mount_arg(&host_mount),
                "type=bind,src=/tmp/work,dst=/workspace,options=rbind:ro"
            );

            let named_mount =
                parse_bind_mount_spec("workspace-cache:/cache").expect("named volume bind mount");
            let expected_source = super::volume_data_path("workspace-cache")
                .display()
                .to_string();
            assert_eq!(named_mount.source, expected_source);
            assert_eq!(
                ctr_mount_arg(&named_mount),
                format!("type=bind,src={expected_source},dst=/cache,options=rbind:rw")
            );
        }

        #[test]
        fn native_pending_container_mounts_are_passed_to_ctr_run() {
            let (name, payload, _) = native_create_request(&json!({
                "name": "sandbox-demo",
                "image": "alpine:3.20",
                "command": "true",
                "volume": ["workspace-cache:/workspace:ro"],
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &payload);
            let args = build_ctr_run_args(&pending, "docker.io/library/alpine:3.20");
            let inspect = pending_inspect_value(&pending);
            let expected_source = super::volume_data_path("workspace-cache")
                .display()
                .to_string();

            assert_eq!(pending.mounts.len(), 1);
            assert!(args.contains(&"--mount".to_string()));
            assert!(args.contains(&format!(
                "type=bind,src={expected_source},dst=/workspace,options=rbind:ro"
            )));
            assert_eq!(inspect["Mounts"][0]["Source"], expected_source);
            assert_eq!(inspect["Mounts"][0]["Type"], "volume");
            assert_eq!(inspect["Mounts"][0]["Name"], "workspace-cache");
            assert_eq!(inspect["Mounts"][0]["Destination"], "/workspace");
            assert_eq!(inspect["Mounts"][0]["RW"], false);
        }

        #[test]
        fn ctr_run_args_use_guest_host_network_by_default() {
            let (name, payload, _) = native_create_request(&json!({
                "name": "network-default-demo",
                "image": "alpine:3.20",
                "command": "true",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &payload);
            let args = build_ctr_run_args(&pending, "docker.io/library/alpine:3.20");

            assert!(args.contains(&"--with-ns".to_string()));
            assert!(args.contains(&"network:/proc/1/ns/net".to_string()));
        }

        #[test]
        fn ctr_run_args_use_cratebay_runc_wrapper_by_default() {
            let (name, payload, _) = native_create_request(&json!({
                "name": "wrapper-default-demo",
                "image": "alpine:3.20",
                "command": "true",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &payload);
            let args = build_ctr_run_args(&pending, "docker.io/library/alpine:3.20");

            assert!(args.contains(&"--runc-binary".to_string()));
            assert!(args.contains(&"/usr/local/bin/cratebay-engine-adapter".to_string()));
        }

        #[test]
        fn cmdline_proxy_value_is_parsed_and_normalized() {
            let cmdline = "console=hvc0 panic=1 cratebay_http_proxy=192.168.64.1:3128 cratebay_runtime_engine=containerd";

            assert_eq!(
                cmdline_value(cmdline, "cratebay_http_proxy").as_deref(),
                Some("192.168.64.1:3128")
            );
            assert_eq!(
                normalize_http_proxy_url("192.168.64.1:3128").as_deref(),
                Some("http://192.168.64.1:3128")
            );
            assert_eq!(
                normalize_http_proxy_url("http://192.168.64.1:3128/").as_deref(),
                Some("http://192.168.64.1:3128")
            );
        }

        #[test]
        fn ctr_run_envs_inject_proxy_without_overriding_explicit_values() {
            let envs = ctr_run_envs(
                &[
                    "A=1".to_string(),
                    "HTTP_PROXY=http://custom.proxy:8080".to_string(),
                    "no_proxy=internal.local".to_string(),
                ],
                Some("192.168.64.1:3128"),
            );

            assert_eq!(
                envs.iter()
                    .filter(|env| env.starts_with("HTTP_PROXY="))
                    .count(),
                1
            );
            assert!(envs.contains(&"HTTP_PROXY=http://custom.proxy:8080".to_string()));
            assert!(envs.contains(&"HTTPS_PROXY=http://192.168.64.1:3128".to_string()));
            assert!(envs.contains(&"http_proxy=http://192.168.64.1:3128".to_string()));
            assert!(envs.contains(&"https_proxy=http://192.168.64.1:3128".to_string()));
            assert!(envs.contains(&"NO_PROXY=localhost,127.0.0.1,::1".to_string()));
            assert!(envs.contains(&"no_proxy=internal.local".to_string()));
            assert_eq!(
                envs.iter()
                    .filter(|env| env.starts_with("no_proxy="))
                    .count(),
                1
            );
        }

        #[test]
        fn buildkit_containers_use_proxy_worker_wrapper() {
            let mut pending = PendingContainer {
                id: "buildkit-id".to_string(),
                name: "buildx_buildkit_demo".to_string(),
                created_at: 0,
                runtime_id: "buildx_buildkit_demo".to_string(),
                image: "moby/buildkit:buildx-stable-1".to_string(),
                command: vec![
                    "/usr/bin/buildkitd-entrypoint".to_string(),
                    "--debug".to_string(),
                ],
                env: Vec::new(),
                working_dir: None,
                mounts: Vec::new(),
                network: None,
                aliases: Vec::new(),
                labels: serde_json::Map::new(),
                netns_name: None,
                netns_path: None,
                ports: Vec::new(),
                log_path: std::env::temp_dir().join("cratebay-buildkit-test.log"),
                no_pull: false,
                registry_mirrors: vec![],
                privileged: true,
                started_with_ctr: false,
                exit_code: None,
            };

            configure_buildkit_proxy_worker(&mut pending, Some("192.168.64.1:3128"))
                .expect("buildkit proxy worker should configure");

            assert!(pending
                .command
                .contains(&"--oci-worker-binary=/usr/local/bin/cratebay-runc-wrapper".to_string()));
            assert!(pending
                .env
                .contains(&"CRATEBAY_REAL_RUNC=/usr/bin/buildkit-runc".to_string()));
            assert!(pending
                .env
                .contains(&"HTTP_PROXY=http://192.168.64.1:3128".to_string()));
            let mount = pending
                .mounts
                .iter()
                .find(|mount| mount.target == "/usr/local/bin/cratebay-runc-wrapper")
                .expect("wrapper mount");
            assert!(mount.readonly);
            assert!(
                fs::metadata(&mount.source)
                    .expect("wrapper file exists")
                    .permissions()
                    .mode()
                    & 0o111
                    != 0
            );
        }

        #[test]
        fn buildkit_proxy_worker_relaxes_default_seccomp_action() {
            let mut config = json!({
                "linux": {
                    "seccomp": {
                        "defaultAction": "SCMP_ACT_ERRNO",
                        "defaultErrnoRet": 1
                    }
                }
            });

            relax_runc_seccomp_default(&mut config);

            assert_eq!(
                config["linux"]["seccomp"]["defaultAction"],
                "SCMP_ACT_ALLOW"
            );
            assert_eq!(config["linux"]["seccomp"]["defaultErrnoRet"], 0);
        }

        #[test]
        fn legacy_python36_ctypes_movaps_store_is_patched_to_movups() {
            let dir = std::env::temp_dir().join(format!(
                "cratebay-ctypes-test-{}-{}",
                std::process::id(),
                unique_task_id("py36")
            ));
            fs::create_dir_all(&dir).expect("tempdir");
            let path = dir.join("_ctypes.cpython-36m-x86_64-linux-gnu.so");
            fs::write(
                &path,
                [
                    b"prefix".as_slice(),
                    &[0x0f, 0x29, 0x43, 0x60],
                    b"suffix".as_slice(),
                ]
                .concat(),
            )
            .expect("write fake _ctypes");

            assert!(
                patch_legacy_python36_ctypes_file(&path).expect("patch should succeed"),
                "first patch should modify the file"
            );
            let patched = fs::read(&path).expect("read patched");
            assert!(patched
                .windows(4)
                .any(|window| window == [0x0f, 0x11, 0x43, 0x60]));
            assert!(!patched
                .windows(4)
                .any(|window| window == [0x0f, 0x29, 0x43, 0x60]));
            assert!(
                !patch_legacy_python36_ctypes_file(&path).expect("second patch should succeed"),
                "second patch should be a no-op"
            );
            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn non_buildkit_containers_do_not_use_proxy_worker_wrapper() {
            let mut pending = PendingContainer {
                id: "app-id".to_string(),
                name: "app".to_string(),
                created_at: 0,
                runtime_id: "app".to_string(),
                image: "alpine:3.20".to_string(),
                command: vec!["sh".to_string()],
                env: Vec::new(),
                working_dir: None,
                mounts: Vec::new(),
                network: None,
                aliases: Vec::new(),
                labels: serde_json::Map::new(),
                netns_name: None,
                netns_path: None,
                ports: Vec::new(),
                log_path: std::env::temp_dir().join("cratebay-app-test.log"),
                no_pull: false,
                registry_mirrors: vec![],
                privileged: false,
                started_with_ctr: false,
                exit_code: None,
            };

            configure_buildkit_proxy_worker(&mut pending, Some("192.168.64.1:3128"))
                .expect("non-buildkit should be a no-op");

            assert_eq!(pending.command, vec!["sh".to_string()]);
            assert!(pending.mounts.is_empty());
        }

        #[test]
        fn pending_port_mappings_feed_cni_runtime_config() {
            let host_config = json!({
                "PortBindings": {
                    "80/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "8080" }],
                    "53/udp": [{}]
                }
            });

            let ports = pending_port_mappings(Some(&host_config));
            let runtime_config = cni_port_mappings_value(&ports);
            let http = ports
                .iter()
                .find(|port| port.container_port == 80)
                .expect("http port mapping");
            let dns = ports
                .iter()
                .find(|port| port.container_port == 53)
                .expect("dns port mapping");

            assert_eq!(ports.len(), 2);
            assert_eq!(http.host_ip.as_deref(), Some("127.0.0.1"));
            assert_eq!(http.host_port, 8080);
            assert_eq!(http.protocol, "tcp");
            assert_eq!(dns.host_port, 53);
            assert_eq!(dns.protocol, "udp");
            assert!(runtime_config
                .as_array()
                .expect("runtime config array")
                .iter()
                .any(|port| port["hostIP"] == "127.0.0.1"
                    && port["hostPort"] == 8080
                    && port["containerPort"] == 80));
        }

        #[test]
        fn cni_plugin_input_preserves_prev_result_and_ports() {
            let ports = vec![CniPortMapping {
                host_ip: None,
                host_port: 8080,
                container_port: 80,
                protocol: "tcp".to_string(),
            }];
            let input = cni_plugin_input(
                &json!({
                    "cniVersion": "1.0.0",
                    "name": "sandbox-net",
                }),
                &json!({
                    "type": "portmap",
                    "capabilities": { "portMappings": true },
                }),
                Some(&json!({ "ips": [{ "address": "10.99.0.2/24" }] })),
                &ports,
            );

            assert_eq!(input["type"], "portmap");
            assert_eq!(input["name"], "sandbox-net");
            assert_eq!(input["cniVersion"], "1.0.0");
            assert_eq!(input["prevResult"]["ips"][0]["address"], "10.99.0.2/24");
            assert_eq!(input["runtimeConfig"]["portMappings"][0]["hostPort"], 8080);
            assert_eq!(
                input["runtimeConfig"]["portMappings"][0]["containerPort"],
                80
            );
        }

        #[test]
        fn ctr_run_args_join_prepared_cratebay_network_namespace() {
            let (name, payload, _) = native_create_request(&json!({
                "name": "sandbox-demo",
                "image": "alpine:3.20",
                "command": "true",
                "network": "demo-pod",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &payload);
            let attachment = pending_network_attachment(&pending, "demo-pod");

            let args = build_ctr_run_args_with_netns(
                &pending,
                "docker.io/library/alpine:3.20",
                Some(&attachment.netns_path),
            );

            assert_eq!(attachment.network, "demo-pod");
            assert!(attachment
                .netns_name
                .starts_with("cb-demo-pod-sandbox-demo-"));
            assert_eq!(
                attachment.netns_name.len(),
                "cb-demo-pod-sandbox-demo-".len() + 12
            );
            assert_eq!(
                attachment.netns_path,
                PathBuf::from("/var/run/netns").join(&attachment.netns_name)
            );
            assert!(args.contains(&"--with-ns".to_string()));
            assert!(args.contains(&format!("network:{}", attachment.netns_path.display())));
            assert!(args.contains(&"docker.io/library/alpine:3.20".to_string()));
        }

        #[test]
        fn netns_names_do_not_collide_for_long_compose_container_names() {
            let network = "vpc_network_devcontainer_vpc-network";
            let mut app = pending_from_create_payload(
                "vpc_network_devcontainer-app-1",
                &json!({
                    "Image": "alpine:3.20",
                    "HostConfig": { "NetworkMode": network },
                }),
            );
            let mut mysql = app.clone();
            mysql.id = "vpc_network_devcontainer-mysql-1".to_string();
            mysql.name = "vpc_network_devcontainer-mysql-1".to_string();
            app.name = "vpc_network_devcontainer-app-1".to_string();

            let app_attachment = pending_network_attachment(&app, network);
            let mysql_attachment = pending_network_attachment(&mysql, network);

            assert_ne!(app_attachment.netns_name, mysql_attachment.netns_name);
            assert!(app_attachment
                .netns_name
                .starts_with("cb-vpc-network-devcontainer-vpc-"));
            assert!(mysql_attachment
                .netns_name
                .starts_with("cb-vpc-network-devcontainer-vpc-"));
        }

        #[test]
        fn native_container_items_dedupe_by_name() {
            let mut items = vec![
                json!({ "id": "external-id", "name": "sandbox-demo" }),
                json!({ "id": "sandbox-demo", "name": "sandbox-demo" }),
                json!({ "id": "other", "name": "other" }),
            ];

            dedupe_native_items_by_name(&mut items);

            assert_eq!(items.len(), 2);
            assert_eq!(items[0]["id"], "external-id");
            assert_eq!(items[1]["id"], "other");
        }

        #[test]
        fn native_port_binding_parses_common_publish_specs() {
            assert_eq!(
                native_port_binding("8080:80/tcp"),
                Some(("80/tcp".to_string(), "8080".to_string()))
            );
            assert_eq!(
                native_port_binding("443"),
                Some(("443/tcp".to_string(), String::new()))
            );
        }

        #[test]
        fn native_image_ref_appends_optional_tag() {
            assert_eq!(
                native_image_ref(&json!({ "image": "alpine", "tag": "latest" })),
                Some("alpine:latest".to_string())
            );
            assert_eq!(
                native_image_ref(&json!({ "image": "alpine:3.20", "tag": "latest" })),
                Some("alpine:3.20".to_string())
            );
        }

        #[test]
        fn native_image_inspect_payload_exposes_cratebay_contract() {
            let payload = native_image_inspect_payload(
                "alpine:3.20",
                "docker.io/library/alpine:3.20@sha256:abc123",
                json!({
                    "Id": "sha256:abc123",
                    "RepoTags": ["docker.io/library/alpine:3.20"],
                    "RepoDigests": ["docker.io/library/alpine@sha256:abc123"],
                    "Created": "2026-06-03T00:00:00Z",
                    "Architecture": "x86_64",
                    "Os": "linux",
                    "Size": 123456,
                    "RootFS": { "Layers": ["sha256:layer1", "sha256:layer2"] }
                }),
            );

            assert_eq!(payload["api"], "cratebay.image.inspect.v1");
            assert_eq!(payload["id"], "sha256:abc123");
            assert_eq!(
                payload["imageRef"],
                "docker.io/library/alpine:3.20@sha256:abc123"
            );
            assert_eq!(payload["repoTags"][0], "docker.io/library/alpine:3.20");
            assert_eq!(payload["sizeBytes"], 123456);
            assert_eq!(payload["layers"], 2);
            assert_eq!(payload["backend"], "containerd");
            assert_eq!(payload["managedBy"], "cratebay");
        }

        #[test]
        fn native_pack_container_image_payload_exposes_cratebay_contract() {
            let pending = PendingContainer {
                id: "abc123".to_string(),
                name: "sandbox-demo".to_string(),
                created_at: 1_780_000_000,
                runtime_id: "sandbox-demo".to_string(),
                image: "alpine:3.20".to_string(),
                command: vec!["sleep".to_string(), "60".to_string()],
                env: vec!["A=1".to_string()],
                working_dir: Some("/workspace".to_string()),
                mounts: vec![],
                network: None,
                aliases: vec![],
                labels: serde_json::Map::new(),
                netns_name: None,
                netns_path: None,
                ports: vec![],
                log_path: PathBuf::from("/run/cratebay/container-logs/sandbox-demo.log"),
                no_pull: false,
                registry_mirrors: vec![],
                privileged: false,
                started_with_ctr: true,
                exit_code: None,
            };
            let payload = native_pack_container_image_payload(
                &pending,
                "sandbox-pack:latest",
                ContainerCommitResult {
                    target_ref: "docker.io/library/sandbox-pack:latest".to_string(),
                    layer_digest: "layer123".to_string(),
                    config_digest: "config123".to_string(),
                    rootfs: PathBuf::from("/run/containerd/rootfs/sandbox-demo"),
                },
            );

            assert_eq!(payload["api"], "cratebay.image.pack.v1");
            assert_eq!(payload["backend"], "containerd");
            assert_eq!(payload["managedBy"], "cratebay");
            assert_eq!(payload["container"], "sandbox-demo");
            assert_eq!(payload["image"], "sandbox-pack:latest");
            assert_eq!(payload["imageRef"], "docker.io/library/sandbox-pack:latest");
            assert_eq!(payload["packed"], true);
            assert_eq!(payload["layerDigest"], "sha256:layer123");
            assert_eq!(payload["configDigest"], "sha256:config123");
        }

        #[test]
        fn ctr_image_pull_candidates_prefer_containerd_registry_refs() {
            assert_eq!(
                ctr_image_pull_candidates("alpine:3.20"),
                vec!["docker.io/library/alpine:3.20", "alpine:3.20"]
            );
            assert_eq!(
                ctr_image_pull_candidates("ghcr.io/acme/app:v1"),
                vec!["ghcr.io/acme/app:v1"]
            );
            assert_eq!(
                ctr_image_pull_candidates("compose-app"),
                vec![
                    "docker.io/library/compose-app:latest",
                    "docker.io/library/compose-app",
                    "compose-app:latest",
                    "compose-app",
                ]
            );
        }

        #[test]
        fn ctr_image_pull_args_use_hosts_dir_for_loopback_registries() {
            let _lock = env_lock();
            let old_tmp_root = std::env::var_os("CRATEBAY_ENGINE_TMP_ROOT");
            let temp_root = std::env::temp_dir().join(format!(
                "cratebay-adapter-hosts-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time")
                    .as_nanos()
            ));
            struct TmpRootGuard {
                old_tmp_root: Option<std::ffi::OsString>,
                temp_root: PathBuf,
            }
            impl Drop for TmpRootGuard {
                fn drop(&mut self) {
                    if let Some(old) = self.old_tmp_root.clone() {
                        std::env::set_var("CRATEBAY_ENGINE_TMP_ROOT", old);
                    } else {
                        std::env::remove_var("CRATEBAY_ENGINE_TMP_ROOT");
                    }
                    let _ = fs::remove_dir_all(&self.temp_root);
                }
            }
            let _guard = TmpRootGuard {
                old_tmp_root,
                temp_root: temp_root.clone(),
            };
            std::env::set_var("CRATEBAY_ENGINE_TMP_ROOT", &temp_root);

            let hosts_root = temp_root.join("containerd-hosts");
            assert_eq!(
                ctr_image_pull_args("127.0.0.1:5000/cratebay-smoke:local")
                    .expect("loopback pull args"),
                vec![
                    "images",
                    "pull",
                    "--hosts-dir",
                    hosts_root.display().to_string().as_str(),
                    "127.0.0.1:5000/cratebay-smoke:local"
                ]
            );
            for namespace in ["127.0.0.1:5000", "127.0.0.1_5000_"] {
                let hosts_toml = fs::read_to_string(hosts_root.join(namespace).join("hosts.toml"))
                    .expect("hosts.toml should be written");
                assert!(hosts_toml.contains("server = \"http://127.0.0.1:5000\""));
                assert!(hosts_toml.contains("[host.\"http://127.0.0.1:5000\"]"));
            }

            assert_eq!(
                ctr_image_pull_args("localhost:5000/cratebay-smoke:local")
                    .expect("localhost pull args"),
                vec![
                    "images",
                    "pull",
                    "--hosts-dir",
                    hosts_root.display().to_string().as_str(),
                    "localhost:5000/cratebay-smoke:local"
                ]
            );
            assert_eq!(
                ctr_image_pull_args("[::1]:5000/cratebay-smoke:local")
                    .expect("ipv6 loopback pull args"),
                vec![
                    "images",
                    "pull",
                    "--hosts-dir",
                    hosts_root.display().to_string().as_str(),
                    "[::1]:5000/cratebay-smoke:local"
                ]
            );
            assert_eq!(
                containerd_hosts_namespaces("127.0.0.1:5000"),
                vec!["127.0.0.1:5000", "127.0.0.1_5000_"]
            );
            assert_eq!(
                ctr_image_pull_args("registry.example.com:5000/team/app:v1")
                    .expect("registry pull args"),
                vec!["images", "pull", "registry.example.com:5000/team/app:v1"]
            );
        }

        #[test]
        fn loopback_registry_refs_parse_repository_and_reference() {
            assert_eq!(
                parse_registry_image_ref("127.0.0.1:5000/cratebay-smoke:local"),
                Some(super::RegistryImageRef {
                    registry: "127.0.0.1:5000".to_string(),
                    repository: "cratebay-smoke".to_string(),
                    reference: "local".to_string(),
                })
            );
            assert_eq!(
                parse_registry_image_ref("localhost:5000/team/app@sha256:abc123"),
                Some(super::RegistryImageRef {
                    registry: "localhost:5000".to_string(),
                    repository: "team/app".to_string(),
                    reference: "sha256:abc123".to_string(),
                })
            );
            assert_eq!(parse_registry_image_ref("alpine:3.20"), None);
        }

        #[test]
        fn chunked_registry_response_body_decodes() {
            assert_eq!(
                decode_http_chunked_body(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n")
                    .expect("chunked body"),
                b"Wikipedia"
            );
            assert!(decode_http_chunked_body(b"4\r\nWiki").is_err());
        }

        #[test]
        fn registry_mirror_rewrite_keeps_docker_hub_rules() {
            assert_eq!(
                normalize_registry_mirrors(Some(&json!([
                    " https://mirror.example.com/ ",
                    "http://mirror-2.example.com",
                    ""
                ]))),
                vec!["mirror.example.com", "mirror-2.example.com"]
            );
            assert_eq!(
                rewrite_image_for_registry_mirror("alpine:3.20", "mirror.example.com"),
                "mirror.example.com/library/alpine:3.20"
            );
            assert_eq!(
                rewrite_image_for_registry_mirror("library/alpine:3.20", "mirror.example.com"),
                "mirror.example.com/library/alpine:3.20"
            );
            assert_eq!(
                rewrite_image_for_registry_mirror("team/app:v1", "https://mirror.example.com/"),
                "mirror.example.com/team/app:v1"
            );
            assert_eq!(
                rewrite_image_for_registry_mirror("ghcr.io/team/app:v1", "mirror.example.com"),
                "ghcr.io/team/app:v1"
            );
        }

        #[test]
        fn start_image_selection_reuses_existing_containerd_refs() {
            let list = b"docker.io/library/alpine:3.20\nghcr.io/acme/app:v1@sha256:abc123\n";

            assert_eq!(
                select_containerd_image_ref(list, "alpine:3.20"),
                Some("docker.io/library/alpine:3.20".to_string())
            );
            assert_eq!(
                select_containerd_image_ref(list, "ghcr.io/acme/app:v1"),
                Some("ghcr.io/acme/app:v1@sha256:abc123".to_string())
            );
            assert_eq!(select_containerd_image_ref(list, "missing:latest"), None);
            assert!(image_refs_equivalent(
                "ghcr.io/acme/app:v1@sha256:abc123",
                "ghcr.io/acme/app:v1"
            ));
        }

        #[test]
        fn start_image_selection_treats_untagged_refs_as_latest() {
            let list = b"vpc_network_devcontainer-app:latest\ndocker.io/library/redis:latest\n";

            assert_eq!(
                select_containerd_image_ref(list, "vpc_network_devcontainer-app"),
                Some("vpc_network_devcontainer-app:latest".to_string())
            );
            assert_eq!(
                select_containerd_image_ref(list, "redis"),
                Some("docker.io/library/redis:latest".to_string())
            );
        }

        #[test]
        fn containerd_image_refs_map_to_docker_compatible_summaries() {
            let (repository, tag, digest) =
                image_ref_parts("docker.io/library/alpine:3.20@sha256:abc123");
            let tagged = ctr_image_summary("docker.io/library/alpine:3.20");
            let digested = ctr_image_summary("docker.io/library/alpine:3.20@sha256:abc123");

            assert_eq!(repository, "docker.io/library/alpine");
            assert_eq!(tag, "3.20");
            assert_eq!(digest.as_deref(), Some("sha256:abc123"));
            assert_eq!(tagged["Id"], "docker.io/library/alpine:3.20");
            assert_eq!(tagged["RepoTags"][0], "docker.io/library/alpine:3.20");
            assert_eq!(tagged["CrateBay"]["backend"], "containerd");
            assert_eq!(digested["Id"], "sha256:abc123");
            assert_eq!(digested["RepoTags"][0], "docker.io/library/alpine:3.20");
            assert_eq!(
                digested["RepoDigests"][0],
                "docker.io/library/alpine@sha256:abc123"
            );
        }

        #[test]
        fn containerd_image_ref_selection_matches_tags_and_digests() {
            let refs = [
                "docker.io/library/alpine:3.20",
                "ghcr.io/acme/app:v1@sha256:abc123",
            ];

            assert_eq!(
                select_containerd_image_ref_from_refs(refs.iter().copied(), "alpine:3.20"),
                Some("docker.io/library/alpine:3.20".to_string())
            );
            assert_eq!(
                select_containerd_image_ref_from_refs(refs.iter().copied(), "ghcr.io/acme/app:v1"),
                Some("ghcr.io/acme/app:v1@sha256:abc123".to_string())
            );
            assert_eq!(
                select_containerd_image_ref_from_refs(refs.iter().copied(), "sha256:abc123"),
                Some("ghcr.io/acme/app:v1@sha256:abc123".to_string())
            );
            assert_eq!(
                select_containerd_image_ref_from_refs(
                    ["docker.io/library/vpc_network_devcontainer-app:latest"]
                        .iter()
                        .copied(),
                    "sha256:docker.io/library/vpc_network_devcontainer-app:latest"
                ),
                Some("docker.io/library/vpc_network_devcontainer-app:latest".to_string())
            );
            assert!(image_ref_matches(
                "ghcr.io/acme/app:v1@sha256:abc123",
                "ghcr.io/acme/app:v1"
            ));
            assert!(!image_ref_matches(
                "docker.io/library/alpine:3.20",
                "busybox"
            ));
        }

        #[test]
        fn image_export_uses_containerd_native_command_shape() {
            let refs = vec![
                "docker.io/library/alpine:3.20".to_string(),
                "ghcr.io/acme/app:v1".to_string(),
            ];
            let args = ctr_image_export_args(&PathBuf::from("/tmp/cratebay-images.tar"), &refs);

            assert_eq!(
                args,
                vec![
                    "images",
                    "export",
                    "/tmp/cratebay-images.tar",
                    "docker.io/library/alpine:3.20",
                    "ghcr.io/acme/app:v1"
                ]
            );
        }

        #[test]
        fn exec_args_use_containerd_task_backend() {
            let record = ExecRecord {
                container_id: "sandbox-id".to_string(),
                cmd: vec!["/bin/sh".to_string(), "-lc".to_string(), "id".to_string()],
                working_dir: Some("/workspace".to_string()),
                attach_stdin: false,
                tty: false,
                exit_code: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
            };

            assert_eq!(
                exec_ctr_args("sandbox-name", &record, "exec-1"),
                vec![
                    "tasks",
                    "exec",
                    "--exec-id",
                    "exec-1",
                    "--cwd",
                    "/workspace",
                    "sandbox-name",
                    "/bin/sh",
                    "-lc",
                    "id"
                ]
            );
        }

        #[test]
        fn exec_args_include_tty_when_requested() {
            let record = ExecRecord {
                container_id: "sandbox-id".to_string(),
                cmd: vec!["/bin/sh".to_string()],
                working_dir: None,
                attach_stdin: true,
                tty: true,
                exit_code: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
            };

            assert_eq!(
                exec_ctr_args("sandbox-name", &record, "exec-tty"),
                vec![
                    "tasks",
                    "exec",
                    "--exec-id",
                    "exec-tty",
                    "--tty",
                    "sandbox-name",
                    "/bin/sh",
                ]
            );
        }

        #[test]
        fn exec_output_limit_appends_until_stream_limit() {
            let mut bytes = b"abc".to_vec();

            let truncated = extend_limited_bytes(&mut bytes, b"defgh", Some(6));

            assert_eq!(bytes, b"abcdef");
            assert!(truncated);
        }

        #[test]
        fn exec_output_limit_marks_late_chunks_as_truncated() {
            let mut bytes = b"abcdef".to_vec();

            let truncated = extend_limited_bytes(&mut bytes, b"gh", Some(6));

            assert_eq!(bytes, b"abcdef");
            assert!(truncated);
        }

        #[test]
        fn exec_output_truncate_reports_when_existing_buffer_exceeds_limit() {
            let mut bytes = b"abcdef".to_vec();

            let truncated = truncate_bytes(&mut bytes, Some(4));

            assert_eq!(bytes, b"abcd");
            assert!(truncated);
        }

        #[test]
        fn terminal_args_use_containerd_tty_backend() {
            let record = ExecRecord {
                container_id: "sandbox-id".to_string(),
                cmd: vec!["sh".to_string(), "-i".to_string()],
                working_dir: Some("/workspace".to_string()),
                attach_stdin: true,
                tty: true,
                exit_code: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
            };

            assert_eq!(
                terminal_ctr_args("sandbox-name", &record, "tty-1"),
                vec![
                    "tasks",
                    "exec",
                    "--exec-id",
                    "tty-1",
                    "--tty",
                    "--cwd",
                    "/workspace",
                    "sandbox-name",
                    "sh",
                    "-i",
                ]
            );
        }

        #[test]
        fn terminal_size_defaults_and_clamps() {
            assert_eq!(terminal_size_from_payload(&json!({})), (80, 24));
            assert_eq!(
                terminal_size_from_payload(&json!({ "cols": 12, "rows": 1 })),
                (20, 2)
            );
            assert_eq!(
                terminal_size_from_payload(&json!({ "cols": 9999, "rows": 9999 })),
                (500, 200)
            );
            assert_eq!(
                terminal_size_from_payload(&json!({ "columns": 132, "rows": 43 })),
                (132, 43)
            );
        }

        #[test]
        fn pty_window_size_can_be_resized() {
            let pty = open_terminal_pty(80, 24).expect("test pty should open");
            assert_eq!(pty_window_size(pty.master.as_raw_fd()).unwrap(), (80, 24));

            set_pty_window_size(pty.master.as_raw_fd(), 120, 33).unwrap();

            assert_eq!(pty_window_size(pty.master.as_raw_fd()).unwrap(), (120, 33));
        }

        #[test]
        fn native_network_create_request_maps_agent_shape() {
            let (name, payload) = native_network_create_request(&json!({
                "name": "pod-demo",
                "driver": "bridge",
                "internal": true,
                "enableIPv6": true,
                "labels": { "com.cratebay.pod": "true" },
                "options": { "mtu": "1500" }
            }))
            .expect("network create payload");

            assert_eq!(name, "pod-demo");
            assert_eq!(payload["Name"], "pod-demo");
            assert_eq!(payload["Driver"], "bridge");
            assert_eq!(payload["Internal"], true);
            assert_eq!(payload["EnableIPv6"], true);
            assert_eq!(payload["Labels"]["com.cratebay.pod"], "true");
            assert_eq!(payload["Options"]["mtu"], "1500");
        }

        #[test]
        fn managed_network_value_adds_cratebay_registry_metadata() {
            let (_, payload) = native_network_create_request(&json!({
                "name": "sandbox-net",
                "driver": "bridge",
                "labels": {
                    "purpose": "sandbox",
                    "com.cratebay.managed": "false"
                },
                "ipam": { "Config": [{ "Subnet": "10.99.0.0/24" }] }
            }))
            .expect("network create payload");

            let value = managed_network_value("sandbox-net", &payload);

            assert_eq!(value["Id"], "sandbox-net");
            assert_eq!(value["Name"], "sandbox-net");
            assert_eq!(value["Driver"], "bridge");
            assert_eq!(value["Labels"]["purpose"], "sandbox");
            assert_eq!(value["Labels"]["com.cratebay.managed"], "true");
            assert_eq!(value["CrateBay"]["backend"], "cratebay-cni");
            assert_eq!(value["IPAM"]["Config"][0]["Subnet"], "10.99.0.0/24");
        }

        #[test]
        fn managed_network_cni_config_uses_requested_subnet() {
            let (_, payload) = native_network_create_request(&json!({
                "name": "sandbox-net",
                "internal": true,
                "ipam": { "Config": [{ "Subnet": "10.99.0.0/24" }] }
            }))
            .expect("network create payload");

            let cni = managed_network_cni_config("sandbox-net", &payload);

            assert_eq!(cni["name"], "sandbox-net");
            assert_eq!(cni["plugins"][0]["type"], "bridge");
            assert_eq!(cni["plugins"][0]["bridge"], "cbsandbox-net");
            assert_eq!(cni["plugins"][0]["ipMasq"], false);
            assert_eq!(
                cni["plugins"][0]["ipam"]["ranges"][0][0]["subnet"],
                "10.99.0.0/24"
            );
            assert_eq!(cni["plugins"][1]["type"], "portmap");
        }

        #[test]
        fn native_pod_create_request_forces_cratebay_pod_labels() {
            let (_, payload) = native_pod_create_request(&json!({
                "name": "demo-pod",
                "labels": {
                    "purpose": "sandbox",
                    "com.cratebay.managed": "false",
                    "com.cratebay.pod": "false"
                }
            }))
            .expect("pod create payload");

            let value = managed_network_value("demo-pod", &payload);

            assert_eq!(value["Labels"]["purpose"], "sandbox");
            assert_eq!(value["Labels"]["com.cratebay.managed"], "true");
            assert_eq!(value["Labels"]["com.cratebay.pod"], "true");
            assert!(is_cratebay_pod_network(&value));
        }

        #[test]
        fn docker_compat_network_items_dedupe_by_name() {
            let mut items = vec![
                json!({ "Id": "cratebay-net", "Name": "sandbox-net" }),
                json!({ "Id": "legacy-net", "Name": "sandbox-net" }),
                json!({ "Id": "other-net", "Name": "other-net" }),
            ];

            dedupe_docker_items_by_name(&mut items);

            assert_eq!(items.len(), 2);
            assert_eq!(items[0]["Id"], "cratebay-net");
            assert_eq!(items[1]["Id"], "other-net");
        }

        #[test]
        fn native_volume_create_request_maps_agent_shape() {
            let (name, payload) = native_volume_create_request(&json!({
                "name": "workspace-cache",
                "driver": "local",
                "labels": { "purpose": "cache" },
                "options": { "kind": "tmp" }
            }))
            .expect("volume create payload");

            assert_eq!(name, "workspace-cache");
            assert_eq!(payload["Name"], "workspace-cache");
            assert_eq!(payload["Driver"], "local");
            assert_eq!(payload["Labels"]["purpose"], "cache");
            assert_eq!(payload["Options"]["kind"], "tmp");
        }

        #[test]
        fn native_image_remove_requires_force_when_pending_container_uses_image() {
            let state = test_state_without_external_frontend();
            let (name, payload, _) = native_create_request(&json!({
                "name": "image-user",
                "image": "alpine:3.20",
                "command": "sleep 60",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &payload);
            let pending_id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &pending_id, &pending_name, pending)
                .expect("store pending image user");

            let removed = native_remove_image(
                &state,
                &state.config,
                "docker.io/library/alpine:3.20".to_string(),
                b"{}",
            );
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove conflict json");
            let details = &body["details"];

            assert_eq!(removed.status, 409);
            assert_eq!(body["message"], "image is in use by CrateBay containers");
            assert_eq!(details["api"], "cratebay.image.remove.v1");
            assert_eq!(details["id"], "docker.io/library/alpine:3.20");
            assert_eq!(details["forceRequired"], true);
            assert_eq!(details["containers"][0]["name"], "image-user");
            assert_eq!(details["containers"][0]["image"], "alpine:3.20");
        }

        #[test]
        fn native_volume_remove_reports_force_request() {
            let _ = std::fs::remove_dir_all(super::volume_root());
            let created = create_volume(br#"{ "Name": "workspace-cache", "Driver": "local" }"#);
            assert_eq!(created.status, 201);

            let removed =
                native_remove_volume("workspace-cache".to_string(), br#"{ "force": true }"#);
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove json");

            assert_eq!(removed.status, 200);
            assert_eq!(body["api"], "cratebay.volume.remove.v1");
            assert_eq!(body["name"], "workspace-cache");
            assert_eq!(body["force"], true);
            assert_eq!(body["removed"], true);
        }

        #[test]
        fn native_volume_remove_requires_force_when_pending_container_uses_volume() {
            let state = test_state_without_external_frontend();
            let _ = std::fs::remove_dir_all(super::volume_root());
            let created = create_volume(br#"{ "Name": "workspace-cache", "Driver": "local" }"#);
            assert_eq!(created.status, 201);
            let (name, payload, _) = native_create_request(&json!({
                "name": "volume-user",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "volume": ["workspace-cache:/workspace:ro"],
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &payload);
            let pending_id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &pending_id, &pending_name, pending)
                .expect("store pending volume user");

            let removed = native_remove_volume("workspace-cache".to_string(), b"{}");
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove conflict json");
            let details = &body["details"];

            assert_eq!(removed.status, 409);
            assert_eq!(body["message"], "volume is in use by CrateBay containers");
            assert_eq!(details["api"], "cratebay.volume.remove.v1");
            assert_eq!(details["name"], "workspace-cache");
            assert_eq!(details["forceRequired"], true);
            assert_eq!(details["containers"][0]["name"], "volume-user");
            assert!(super::volume_data_path("workspace-cache").exists());
        }

        #[test]
        fn native_volume_force_remove_deletes_referenced_volume() {
            let state = test_state_without_external_frontend();
            let _ = std::fs::remove_dir_all(super::volume_root());
            let created = create_volume(br#"{ "Name": "workspace-cache", "Driver": "local" }"#);
            assert_eq!(created.status, 201);
            let (name, payload, _) = native_create_request(&json!({
                "name": "volume-user",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "volume": ["workspace-cache:/workspace:ro"],
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &payload);
            let pending_id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &pending_id, &pending_name, pending)
                .expect("store pending volume user");

            let removed =
                native_remove_volume("workspace-cache".to_string(), br#"{ "force": true }"#);
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove json");

            assert_eq!(removed.status, 200);
            assert_eq!(body["force"], true);
            assert!(!super::volume_data_path("workspace-cache").exists());
            assert!(pending_container(&state, "volume-user").is_some());
        }

        #[test]
        fn native_network_remove_requires_force_when_pending_container_uses_network() {
            let state = test_state_without_external_frontend();
            let (_, payload) = native_network_create_request(&json!({
                "name": "workspace-net",
                "driver": "bridge"
            }))
            .expect("network create payload");
            create_managed_network(&state.config, &payload).expect("managed network");
            let (name, create_payload, _) = native_create_request(&json!({
                "name": "network-user",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "network": "workspace-net",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &create_payload);
            let pending_id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &pending_id, &pending_name, pending)
                .expect("store pending network user");

            let removed =
                native_remove_network(&state, &state.config, "workspace-net".to_string(), b"{}");
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove conflict json");
            let details = &body["details"];

            assert_eq!(removed.status, 409);
            assert_eq!(body["message"], "network is in use by CrateBay containers");
            assert_eq!(details["api"], "cratebay.network.remove.v1");
            assert_eq!(details["id"], "workspace-net");
            assert_eq!(details["forceRequired"], true);
            assert_eq!(details["containers"][0]["name"], "network-user");
            assert!(super::network_registry_path("workspace-net").exists());
        }

        #[test]
        fn native_network_force_remove_detaches_pending_container() {
            let state = test_state_without_external_frontend();
            let (_, payload) = native_network_create_request(&json!({
                "name": "workspace-net",
                "driver": "bridge"
            }))
            .expect("network create payload");
            create_managed_network(&state.config, &payload).expect("managed network");
            let (name, create_payload, _) = native_create_request(&json!({
                "name": "network-user",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "network": "workspace-net",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &create_payload);
            let pending_id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &pending_id, &pending_name, pending)
                .expect("store pending network user");

            let removed = native_remove_network(
                &state,
                &state.config,
                "workspace-net".to_string(),
                br#"{ "force": true }"#,
            );
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove json");
            let pending = pending_container(&state, "network-user").expect("pending network user");

            assert_eq!(removed.status, 200);
            assert_eq!(body["force"], true);
            assert!(!super::network_registry_path("workspace-net").exists());
            assert_eq!(pending.network, None);
            assert!(pending.netns_name.is_none());
            assert!(pending.netns_path.is_none());
        }

        #[test]
        fn docker_volume_create_preserves_compose_labels() {
            let _ = std::fs::remove_dir_all(super::volume_root());
            let response = create_volume(
                br#"{
                    "Name": "compose-cache",
                    "Driver": "local",
                    "Labels": {
                        "com.docker.compose.project": "demo",
                        "com.docker.compose.volume": "cache"
                    },
                    "Options": { "type": "none" }
                }"#,
            );
            let inspect = inspect_volume("compose-cache".to_string());
            let payload: serde_json::Value =
                serde_json::from_slice(&inspect.body).expect("volume inspect json");

            assert_eq!(response.status, 201);
            assert_eq!(inspect.status, 200);
            assert_eq!(payload["Labels"]["com.docker.compose.project"], "demo");
            assert_eq!(payload["Labels"]["com.docker.compose.volume"], "cache");
            assert_eq!(payload["Options"]["type"], "none");
        }

        #[test]
        fn native_pod_create_request_adds_cratebay_labels() {
            let (name, payload) = native_pod_create_request(&json!({
                "name": "demo-pod",
                "driver": "bridge",
                "labels": { "custom": "yes" }
            }))
            .expect("pod create payload");

            assert_eq!(name, "demo-pod");
            assert_eq!(payload["Name"], "demo-pod");
            assert_eq!(payload["Driver"], "bridge");
            assert_eq!(payload["Labels"]["com.cratebay.managed"], "true");
            assert_eq!(payload["Labels"]["com.cratebay.pod"], "true");
            assert_eq!(payload["Labels"]["custom"], "yes");
        }

        #[test]
        fn native_pod_list_and_inspect_include_started_cratebay_containers() {
            let state = test_state_without_external_frontend();
            let (_, pod_payload) = native_pod_create_request(&json!({
                "name": "demo-pod",
                "driver": "bridge"
            }))
            .expect("pod create payload");
            create_managed_network(&state.config, &pod_payload).expect("managed pod network");

            let mut query = HashMap::new();
            query.insert("name".to_string(), "sandbox-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"],
                    "HostConfig": { "NetworkMode": "demo-pod" }
                }"#,
            );
            assert_eq!(create.status, 201);
            mark_pending_started_with_ctr(&state, "sandbox-demo", "sandbox-demo");
            let pending = pending_container(&state, "sandbox-demo").expect("pod container");

            let listed = list_cratebay_pods(&state);
            let listed_body: serde_json::Value =
                serde_json::from_slice(&listed.body).expect("native pod list response");
            let inspected = inspect_cratebay_pod(&state, "demo-pod".to_string());
            let inspected_body: serde_json::Value =
                serde_json::from_slice(&inspected.body).expect("native pod inspect response");

            assert_eq!(listed.status, 200);
            assert_eq!(listed_body["api"], "cratebay.pods.v1");
            assert_eq!(listed_body["items"][0]["name"], "demo-pod");
            assert_eq!(listed_body["items"][0]["containerCount"], 1);
            assert_eq!(
                listed_body["items"][0]["containers"][0]["name"],
                "sandbox-demo"
            );
            assert_eq!(inspected.status, 200);
            assert_eq!(inspected_body["api"], "cratebay.pod.inspect.v1");
            assert_eq!(inspected_body["item"]["containerCount"], 1);
            assert_eq!(inspected_body["item"]["containers"][0]["id"], pending.id);
        }

        #[test]
        fn native_pod_remove_requires_force_when_pending_container_uses_pod() {
            let state = test_state_without_external_frontend();
            let (_, pod_payload) = native_pod_create_request(&json!({
                "name": "demo-pod",
                "driver": "bridge"
            }))
            .expect("pod create payload");
            create_managed_network(&state.config, &pod_payload).expect("managed pod network");
            let (name, create_payload, _) = native_create_request(&json!({
                "name": "pod-user",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "pod": "demo-pod",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &create_payload);
            let pending_id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &pending_id, &pending_name, pending)
                .expect("store pending pod user");

            let removed = native_remove_pod(&state, &state.config, "demo-pod".to_string(), b"{}");
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove conflict json");
            let details = &body["details"];

            assert_eq!(removed.status, 409);
            assert_eq!(body["message"], "pod is in use by CrateBay containers");
            assert_eq!(details["api"], "cratebay.pod.remove.v1");
            assert_eq!(details["name"], "demo-pod");
            assert_eq!(details["forceRequired"], true);
            assert_eq!(details["containers"][0]["name"], "pod-user");
            assert!(super::network_registry_path("demo-pod").exists());
        }

        #[test]
        fn native_pod_force_remove_detaches_pending_container() {
            let state = test_state_without_external_frontend();
            let (_, pod_payload) = native_pod_create_request(&json!({
                "name": "demo-pod",
                "driver": "bridge"
            }))
            .expect("pod create payload");
            create_managed_network(&state.config, &pod_payload).expect("managed pod network");
            let (name, create_payload, _) = native_create_request(&json!({
                "name": "pod-user",
                "image": "alpine:3.20",
                "command": "sleep 60",
                "pod": "demo-pod",
            }))
            .expect("native create payload");
            let pending = pending_from_create_payload(&name, &create_payload);
            let pending_id = pending.id.clone();
            let pending_name = pending.name.clone();
            store_pending_container(&state, &pending_id, &pending_name, pending)
                .expect("store pending pod user");

            let removed = native_remove_pod(
                &state,
                &state.config,
                "demo-pod".to_string(),
                br#"{ "force": true }"#,
            );
            let body: serde_json::Value =
                serde_json::from_slice(&removed.body).expect("native remove json");
            let pending = pending_container(&state, "pod-user").expect("pending pod user");

            assert_eq!(removed.status, 200);
            assert_eq!(body["force"], true);
            assert!(!super::network_registry_path("demo-pod").exists());
            assert_eq!(pending.network, None);
            assert!(pending.netns_name.is_none());
            assert!(pending.netns_path.is_none());
        }

        #[test]
        fn native_pod_attach_and_detach_update_pending_container_membership() {
            let state = test_state_without_external_frontend();
            let (_, pod_payload) = native_pod_create_request(&json!({
                "name": "demo-pod",
                "driver": "bridge"
            }))
            .expect("pod create payload");
            create_managed_network(&state.config, &pod_payload).expect("managed pod network");

            let mut query = HashMap::new();
            query.insert("name".to_string(), "sandbox-demo".to_string());
            let create = create_container(
                &state,
                &query,
                br#"{
                    "Image": "alpine:3.20",
                    "Cmd": ["sleep", "60"]
                }"#,
            );
            assert_eq!(create.status, 201);
            assert_eq!(
                pending_container(&state, "sandbox-demo")
                    .expect("pending before attach")
                    .network,
                None
            );

            let attached = native_attach_container_to_pod(
                &state,
                "demo-pod".to_string(),
                br#"{ "container": "sandbox-demo" }"#,
            );
            let attached_body: serde_json::Value =
                serde_json::from_slice(&attached.body).expect("attach json");
            assert_eq!(attached.status, 200);
            assert_eq!(attached_body["api"], "cratebay.pod.attach.v1");
            assert_eq!(attached_body["attached"], true);
            assert_eq!(
                pending_container(&state, "sandbox-demo")
                    .expect("pending after attach")
                    .network
                    .as_deref(),
                Some("demo-pod")
            );

            let detached = native_detach_container_from_pod(
                &state,
                "demo-pod".to_string(),
                br#"{ "container": "sandbox-demo", "force": true }"#,
            );
            let detached_body: serde_json::Value =
                serde_json::from_slice(&detached.body).expect("detach json");
            assert_eq!(detached.status, 200);
            assert_eq!(detached_body["api"], "cratebay.pod.detach.v1");
            assert_eq!(detached_body["detached"], true);
            assert_eq!(detached_body["force"], true);
            assert_eq!(
                pending_container(&state, "sandbox-demo")
                    .expect("pending after detach")
                    .network,
                None
            );
        }

        #[test]
        fn cratebay_pod_network_filter_requires_pod_label() {
            let pod = normalize_network_inspect(
                json!({
                    "Name": "demo-pod",
                    "Id": "pod123",
                    "Labels": { "com.cratebay.pod": "true" }
                }),
                "demo-pod",
            );
            let regular = normalize_network_inspect(
                json!({
                    "Name": "bridge",
                    "Id": "bridge",
                    "Labels": {}
                }),
                "bridge",
            );

            assert!(is_cratebay_pod_network(&pod));
            assert!(!is_cratebay_pod_network(&regular));
        }

        #[test]
        fn parses_last_exit_code_from_ctr_wait_output() {
            assert_eq!(parse_exit_code(b"66\n"), Some(66));
            assert_eq!(parse_exit_code(b"container-name 66\n"), Some(66));
        }

        #[test]
        fn parses_sizes_and_dates_without_external_crates() {
            assert_eq!(parse_human_size("1.5 MiB"), Some(1_572_864));
            assert_eq!(unix_seconds_to_utc(1_609_459_200), (2021, 1, 1, 0, 0, 0));
        }

        #[test]
        fn filters_networks_by_cratebay_pod_label() {
            let network = normalize_network_inspect(
                json!({
                    "Name": "demo-pod",
                    "ID": "abc",
                    "Driver": "bridge",
                    "Labels": { "com.cratebay.pod": "true" }
                }),
                "demo-pod",
            );
            let filters =
                parse_filters(Some(&r#"{"label":["com.cratebay.pod=true"]}"#.to_string()));
            assert!(network_matches_filters(&network, &filters));

            let filters =
                parse_filters(Some(&r#"{"label":["com.cratebay.pod=false"]}"#.to_string()));
            assert!(!network_matches_filters(&network, &filters));
        }

        #[test]
        fn builds_network_connect_and_disconnect_args() {
            let payload = json!({
                "Container": "container-1",
                "EndpointConfig": {
                    "Aliases": ["api", "worker"],
                    "IPAMConfig": {
                        "IPv4Address": "10.88.0.10",
                        "IPv6Address": "fd00::10"
                    }
                },
                "Force": true
            });
            assert_eq!(
                build_network_connect_args("pod-a", "container-1", &payload),
                vec![
                    "network",
                    "connect",
                    "--ip",
                    "10.88.0.10",
                    "--ip6",
                    "fd00::10",
                    "--alias",
                    "api",
                    "--alias",
                    "worker",
                    "pod-a",
                    "container-1",
                ]
            );
            assert_eq!(
                build_network_disconnect_args("pod-a", "container-1", &payload),
                vec!["network", "disconnect", "--force", "pod-a", "container-1"]
            );
        }
    }
}
