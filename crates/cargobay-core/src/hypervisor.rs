use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HypervisorError {
    #[error("VM creation failed: {0}")]
    CreateFailed(String),
    #[error("VM not found: {0}")]
    NotFound(String),
    #[error("unsupported platform")]
    Unsupported,
    #[error("Rosetta not available: {0}")]
    RosettaUnavailable(String),
    #[error("VirtioFS error: {0}")]
    VirtioFsError(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Unified hypervisor interface across platforms.
pub trait Hypervisor: Send + Sync {
    fn create_vm(&self, config: VmConfig) -> Result<String, HypervisorError>;
    fn start_vm(&self, id: &str) -> Result<(), HypervisorError>;
    fn stop_vm(&self, id: &str) -> Result<(), HypervisorError>;
    fn delete_vm(&self, id: &str) -> Result<(), HypervisorError>;
    fn list_vms(&self) -> Result<Vec<VmInfo>, HypervisorError>;

    /// Check if Rosetta x86_64 translation is available on this platform.
    fn rosetta_available(&self) -> bool {
        false
    }

    /// Mount a host directory into the VM via VirtioFS.
    fn mount_virtiofs(
        &self,
        _vm_id: &str,
        _share: &SharedDirectory,
    ) -> Result<(), HypervisorError> {
        Err(HypervisorError::VirtioFsError(
            "VirtioFS not supported on this platform".into(),
        ))
    }

    /// Unmount a VirtioFS share from the VM.
    fn unmount_virtiofs(&self, _vm_id: &str, _tag: &str) -> Result<(), HypervisorError> {
        Err(HypervisorError::VirtioFsError(
            "VirtioFS not supported on this platform".into(),
        ))
    }

    /// List active VirtioFS mounts for a VM.
    fn list_virtiofs_mounts(&self, _vm_id: &str) -> Result<Vec<SharedDirectory>, HypervisorError> {
        Ok(vec![])
    }

    /// Add a port forward rule to a VM (persisted in the VM store).
    fn add_port_forward(
        &self,
        _vm_id: &str,
        _pf: &PortForward,
    ) -> Result<(), HypervisorError> {
        Err(HypervisorError::Unsupported)
    }

    /// Remove a port forward rule from a VM.
    fn remove_port_forward(
        &self,
        _vm_id: &str,
        _host_port: u16,
    ) -> Result<(), HypervisorError> {
        Err(HypervisorError::Unsupported)
    }

    /// List port forwards for a VM.
    fn list_port_forwards(&self, _vm_id: &str) -> Result<Vec<PortForward>, HypervisorError> {
        Ok(vec![])
    }
}

/// A single host-port to guest-port forwarding rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForward {
    pub host_port: u16,
    pub guest_port: u16,
    /// "tcp" or "udp"
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub name: String,
    pub cpus: u32,
    pub memory_mb: u64,
    pub disk_gb: u64,
    /// Enable Rosetta x86_64 translation (macOS Apple Silicon only).
    pub rosetta: bool,
    /// Directories to share via VirtioFS.
    pub shared_dirs: Vec<SharedDirectory>,
    /// Port forwards from host to guest.
    #[serde(default)]
    pub port_forwards: Vec<PortForward>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            name: "default".into(),
            cpus: 2,
            memory_mb: 2048,
            disk_gb: 20,
            rosetta: false,
            shared_dirs: vec![],
            port_forwards: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedDirectory {
    /// Tag used to identify the mount inside the VM.
    pub tag: String,
    /// Host path to share.
    pub host_path: String,
    /// Guest mount point (e.g., /mnt/host).
    pub guest_path: String,
    /// Read-only mount.
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub cpus: u32,
    pub memory_mb: u64,
    #[serde(default = "default_disk_gb")]
    pub disk_gb: u64,
    /// Whether Rosetta x86_64 translation is enabled.
    pub rosetta_enabled: bool,
    /// Active VirtioFS mounts.
    pub shared_dirs: Vec<SharedDirectory>,
    /// Active port forwards.
    #[serde(default)]
    pub port_forwards: Vec<PortForward>,
}

fn default_disk_gb() -> u64 {
    20
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VmState {
    Running,
    Stopped,
    Creating,
}
