use crate::hypervisor::{HypervisorError, VmInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Default)]
struct VmStoreFile {
    version: u32,
    #[serde(default)]
    vms: Vec<VmInfo>,
}

#[derive(Debug, Clone)]
pub struct VmStore {
    path: PathBuf,
}

impl Default for VmStore {
    fn default() -> Self {
        Self::new()
    }
}

impl VmStore {
    pub fn new() -> Self {
        let path = config_dir().join("vms.json");
        Self { path }
    }

    pub fn load_vms(&self) -> Result<Vec<VmInfo>, HypervisorError> {
        if !self.path.exists() {
            return Ok(vec![]);
        }

        let content = std::fs::read_to_string(&self.path)?;
        let mut file: VmStoreFile =
            serde_json::from_str(&content).map_err(|e| HypervisorError::Storage(e.to_string()))?;

        if file.version == 0 {
            file.version = 1;
        }

        // De-dupe by id (last one wins).
        let mut by_id: HashMap<String, VmInfo> = HashMap::new();
        for vm in file.vms {
            by_id.insert(vm.id.clone(), vm);
        }

        Ok(by_id.into_values().collect())
    }

    pub fn save_vms(&self, vms: &[VmInfo]) -> Result<(), HypervisorError> {
        let file = VmStoreFile {
            version: 1,
            vms: vms.to_vec(),
        };

        let json = serde_json::to_vec_pretty(&file)
            .map_err(|e| HypervisorError::Storage(e.to_string()))?;
        write_atomic(&self.path, &json)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn next_id_for_prefix(vms: &[VmInfo], prefix: &str) -> u64 {
    vms.iter()
        .filter_map(|vm| vm.id.strip_prefix(prefix))
        .filter_map(|rest| rest.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .max(1)
}

/// Path to the console log file for a given VM.
pub fn vm_console_log_path(vm_id: &str) -> PathBuf {
    data_dir().join("vms").join(vm_id).join("console.log")
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGOBAY_CONFIG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("com.cargobay.app");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("cargobay");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("cargobay");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("cargobay");
        }
    }

    std::env::temp_dir().join("cargobay")
}

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGOBAY_DATA_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("cargobay");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("cargobay");
        }
    }

    // Default: same as config dir (macOS/Windows per docs).
    config_dir()
}

pub fn log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGOBAY_LOG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        data_dir()
    }

    #[cfg(not(target_os = "linux"))]
    {
        config_dir()
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("vms.json");
    let unique = format!(
        "{}.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        file_name
    );
    let tmp_path = dir.join(format!(".{}.tmp", unique));

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }

    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Windows fails rename if destination exists.
            if path.exists() {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&tmp_path, path).map_err(|_| e)?;
                return Ok(());
            }
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hypervisor::VmState;
    use std::ffi::OsString;

    /// RAII guard that sets an env var and restores it on drop.
    struct EnvGuard {
        key: String,
        prev: Option<OsString>,
    }

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let prev = std::env::var_os(key);
            std::env::set_var(key, value);
            Self {
                key: key.to_string(),
                prev,
            }
        }

        fn remove(key: &str) -> Self {
            let prev = std::env::var_os(key);
            std::env::remove_var(key);
            Self {
                key: key.to_string(),
                prev,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.prev.take() {
                Some(v) => std::env::set_var(&self.key, v),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    // -----------------------------------------------------------------------
    // config_dir tests
    // -----------------------------------------------------------------------

    #[test]
    fn config_dir_respects_env_override() {
        let _g = EnvGuard::set("CARGOBAY_CONFIG_DIR", "/tmp/cargobay-test-config");
        assert_eq!(config_dir(), PathBuf::from("/tmp/cargobay-test-config"));
    }

    #[test]
    fn config_dir_returns_nonempty_path_without_override() {
        let _g = EnvGuard::remove("CARGOBAY_CONFIG_DIR");
        let dir = config_dir();
        assert!(
            !dir.as_os_str().is_empty(),
            "config_dir should not be empty"
        );
    }

    // -----------------------------------------------------------------------
    // data_dir tests
    // -----------------------------------------------------------------------

    #[test]
    fn data_dir_respects_env_override() {
        let _g = EnvGuard::set("CARGOBAY_DATA_DIR", "/tmp/cargobay-test-data");
        assert_eq!(data_dir(), PathBuf::from("/tmp/cargobay-test-data"));
    }

    #[test]
    fn data_dir_returns_nonempty_path_without_override() {
        let _g1 = EnvGuard::remove("CARGOBAY_DATA_DIR");
        let _g2 = EnvGuard::remove("CARGOBAY_CONFIG_DIR");
        let dir = data_dir();
        assert!(!dir.as_os_str().is_empty(), "data_dir should not be empty");
    }

    // -----------------------------------------------------------------------
    // log_dir tests
    // -----------------------------------------------------------------------

    #[test]
    fn log_dir_respects_env_override() {
        let _g = EnvGuard::set("CARGOBAY_LOG_DIR", "/tmp/cargobay-test-logs");
        assert_eq!(log_dir(), PathBuf::from("/tmp/cargobay-test-logs"));
    }

    #[test]
    fn log_dir_returns_nonempty_path_without_override() {
        let _g1 = EnvGuard::remove("CARGOBAY_LOG_DIR");
        let _g2 = EnvGuard::remove("CARGOBAY_CONFIG_DIR");
        let _g3 = EnvGuard::remove("CARGOBAY_DATA_DIR");
        let dir = log_dir();
        assert!(!dir.as_os_str().is_empty(), "log_dir should not be empty");
    }

    // -----------------------------------------------------------------------
    // vm_console_log_path tests
    // -----------------------------------------------------------------------

    #[test]
    fn vm_console_log_path_contains_vm_id() {
        let _g = EnvGuard::set("CARGOBAY_DATA_DIR", "/tmp/cb-test");
        let path = vm_console_log_path("vm-42");
        assert_eq!(path, PathBuf::from("/tmp/cb-test/vms/vm-42/console.log"));
    }

    #[test]
    fn vm_console_log_path_different_ids_produce_different_paths() {
        let _g = EnvGuard::set("CARGOBAY_DATA_DIR", "/tmp/cb-test");
        assert_ne!(vm_console_log_path("a"), vm_console_log_path("b"));
    }

    // -----------------------------------------------------------------------
    // next_id_for_prefix tests
    // -----------------------------------------------------------------------

    #[test]
    fn next_id_for_prefix_empty_list_returns_one() {
        assert_eq!(next_id_for_prefix(&[], "stub-"), 1);
    }

    #[test]
    fn next_id_for_prefix_increments_past_max() {
        let vms = vec![
            VmInfo {
                id: "stub-1".into(),
                name: "a".into(),
                state: VmState::Stopped,
                cpus: 1,
                memory_mb: 512,
                disk_gb: 10,
                rosetta_enabled: false,
                shared_dirs: vec![],
                port_forwards: vec![],
            },
            VmInfo {
                id: "stub-3".into(),
                name: "b".into(),
                state: VmState::Stopped,
                cpus: 1,
                memory_mb: 512,
                disk_gb: 10,
                rosetta_enabled: false,
                shared_dirs: vec![],
                port_forwards: vec![],
            },
        ];
        assert_eq!(next_id_for_prefix(&vms, "stub-"), 4);
    }

    #[test]
    fn next_id_for_prefix_ignores_other_prefixes() {
        let vms = vec![VmInfo {
            id: "vz-100".into(),
            name: "x".into(),
            state: VmState::Stopped,
            cpus: 1,
            memory_mb: 512,
            disk_gb: 10,
            rosetta_enabled: false,
            shared_dirs: vec![],
            port_forwards: vec![],
        }];
        assert_eq!(next_id_for_prefix(&vms, "stub-"), 1);
    }

    // -----------------------------------------------------------------------
    // VmStore round-trip tests
    // -----------------------------------------------------------------------

    #[test]
    fn vm_store_save_and_load_round_trip() {
        let tmp = std::env::temp_dir().join(format!(
            "cargobay-store-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let store = VmStore {
            path: tmp.join("vms.json"),
        };

        let vms = vec![VmInfo {
            id: "stub-1".into(),
            name: "test-vm".into(),
            state: VmState::Running,
            cpus: 4,
            memory_mb: 4096,
            disk_gb: 50,
            rosetta_enabled: true,
            shared_dirs: vec![],
            port_forwards: vec![],
        }];

        store.save_vms(&vms).unwrap();
        let loaded = store.load_vms().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "stub-1");
        assert_eq!(loaded[0].name, "test-vm");
        assert_eq!(loaded[0].cpus, 4);
        assert_eq!(loaded[0].memory_mb, 4096);

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn vm_store_load_returns_empty_when_file_missing() {
        let store = VmStore {
            path: PathBuf::from("/tmp/cargobay-nonexistent-dir/vms.json"),
        };
        let vms = store.load_vms().unwrap();
        assert!(vms.is_empty());
    }

    #[test]
    fn vm_store_deduplicates_by_id() {
        let tmp = std::env::temp_dir().join(format!(
            "cargobay-dedup-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        // Write a file with duplicate IDs manually.
        let json = r#"{"version":1,"vms":[
            {"id":"stub-1","name":"first","state":"Stopped","cpus":1,"memory_mb":512,"disk_gb":10,"rosetta_enabled":false,"shared_dirs":[],"port_forwards":[]},
            {"id":"stub-1","name":"second","state":"Running","cpus":2,"memory_mb":1024,"disk_gb":20,"rosetta_enabled":false,"shared_dirs":[],"port_forwards":[]}
        ]}"#;
        let path = tmp.join("vms.json");
        std::fs::write(&path, json).unwrap();

        let store = VmStore { path };
        let vms = store.load_vms().unwrap();
        assert_eq!(vms.len(), 1, "duplicate IDs should be de-duped");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // -----------------------------------------------------------------------
    // write_atomic tests
    // -----------------------------------------------------------------------

    #[test]
    fn write_atomic_creates_file_and_dirs() {
        let tmp = std::env::temp_dir().join(format!(
            "cargobay-atomic-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = tmp.join("sub").join("test.txt");

        write_atomic(&path, b"hello").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("hello"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
