# Built-in Container Runtime Specification

> Version: 1.3.1 | Last Updated: 2026-05-31 | Author: runtime-dev

---

## Table of Contents

1. [Design Goal](#1-design-goal)
2. [Platform Strategies](#2-platform-strategies)
3. [Runtime Lifecycle](#3-runtime-lifecycle)
4. [Engine Compatibility Endpoint Exposure](#4-engine-compatibility-endpoint-exposure)
5. [VirtioFS / Shared Directory](#5-virtiofs--shared-directory)
6. [Port Forwarding](#6-port-forwarding)
7. [Resource Management](#7-resource-management)
8. [First-Run Experience](#8-first-run-experience)
9. [Runtime Health Check](#9-runtime-health-check)
10. [Explicit Docker Host Compatibility](#10-explicit-docker-host-compatibility)
11. [Platform-Specific Implementation](#11-platform-specific-implementation)

---

## 1. Design Goal

### Zero-Dependency Container Runtime

CrateBay ships a **built-in container runtime** so that users can install the application and immediately start creating and managing containers — without installing Docker Desktop, Colima, or any other external tool.

**Note:** External Docker-compatible endpoints can be used for compatibility
and diagnostics, but only when explicitly selected by the operator.

### 1.1 Product Runtime Strategy

CrateBay has a **single runtime roadmap**:

- The **built-in runtime** is the **primary product path** across macOS, Linux, and Windows.
- External Docker-compatible hosts are a compatibility override, not a co-equal roadmap track.
- Container, image, pod, network, volume, exec, logs, and terminal management MUST prefer the **native CrateBay Engine API** (`cratebay.engine.v1`) backed by containerd/runc/CNI.
- The Docker-compatible API/socket remains a compatibility endpoint for existing clients and explicit host overrides; it is not the runtime backend.
- When runtime-related issues occur, contributors SHOULD fix the built-in runtime path first before expanding external-host behavior.
- External-engine-specific product features, product flows, or architectural branches are **out of scope** unless explicitly approved by a human maintainer.

Explicit Docker-compatible hosts remain useful for:

1. temporary recovery when the built-in runtime is unavailable,
2. development or CI environments needing a quick Docker-compatible engine,
3. explicitly requested host or enterprise constraints.

This strategy keeps CrateBay aligned with its zero-dependency product goal while preserving a pragmatic compatibility fallback.

### Key Requirements

| Requirement | Description |
|-------------|-------------|
| **Zero external dependencies** | No Docker, no Colima, no manual configuration |
| **Automatic provisioning** | First launch downloads and configures the runtime automatically |
| **Transparent to users** | Users interact with images, containers, pods, and runtime controls through the desktop app or CLI |
| **Native performance** | Use platform-native virtualization (VZ.framework, KVM, WSL2) for near-native speed |
| **Small footprint** | VM image < 500 MB download, < 1 GB disk after provisioning |
| **Explicit compatibility** | Use external Docker-compatible endpoints only when `DOCKER_HOST` or `--docker-host` is set |

### Architecture Overview

```
CrateBay App
     │
     ├── Explicit DOCKER_HOST / --docker-host? ──→ Use that endpoint
     │                                             (compatibility mode)
     │
     └── Default path ──→ Start / reuse built-in runtime
                           │
             ┌─────────────┼─────────────┐
             │             │             │
         macOS          Linux        Windows
             │             │             │
      VZ.framework     KVM/QEMU       WSL2
             │             │             │
       Linux VM       Linux VM     WSL2 Distro
             │             │             │
      CrateBay Engine CrateBay Engine CrateBay Engine
      containerd/runc containerd/runc containerd/runc
             │             │             │
      Engine socket   TCP/Socket     TCP/Pipe
             │             │             │
             └─────────────┼─────────────┘
                           │
                  Native API first;
                  Docker-compatible endpoint for legacy clients
```

---

## 2. Platform Strategies

### 2.1 macOS: Virtualization.framework

| Aspect | Detail |
|--------|--------|
| **Hypervisor** | Apple Virtualization.framework (VZ) |
| **VM Type** | `VZVirtualMachine` with `VZLinuxBootLoader` |
| **Guest OS** | Alpine Linux (minimal, ~150 MB) |
| **Engine** | CrateBay Engine adapter backed by containerd + runc + CNI |
| **File Sharing** | VirtioFS (`VZVirtioFileSystemDeviceConfiguration`) |
| **Networking** | NAT via `VZNATNetworkDeviceAttachment` |
| **Requirements** | macOS 13+ (Ventura), Apple Silicon or Intel |

```
macOS Host
├── CrateBay.app
│   └── Rust Backend
│       └── VZ.framework API calls
│           └── VZVirtualMachine
│               ├── VZLinuxBootLoader (vmlinuz + initrd)
│               ├── VZVirtioBlockStorageDevice (rootfs.img)
│               ├── VZVirtioFileSystemDevice (shared dirs)
│               ├── (optional) VZVirtioSocketDevice (vsock)
│               └── VZNATNetworkDeviceAttachment
│                   └── Alpine Linux
│                       ├── containerd + runc + CNI
│                       └── CrateBay Engine adapter
│                           └── /run/cratebay/engine.sock
│                               └── Exposed via reverse TCP (default) or vsock (optional) → host socket
```

### 2.2 Linux: KVM/QEMU

| Aspect | Detail |
|--------|--------|
| **Hypervisor** | KVM (hardware) + QEMU (userspace) |
| **VM Type** | Lightweight QEMU VM with KVM acceleration |
| **Guest OS** | Alpine Linux (same image as macOS) |
| **Engine** | CrateBay Engine adapter backed by containerd + runc + CNI |
| **File Sharing** | VirtioFS via virtiofsd |
| **Networking** | User-mode networking (SLIRP) or TAP |
| **Requirements** | Linux kernel 5.10+, KVM support (`/dev/kvm`) |

```
Linux Host
├── CrateBay binary
│   └── Rust Backend
│       └── QEMU process management
│           └── qemu-system-x86_64 (or aarch64)
│               ├── -enable-kvm
│               ├── -kernel vmlinuz -initrd initrd
│               ├── -drive file=rootfs.img
│               ├── -chardev socket for CrateBay Engine compatibility endpoint
│               └── -virtfs for shared directories
│                   └── Alpine Linux
│                       ├── containerd + runc + CNI
│                       └── CrateBay Engine adapter
│                           └── Exposed via compatibility socket
```

### 2.3 Windows: WSL2

| Aspect | Detail |
|--------|--------|
| **Hypervisor** | Hyper-V (via WSL2) |
| **VM Type** | WSL2 lightweight utility VM |
| **Distro** | Custom WSL2 distro (Alpine-based) |
| **Engine** | CrateBay Engine adapter backed by containerd + runc + CNI inside WSL2 |
| **File Sharing** | Plan 9 (9P) protocol (built into WSL2) |
| **Networking** | WSL2 NAT networking |
| **Requirements** | Windows 10 21H2+ or Windows 11, WSL2 enabled |

```
Windows Host
├── CrateBay.exe
│   └── Rust Backend
│       └── WSL2 management (wsl.exe commands)
│           └── CrateBay WSL2 Distro
│               ├── Alpine Linux
│               ├── containerd + runc + CNI
│               ├── CrateBay Engine adapter
│               └── /run/cratebay/engine.sock
│                   └── Exposed via:
│                       ├── socat → TCP localhost:2375
│                       └── or WSL2 interop socket
```

---

## 3. Runtime Lifecycle

### 3.1 State Machine

```
                    ┌─────────┐
                    │  NONE   │ (no runtime detected)
                    └────┬────┘
                         │ provision()
                    ┌────▼────┐
                    │PROVISION│ (downloading VM image)
                    │  ING    │
                    └────┬────┘
                         │ complete
                    ┌────▼────┐
                    │PROVISION│ (image ready, not started)
                    │   ED    │
                    └────┬────┘
                         │ start()
                    ┌────▼────┐
                    │STARTING │ (VM booting)
                    └────┬────┘
                         │ health_check() passes
                    ┌────▼────┐
              ┌────→│  READY  │ (CrateBay Engine available)
              │     └────┬────┘
              │          │ stop()
              │     ┌────▼────┐
              │     │STOPPING │
              │     └────┬────┘
              │          │ stopped
              │     ┌────▼────┐
              └─────│ STOPPED │
                    └─────────┘
```

### 3.2 Lifecycle Operations

```rust
/// Runtime lifecycle trait (platform-agnostic)
pub trait RuntimeManager: Send + Sync {
    /// Detect current runtime state
    async fn detect(&self) -> Result<RuntimeState, AppError>;

    /// Download and prepare VM image (first run).
    /// Uses `Box<dyn Fn>` instead of `impl Fn` for async_trait
    /// object safety (required for `Arc<dyn RuntimeManager>`).
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError>;

    /// Start the runtime VM
    async fn start(&self) -> Result<(), AppError>;

    /// Stop the runtime VM gracefully
    async fn stop(&self) -> Result<(), AppError>;

    /// Check if runtime is healthy and CrateBay Engine is responsive
    async fn health_check(&self) -> Result<HealthStatus, AppError>;

    /// Get the host-exposed CrateBay Engine socket path.
    fn engine_socket_path(&self) -> PathBuf;

    /// Compatibility alias for older callers.
    fn docker_socket_path(&self) -> PathBuf {
        self.engine_socket_path()
    }

    /// Get current resource usage
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError>;
}

#[derive(Debug, Clone, Serialize)]
pub enum RuntimeState {
    None,           // No runtime, needs provisioning
    Provisioning,   // Downloading VM image
    Provisioned,    // Image ready, not running
    Starting,       // VM is booting
    Ready,          // CrateBay Engine is available
    Stopping,       // Shutting down
    Stopped,        // VM stopped
    Error(String),  // Runtime error
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvisionProgress {
    pub stage: String,         // "downloading", "extracting", "configuring"
    pub percent: f32,          // 0.0 - 100.0
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub message: String,
}
```

### 3.3 Automatic Start Flow

```
App Launch / First Engine Operation (GUI or CLI)
    │
    ├── engine::ensure_engine_contract()
    │   ├── Cross-process lock (engine.lock)
    │   ├── Reuse already-running built-in runtime if the native contract responds
    │   └── Otherwise provision/start built-in runtime
    │
    ├── runtime.detect()
    │   ├── RuntimeState::None → runtime.provision() → runtime.start()
    │   ├── RuntimeState::Provisioned → runtime.start()
    │   ├── RuntimeState::Stopped → runtime.start()
    │   └── RuntimeState::Ready → already good
    │
    └── Wait for CrateBay Engine responsiveness (max 45s)
        ├── Native Engine contract responsive → READY
        ├── Compatibility endpoint responsive → legacy clients can connect
        └── Timeout / error → surface error to user
```

`engine::ensure_engine_contract()` is the primary built-in Engine bring-up path
for native GUI and CLI management commands. The legacy
`engine::ensure_docker()`/`ensure_engine_compatibility()` path remains available
for compatibility clients, but compatibility readiness is not the gate for
native `/cratebay/*` management calls. External Docker-compatible engines are
handled outside this path by explicit host selection (`--docker-host`,
`--engine-host`, or `DOCKER_HOST`), so the default product behavior remains
deterministic.

Runtime managers must treat `RuntimeState::Ready` as a strict native contract
state: `/cratebay/engine` must report `adapter.api = cratebay.engine.v1` and
`kind = cratebay-containerd`. Compatibility `_ping` may be reported for legacy
clients and may help choose a reachable Windows relay endpoint, but it must not
be the success gate for native Ready or native auto-start.

### 3.4 Concurrency & Lifetime

#### Cross-process mutual exclusion

CrateBay must support **GUI + CLI** being used concurrently. To avoid two processes
provisioning or starting the same VM at the same time, all runtime bring-up is
guarded by a **cross-process lock**:

- Lock file: `<host_engine_socket_dir>/engine.lock` (colocated with the host-exposed Engine socket; see §4.1)

When `CRATEBAY_DATA_DIR` is explicitly set, CrateBay uses a deterministic short socket path under the system temp directory (to avoid Unix socket path length limits and to isolate multiple runtimes). In that case, the lock file is colocated with that derived temp socket directory.
- Scope: provision + start + initial Engine wait loop
- Behavior: second process waits for the lock, then re-checks Engine availability

#### GUI exit does not stop the runtime

The built-in runtime is treated as a **long-lived engine**. Closing the GUI does
**not** automatically stop the runtime VM so that:

- CLI can continue to manage containers/images after GUI is closed
- Engine state survives app restarts (PID/socket recovery in runtime managers)

Users can stop the runtime explicitly via `runtime_stop` (or the equivalent CLI).

---

## 4. Engine Compatibility Endpoint Exposure

### 4.1 Socket Path Convention

| Platform | Default Socket Location |
|----------|------------------------|
| macOS | `~/.cratebay/runtime/engine.sock` |
| Linux | `~/.cratebay/runtime/engine.sock` |
| Windows | `\\.\pipe\cratebay-engine` or `localhost:2375` |

The canonical host-exposed path is named `engine.sock` and is served by the
CrateBay Engine adapter backed by containerd/runc/CNI. A `docker.sock`
symlink may be maintained beside it only as a Docker-compatible client alias.

**Socket path resolution (macOS/Linux):**

1. If `CRATEBAY_ENGINE_SOCKET_PATH` is set (non-empty), use it.
2. Else, if `CRATEBAY_DOCKER_SOCKET_PATH` is set (non-empty), use it as a legacy compatibility override.
3. Else, if `CRATEBAY_DATA_DIR` is explicitly set, use `/tmp/cratebay-<hash>/engine.sock` (deterministic hash derived from `CRATEBAY_DATA_DIR`).
4. Else, use `$HOME/.cratebay/runtime/engine.sock`.

The canonical host socket (`engine.sock`) may be a symlink pointing to a per-VM socket (`engine-<vm_id>.sock` or `engine-<vm_id>-<hash>.sock` when `CRATEBAY_DATA_DIR` is set) so multiple isolated runtimes do not collide. A colocated `docker.sock` symlink can point to the same target for external compatibility clients.

### 4.2 macOS: Engine Endpoint Forwarding Modes

macOS uses Apple's Virtualization.framework (VZ) to run the guest VM. CrateBay
Engine compatibility API access is exposed as a host Unix socket
(`~/.cratebay/runtime/engine.sock`) via one of the following forwarding modes:

#### 4.2.1 Architecture-Based Default Selection

| Architecture | Default Mode | Rationale |
|--------------|--------------|-----------|
| Apple Silicon (arm64) | **vsock** | VZ.framework has reliable vsock on arm64; lower latency |
| Intel (x86_64) | **reverse TCP** | Vsock stability issues on Intel; fallback to proven TCP path |

The mode can be overridden via `CRATEBAY_RUNTIME_SOCKET_FORWARD` environment
variable.

#### 4.2.2 Vsock Forwarding (Apple Silicon default)

Vsock (Virtio Socket) provides a direct host-guest communication channel via
`VZVirtioSocketDevice`. The host-side VZ runner listens on a vsock port and
proxies to the host Unix socket.

```
VM (guest)                          Host (macOS)
CrateBay Engine adapter             cratebay-vz runner
/run/cratebay/engine.sock           ~/.cratebay/runtime/engine.sock
       │                                      │
       └── cratebay-guest-agent ←── vsock ──→ host creates Unix socket
           listens vsock:{port}    AF_VSOCK   proxies ↔ vsock
```

**Flow:**
1. VZ runner creates a `VZVirtioSocketDevice` and starts listening on host vsock port
2. Host-side: for each Unix socket connection, VZ runner connects to guest vsock port
3. Guest-side: `cratebay-guest-agent --port {port} --engine-sock /run/cratebay/engine.sock` listens on vsock
4. Bidirectional proxy: host vsock ↔ guest CrateBay Engine compatibility socket

**Advantages:**
- Lower latency than TCP (no network stack overhead)
- No port conflicts with host network
- Reliable connection multiplexing

#### 4.2.3 Reverse TCP Forwarding (Intel default)

On Intel Macs or when vsock is disabled, the runtime uses reverse TCP
forwarding where the guest initiates the connection back to the host.

```
VM (guest)                             Host (macOS)
CrateBay Engine adapter                cratebay-vz runner
/run/cratebay/engine.sock              ~/.cratebay/runtime/engine.sock
       │                                       │
       └── cratebay-guest-agent ── TCP ─────→  tcp listener (0.0.0.0:6237)
           connect mode                        proxies ↔ unix socket
```

**Flow:**
1. VZ runner binds TCP listener on host (default `0.0.0.0:6237`)
2. VZ runner binds Unix socket at `~/.cratebay/runtime/engine.sock`
3. Guest-side: `cratebay-guest-agent --connect {host_gateway}:{port}` dials host TCP
4. Host-side: for each Unix socket client, wait for guest TCP connection (5s timeout)
5. Proxy: Unix socket client ↔ guest TCP ↔ CrateBay Engine compatibility socket

**Implementation Details (cratebay-vz `start_tcp_forward`):**
- Host binds both Unix socket and TCP listener
- Each Unix client acceptance triggers wait for guest reverse connection (30s timeout)
- Guest agent uses **concurrent worker model**: multiple reverse-TCP workers connect back in parallel
- HTTP request preface is rewritten with `Connection: close` so each reverse-TCP tunnel is single-shot and workers return to the pool after every request
- Uses bidirectional copy threads for efficient byte shuffling

**Concurrency Model (v1.3.0+):**

| Component | Model | Description |
|-----------|-------|-------------|
| Guest agent (`run_connect`) | Concurrent worker pool | Multiple reverse-TCP workers connect back in parallel; no single-worker serialization |
| Host VZ runner | Per-connection threads | Each Unix client + guest TCP pair handled in dedicated thread |
| HTTP handling | Single-shot per tunnel | `Connection: close` is injected in reverse TCP mode so the CrateBay Engine adapter closes the upstream connection and frees the worker |

#### 4.2.4 Configuration (environment variables)

| Variable | Purpose | Default |
|----------|---------|---------|
| `CRATEBAY_RUNTIME_SOCKET_FORWARD` | Engine compatibility endpoint forwarding mode: `vsock`, `tcp`, or `auto` | `auto` (arm64→vsock, x86_64→tcp) |
| `CRATEBAY_ENGINE_PROXY_PORT` | Port used by vsock (guest port) or TCP (host listener) | `6237` (if `CRATEBAY_DATA_DIR` is set and no explicit override is provided, a deterministic high port in `42000-51999` is derived) |
| `CRATEBAY_ENGINE_VSOCK_PORT` | Legacy alias for `CRATEBAY_ENGINE_PROXY_PORT` | — |
| `CRATEBAY_DOCKER_PROXY_PORT` | Legacy alias for `CRATEBAY_ENGINE_PROXY_PORT` | — |
| `CRATEBAY_DOCKER_VSOCK_PORT` | Legacy alias for `CRATEBAY_ENGINE_PROXY_PORT` | — |
| `CRATEBAY_ENGINE_SOCKET_PATH` | Override the host-exposed CrateBay Engine Unix socket path (macOS/Linux). | — |
| `CRATEBAY_DOCKER_SOCKET_PATH` | Legacy alias for `CRATEBAY_ENGINE_SOCKET_PATH`. | — |

#### 4.2.5 Guest Agent Modes

`cratebay-guest-agent` supports three modes corresponding to host forwarding:

| Mode | Command | Use Case |
|------|---------|----------|
| vsock (listen) | `--port {port}` | Default for vsock forwarding (arm64) |
| tcp (listen) | `--tcp --listen 0.0.0.0:{port}` | Legacy TCP listen mode |
| connect (dial-back) | `--connect {host}:{port}` | Reverse TCP for Intel Macs |

In all modes, native CrateBay automation should call the Engine API first. Legacy
Docker-compatible clients can still connect to the host Unix socket path.

**Kernel cmdline coupling (v1.3.0+):** The host passes `cratebay_engine_proxy_port=<port>` via the VM kernel cmdline so the guest init script can start `cratebay-guest-agent` with the exact same port (including derived ports when `CRATEBAY_DATA_DIR` is set). During upgrades, the host may also pass the legacy `cratebay_docker_proxy_port=<port>` alias for older runtime images.

### 4.3 Linux: QEMU Socket Forwarding

```
VM (guest)                          Host (Linux)
CrateBay Engine adapter             CrateBay Backend
/run/cratebay/engine.sock           ~/.cratebay/runtime/engine.sock
       │                                      │
       └── QEMU -chardev socket  ──────────→  Unix socket
           forwarding                          on host
```

### 4.4 Windows: TCP or Named Pipe

```
WSL2 Distro                         Host (Windows)
CrateBay Engine adapter              CrateBay Backend
/run/cratebay/engine.sock            \\.\pipe\cratebay-engine
       │                                      │
       └── socat TCP-LISTEN:2375  ──────────→  localhost:2375
           or named pipe proxy                  or named pipe
```

---

## 5. VirtioFS / Shared Directory

### 5.1 Purpose

Users may want to mount host directories into containers (e.g., project source code). The runtime provides transparent file sharing between host and VM.

### 5.2 Shared Directory Structure

```
Host: ~/.cratebay/shared/         → VM: /mnt/host/
Host: ~/Projects/                 → VM: /mnt/projects/ (user-configurable)
```

### 5.3 Platform Implementation

| Platform | Technology | Performance |
|----------|-----------|-------------|
| macOS | VirtioFS (`VZVirtioFileSystemDeviceConfiguration`) | Near-native |
| Linux | virtiofsd + VirtioFS | Near-native |
| Windows | WSL2 Plan 9 (9P) mount | Good (built into WSL2) |

### 5.4 Container Bind Mount Flow

```
User Request: "Create a container with ~/Projects/myapp mounted"
    │
    ├── Host path: ~/Projects/myapp
    │
    ├── VM shared dir: /mnt/projects/myapp (via VirtioFS)
    │
    └── Container bind mount: /mnt/projects/myapp → /workspace
        (Docker -v flag inside the VM)
```

---

## 6. Port Forwarding

### 6.1 Mechanism

Containers running inside the VM need their ports accessible from the host.

```
Host Browser                        VM                  Container
localhost:8080  ──→  VM NAT  ──→  0.0.0.0:8080  ──→  container:8080
```

### 6.2 Implementation

| Platform | Approach |
|----------|----------|
| macOS | VZ.framework NAT device automatically forwards ports |
| Linux | QEMU user-mode networking with `-hostfwd` flags, or dynamic port forwarding via socat |
| Windows | WSL2 localhost forwarding (automatic in Windows 11) |

### 6.3 Dynamic Port Forwarding

When a container exposes a port, the runtime detects it and sets up forwarding:

```rust
pub struct PortForward {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: Protocol, // TCP or UDP
}

/// Detect exposed ports from container config and set up forwarding
pub async fn setup_port_forwards(
    container_id: &str,
    ports: Vec<PortForward>,
) -> Result<(), AppError> { /* ... */ }
```

### 6.4 HTTP Proxy Bridge (Optional)

Some networks block direct egress from the runtime VM (e.g., pulling images from Docker Hub).
To support these environments, the runtime can be configured to use an HTTP proxy.

**Guest-side configuration**

The runtime VM reads a kernel cmdline parameter:

```
cratebay_http_proxy=<host:port>
```

When present, the guest configures HTTP(S) egress (containerd / CrateBay Engine adapter / package manager) to use the proxy.

**Host-side bridge (macOS VZ)**

On macOS, host-local proxies are often bound to `127.0.0.1` (e.g., Clash/V2Ray).
The VM cannot reach host loopback directly, so CrateBay can bridge the proxy by:

- Binding a host TCP listener (default `0.0.0.0:3128`)
- Forwarding it to the target proxy (e.g., `127.0.0.1:7897`)
- Pointing the guest proxy to the host bridge IP (default `192.168.64.1:3128`)

This is implemented via the `cratebay-vz` runner argument:

```
--host-tcp-forward 0.0.0.0:<bind_port>=<target_host>:<target_port>
```

**Configuration (environment variables)**

| Variable | Purpose | Default |
|----------|---------|---------|
| `CRATEBAY_RUNTIME_HTTP_PROXY` | Proxy endpoint. In passthrough mode: guest-reachable `<host:port>`. In bridge mode: host proxy target (falls back to `HTTPS_PROXY/HTTP_PROXY`). | — |
| `CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE` | Enable host proxy bridge mode (macOS). | `0` |
| `CRATEBAY_RUNTIME_HTTP_PROXY_BIND_HOST` | Host bind address for the bridge listener. | `0.0.0.0` |
| `CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT` | Host bind port for the bridge listener. | `3128` (if `CRATEBAY_DATA_DIR` is set and no explicit override is provided, a deterministic high port in `52000-61999` is derived) |
| `CRATEBAY_RUNTIME_HTTP_PROXY_GUEST_HOST` | Guest-visible host IP for the bridge (VZ shared network). | `192.168.64.1` |

---

## 7. Resource Management

### 7.1 Default Resource Allocation

| Resource | Default | Minimum | Maximum |
|----------|---------|---------|---------|
| CPU cores | 2 | 1 | Host cores - 1 |
| Memory | 2 GB | 1 GB | Host RAM / 2 |
| Disk | 20 GB (thin provisioned) | 10 GB | 100 GB |

### 7.2 Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of CPU cores allocated to the VM
    pub cpu_cores: u32,

    /// Memory allocated to the VM in MB
    pub memory_mb: u64,

    /// Maximum disk size in GB (thin provisioned)
    pub disk_gb: u32,

    /// Whether to auto-start runtime on app launch
    pub auto_start: bool,

    /// Shared directories (host_path → guest_mount_point)
    pub shared_dirs: Vec<SharedDir>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            cpu_cores: 2,
            memory_mb: 2048,
            disk_gb: 20,
            auto_start: true,
            shared_dirs: vec![],
        }
    }
}
```

### 7.3 Resource Monitoring

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ResourceUsage {
    pub cpu_percent: f32,       // VM or runner CPU usage (0-100+)
    pub memory_used_mb: u64,    // VM or runner memory in use
    pub memory_total_mb: u64,   // VM memory allocated
    pub disk_used_gb: f32,      // Host-allocated or guest-reported disk usage
    pub disk_total_gb: f32,     // VM disk capacity
    pub container_count: u32,   // CrateBay-managed containers inside VM
}
```

Resource usage is returned by desktop `runtime_status`, CLI
`cratebay runtime status`, and the `runtime` section of
`cratebay runtime diagnostics`.

Platform probes:

- macOS/VZ: CPU and memory are host-side `cratebay-vz` runner process
  approximations; disk usage is the host-allocated size of the sparse
  `disk.raw`; container count uses the native CrateBay Engine container list
  first and the compatibility endpoint only as a fallback.
- Linux/KVM: CPU and memory are QEMU process `/proc` metrics; disk usage is
  the host-allocated size of the sparse runtime disk image; container count
  uses the native CrateBay Engine container list first and the compatibility
  endpoint only as a fallback.
- Windows/WSL2: CPU and disk are sampled from guest `/proc/stat` and
  `df -B1`; memory is queried inside the WSL distro; container count uses the
  native CrateBay Engine container list first and the compatibility endpoint
  only as a fallback.

---

## 8. First-Run Experience

### 8.1 Flow

```
First App Launch
    │
    ├── detect() → RuntimeState::None
    │
    ├── Show "Setting up CrateBay Runtime" UI
    │   └── Progress bar with stages
    │
    ├── Stage 1: Download VM Image (~500 MB)
    │   ├── Source: GitHub Release assets / CDN
    │   ├── Show: "Downloading runtime image... 45%"
    │   └── Resume support for interrupted downloads
    │
    ├── Stage 2: Extract and Configure
    │   ├── Extract rootfs image
    │   ├── Configure containerd, runc, CNI, and CrateBay Engine adapter
    │   └── Show: "Configuring runtime..."
    │
    ├── Stage 3: Start Runtime
    │   ├── Boot VM
    │   ├── Start CrateBay Engine inside VM
    │   └── Show: "Starting container engine..."
    │
    ├── Stage 4: Health Check
    │   ├── Verify native Engine contract and compatibility endpoint are responsive
    │   └── Show: "Verifying..."
    │
    └── Complete: "CrateBay is ready!"
```

### 8.2 VM Image Distribution

| Platform | Image Format | Approximate Size |
|----------|-------------|-----------------|
| macOS (arm64) | vmlinuz + initrd + rootfs.img | ~400 MB |
| macOS (x86_64) | vmlinuz + initrd + rootfs.img | ~400 MB |
| Linux (x86_64) | vmlinuz + initrd + rootfs.qcow2 | ~400 MB |
| Linux (arm64) | vmlinuz + initrd + rootfs.qcow2 | ~400 MB |
| Windows | WSL2 tar export | ~350 MB |

### 8.4 Bundled Asset Layout (Desktop App)

For "install-and-run" UX, the desktop app bundle should include runtime assets
as resources so both **GUI and CLI** can provision the built-in runtime without
external Docker dependencies:

```
resources/
  runtime-images/    # kernel + initramfs (+ optional rootfs.img)
  runtime-linux/     # Linux-only: qemu-system-* + lib/ + share/qemu/
  runtime-wsl/       # Windows-only: WSL rootfs.tar for distro import
```

The runtime managers copy/install these assets into the per-user data dir on
first run during `runtime.provision()`.

### 8.3 Image Contents

The VM image is a minimal Alpine Linux with:
- containerd + ctr
- runc
- CrateBay Engine adapter (`cratebay.engine.v1`)
- CrateBay guest-agent for host/guest endpoint forwarding
- CNI plugins and CrateBay-managed CNI config
- Standard container networking (iptables, bridge/portmap/firewall plugins)
- Minimal shell utilities

No GUI, no unnecessary packages — optimized for size and boot speed.

Runtime asset builders and bundled runtime resource docs are covered by
`scripts/runtime-native-guard.sh`, which prevents the built-in runtime path from
adding real Docker daemon packages or services. Docker-compatible socket names,
legacy fields, Docker Hub registry references, and explicit `DOCKER_HOST`
overrides remain compatibility surfaces rather than runtime backends.

---

## 9. Runtime Health Check

### 9.1 Health Check Protocol

```rust
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub runtime_state: RuntimeState,
    pub engine_responsive: bool,
    pub compatibility_responsive: bool,
    pub compatibility_version: Option<String>,
    pub docker_responsive: bool, // legacy compatibility field
    pub docker_version: Option<String>, // legacy compatibility field
    pub uptime_seconds: Option<u64>,
    pub last_check: String, // RFC3339 timestamp
    pub engine_source: Option<String>,
    pub docker_source: Option<String>, // legacy compatibility field
}

/// Perform a health check on the runtime
pub async fn health_check(&self) -> Result<HealthStatus, AppError> {
    // 1. Check if VM process is running
    let vm_running = self.is_vm_process_alive();

    // 2. Check if the Engine compatibility socket exists
    let socket_exists = self.engine_socket_path().exists();

    // 3. Try native Engine contract first.
    let engine_responsive = socket_exists
        && engine_http_get_json_unix_socket(self.engine_socket_path(), "/cratebay/engine").is_ok();

    // 4. Try Engine compatibility ping for legacy clients.
    let compatibility_responsive = if socket_exists {
        let compatibility = Docker::connect_with_unix(
            self.engine_socket_path().to_str().unwrap(),
            5, // 5 second timeout
            API_DEFAULT_VERSION,
        )?;
        compatibility.ping().await.is_ok()
    } else {
        false
    };

    Ok(HealthStatus {
        runtime_state: if engine_responsive {
            RuntimeState::Ready
        } else if vm_running {
            RuntimeState::Starting
        } else {
            RuntimeState::Stopped
        },
        engine_responsive,
        compatibility_responsive,
        compatibility_version: None, // filled if responsive
        docker_responsive: compatibility_responsive,
        docker_version: None, // legacy compatibility field
        uptime_seconds: None,
        last_check: Utc::now().to_rfc3339(),
        engine_source: Some("builtin".to_string()),
        docker_source: Some("builtin".to_string()),
    })
}
```

**Stability requirements**

- `last_check` MUST always be a valid RFC3339 timestamp (including when emitting an error status).
- To avoid transient UI flicker (e.g., `Ready → Starting` due to brief socket jitter), implementations SHOULD:
  - Retry the Engine compatibility ping a small number of times (e.g., 3 attempts with short backoff), and
  - Use the native Engine contract as the authoritative `Ready` signal, and
  - Use a short failure threshold (e.g., 2–3 consecutive failed checks) before downgrading from `Ready`.

### 9.2 Periodic Health Monitoring

The backend runs a background health check every 30 seconds:

```rust
pub fn start_health_monitor(
    runtime: Arc<dyn RuntimeManager>,
    app: AppHandle,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match runtime.health_check().await {
                Ok(status) => {
                    let _ = app.emit("runtime:health", &status);
                }
                Err(e) => {
                    tracing::warn!("Health check failed: {}", e);
                    let _ = app.emit("runtime:health", &HealthStatus {
                        runtime_state: RuntimeState::Error(e.to_string()),
                        engine_responsive: false,
                        compatibility_responsive: false,
                        compatibility_version: None,
                        docker_responsive: false,
                        docker_version: None,
                        uptime_seconds: None,
                        last_check: Utc::now().to_rfc3339(),
                        engine_source: Some("builtin".to_string()),
                        docker_source: Some("builtin".to_string()),
                    });
                }
            }
        }
    });
}
```

---

## 10. Explicit Docker Host Compatibility

### 10.1 Default Resolution

CrateBay does not auto-detect Docker Desktop, Colima, OrbStack, Podman, or
platform default sockets before starting its own runtime. The default engine
resolution is:

```
Priority 1: Already-running built-in runtime endpoint
Priority 2: Provision/start built-in runtime
```

This keeps the CLI and desktop app aligned with the self-contained runtime
strategy and avoids accidentally managing containers from a different host
engine.

### 10.2 Explicit Overrides

CrateBay supports common Docker host formats when the operator explicitly passes
`--docker-host` or sets `DOCKER_HOST`:

- `unix:///path/to/docker.sock` (macOS/Linux)
- `/path/to/docker.sock` (macOS/Linux shorthand)
- `tcp://host:port` (treated as `http://host:port`)
- `http://host:port` / `https://host:port`
- `npipe:////./pipe/docker_engine` (Windows)

If an explicit host is set but is not reachable, CrateBay surfaces that failure
instead of silently falling back to a different engine.

### 10.3 Coexistence Strategy

| Scenario | Behavior |
|----------|----------|
| No explicit host | Start or reuse the built-in runtime |
| Built-in runtime already running | Reuse the built-in runtime socket/host |
| Docker Desktop / Colima / OrbStack running but no override set | Ignore it and keep using the built-in runtime path |
| `--docker-host` or `DOCKER_HOST` set | Use that explicit Docker-compatible endpoint |
| Explicit host stopped or unreachable | Surface the connection error; user can unset the override to return to built-in runtime |
| External host + built-in runtime both running | Prefer the explicit host only while the override is set |

---

## 11. Platform-Specific Implementation

### 11.1 macOS: VZ.framework Implementation

The macOS runtime uses Apple's Virtualization.framework via an external Swift
binary (`cratebay-vz`). The Rust code spawns and manages this process.

#### Architecture-Specific Socket Forwarding

```rust
// cratebay-core/src/runtime/macos.rs

/// Determine the default socket forwarding mode based on CPU architecture.
fn default_socket_forward_mode() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    { "vsock" }  // Apple Silicon: vsock is reliable
    #[cfg(target_arch = "x86_64")]
    { "tcp" }    // Intel: fallback to reverse TCP
}

/// Resolve the socket forwarding mode from environment or architecture default.
fn resolve_socket_forward_mode() -> String {
    std::env::var("CRATEBAY_RUNTIME_SOCKET_FORWARD")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| matches!(v.as_str(), "vsock" | "tcp"))
        .unwrap_or_else(|| default_socket_forward_mode().to_string())
}
```

#### MacOSRuntime Structure

```rust
#[cfg(target_os = "macos")]
pub struct MacOSRuntime {
    config: RuntimeConfig,
    state: Arc<Mutex<RuntimeState>>,
    runner: Arc<Mutex<Option<Child>>>,      // VZ runner child process
    runner_pid: Arc<Mutex<Option<u32>>>,    // For PID file recovery
    started_at: Arc<Mutex<Option<Instant>>>,
    consecutive_health_failures: Arc<Mutex<u8>>,
}

impl MacOSRuntime {
    pub fn new() -> Self {
        // ... initialization with PID file recovery
    }

    /// Spawn the VZ runner process with appropriate forwarding mode
    fn spawn_runner(&self) -> Result<Child, AppError> {
        let runner_path = vz_runner_path();
        let forward_mode = resolve_socket_forward_mode();

        let mut cmd = Command::new(&runner_path);
        cmd.arg("--kernel").arg(&paths.kernel_path)
           .arg("--disk").arg(&disk)
           .arg("--cpus").arg(self.config.cpu_cores.to_string())
           .arg("--memory-mb").arg(self.config.memory_mb.to_string())
           .arg("--cmdline").arg(&cmdline)
           .arg("--ready-file").arg(&ready_file);

        // Set up Engine compatibility endpoint forwarding based on mode
        let forward_spec = format!("{}:{}", port, sock_path.to_string_lossy());
        match forward_mode.as_str() {
            "vsock" => {
                cmd.arg("--vsock-forward").arg(&forward_spec);
            }
            _ => {
                cmd.arg("--tcp-forward").arg(&forward_spec);
            }
        }

        // ... spawn and return
    }
}

impl RuntimeManager for MacOSRuntime {
    async fn start(&self) -> Result<(), AppError> {
        // ... check state, cleanup stray processes
        let child = self.spawn_runner()?;
        // ... wait for ready file, wait for CrateBay Engine
    }

    // ... other trait methods
}
```

#### VZ Runner Binary (`cratebay-vz`)

The Swift-based VZ runner handles all Virtualization.framework API calls and
socket forwarding:

```
cratebay-vz
  --kernel <path>           Kernel image (vmlinuz)
  --initrd <path>           Initial ramdisk
  --disk <path>             VM disk image
  --cpus <n>                CPU cores
  --memory-mb <n>           Memory in MB
  --cmdline <str>           Kernel command line
  --ready-file <path>       Written when VM is ready
  --console-log <path>      Console output log
  --vsock-forward <spec>    guest_port:unix_socket_path (vsock mode)
  --tcp-forward <spec>      guest_port:unix_socket_path (reverse TCP mode)
  --share <spec>            tag:host_path[:ro] (VirtioFS share)
```

### 11.2 Linux: KVM/QEMU Implementation

```rust
// cratebay-core/src/runtime/linux.rs
#[cfg(target_os = "linux")]
pub struct LinuxRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,
    qemu_process: Option<Child>,
}

impl LinuxRuntime {
    /// Build QEMU command line arguments
    fn build_qemu_args(&self) -> Vec<String> {
        let mut args = vec![
            "-enable-kvm".to_string(),
            "-m".to_string(), format!("{}M", self.config.memory_mb),
            "-smp".to_string(), format!("{}", self.config.cpu_cores),
            "-kernel".to_string(), self.data_dir.join("vmlinuz").to_string_lossy().to_string(),
            "-initrd".to_string(), self.data_dir.join("initrd").to_string_lossy().to_string(),
            "-drive".to_string(), format!(
                "file={},format=qcow2,if=virtio",
                self.data_dir.join("rootfs.qcow2").display()
            ),
            "-nographic".to_string(),
            "-nodefaults".to_string(),
        ];

        // Engine compatibility endpoint forwarding
        let socket_path = self.engine_socket_path();
        args.extend_from_slice(&[
            "-chardev".to_string(),
            format!("socket,id=engine,path={},server=on,wait=off", socket_path.display()),
        ]);

        // Shared directories (VirtioFS)
        for dir in &self.config.shared_dirs {
            args.extend_from_slice(&[
                "-virtfs".to_string(),
                format!(
                    "local,path={},mount_tag={},security_model=mapped-xattr",
                    dir.host_path, dir.tag
                ),
            ]);
        }

        // Network
        args.extend_from_slice(&[
            "-netdev".to_string(), "user,id=net0".to_string(),
            "-device".to_string(), "virtio-net-pci,netdev=net0".to_string(),
        ]);

        args
    }
}

impl RuntimeManager for LinuxRuntime {
    async fn start(&self) -> Result<(), AppError> {
        // Check KVM availability
        if !Path::new("/dev/kvm").exists() {
            return Err(AppError::Runtime(
                "KVM not available. Ensure virtualization is enabled in BIOS.".into()
            ));
        }

        let args = self.build_qemu_args();
        let child = Command::new("qemu-system-x86_64")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Runtime(format!("Failed to start QEMU: {}", e)))?;

        self.qemu_process = Some(child);
        self.wait_for_engine(Duration::from_secs(30)).await?;
        Ok(())
    }

    // ... other trait methods
}
```

### 11.3 Windows: WSL2 Implementation

```rust
// cratebay-core/src/runtime/windows.rs
#[cfg(target_os = "windows")]
pub struct WindowsRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,
    distro_name: String, // "CrateBay"
}

impl WindowsRuntime {
    const DISTRO_NAME: &'static str = "CrateBay";

    /// Check if WSL2 is available
    async fn check_wsl2(&self) -> Result<bool, AppError> {
        let output = Command::new("wsl")
            .args(["--status"])
            .output()
            .await
            .map_err(|e| AppError::Runtime(format!("WSL check failed: {}", e)))?;
        Ok(output.status.success())
    }

    /// Import CrateBay distro into WSL2
    async fn import_distro(&self) -> Result<(), AppError> {
        let tar_path = self.data_dir.join("cratebay-wsl.tar");
        let install_dir = self.data_dir.join("wsl-distro");

        let output = Command::new("wsl")
            .args([
                "--import",
                Self::DISTRO_NAME,
                &install_dir.to_string_lossy(),
                &tar_path.to_string_lossy(),
                "--version", "2",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(AppError::Runtime(
                format!("WSL import failed: {}", String::from_utf8_lossy(&output.stderr))
            ));
        }
        Ok(())
    }
}

impl RuntimeManager for WindowsRuntime {
    async fn start(&self) -> Result<(), AppError> {
        // Ensure WSL2 is available
        if !self.check_wsl2().await? {
            return Err(AppError::Runtime(
                "WSL2 is not available. Please enable WSL2 in Windows Features.".into()
            ));
        }

        // Start the distro and launch CrateBay Engine (containerd + adapter)
        Command::new("wsl")
            .args(["-d", Self::DISTRO_NAME, "--", "/usr/local/bin/cratebay-engine-start"])
            .spawn()
            .map_err(|e| AppError::Runtime(format!("Failed to start CrateBay Engine: {}", e)))?;

        // Set up compatibility API forwarding (CrateBay Engine socket → named pipe)
        self.setup_engine_forward().await?;

        self.wait_for_engine(Duration::from_secs(30)).await?;
        Ok(())
    }

    async fn provision(
        &self,
        on_progress: impl Fn(ProvisionProgress) + Send + 'static,
    ) -> Result<(), AppError> {
        // 1. Check WSL2 availability
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 10.0,
            message: "Checking WSL2 availability...".into(),
            ..Default::default()
        });

        if !self.check_wsl2().await? {
            return Err(AppError::Runtime(
                "WSL2 is required. Please run: wsl --install".into()
            ));
        }

        // 2. Download distro image
        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 20.0,
            message: "Downloading CrateBay runtime image...".into(),
            ..Default::default()
        });
        self.download_image(|progress| {
            on_progress(ProvisionProgress {
                stage: "downloading".into(),
                percent: 20.0 + progress * 0.6,
                ..Default::default()
            });
        }).await?;

        // 3. Import into WSL2
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 85.0,
            message: "Importing WSL2 distro...".into(),
            ..Default::default()
        });
        self.import_distro().await?;

        on_progress(ProvisionProgress {
            stage: "complete".into(),
            percent: 100.0,
            message: "Runtime provisioned successfully".into(),
            ..Default::default()
        });

        Ok(())
    }

    // ... other trait methods
}
```

### 11.4 Platform Comparison Summary

| Aspect | macOS (VZ) | Linux (KVM) | Windows (WSL2) |
|--------|-----------|-------------|----------------|
| Boot time | ~3-5 s | ~3-5 s | ~2-4 s |
| Memory overhead | ~200 MB | ~200 MB | ~150 MB (shared) |
| File sharing perf | Excellent (VirtioFS) | Excellent (VirtioFS) | Good (9P) |
| Port forwarding | Automatic (NAT) | Manual (hostfwd) | Automatic (Win11) |
| Engine endpoint (arm64) | **vsock → Unix** | chardev → Unix | TCP/pipe |
| Engine endpoint (x86_64) | **reverse TCP → Unix** | chardev → Unix | TCP/pipe |
| First-run download | ~400 MB | ~400 MB | ~350 MB |
| Min OS version | macOS 13 | Kernel 5.10 | Win10 21H2 |

#### macOS Socket Forwarding Mode Selection

| Condition | Mode Used |
|-----------|-----------|
| Apple Silicon + default | vsock |
| Apple Silicon + `CRATEBAY_RUNTIME_SOCKET_FORWARD=tcp` | reverse TCP |
| Intel x86_64 + default | reverse TCP |
| Intel x86_64 + `CRATEBAY_RUNTIME_SOCKET_FORWARD=vsock` | vsock (may have issues) |

---

## Appendix A: VM Image Build Process

The VM images are built in CI and published as GitHub Release assets:

```
1. Start with Alpine Linux minimal rootfs
2. Install containerd, runc, CNI plugins, iptables, and iproute2
3. Install CrateBay Engine adapter and guest-agent
4. Configure containerd root/state, CNI networks, and CrateBay storage directories
5. Configure Engine compatibility endpoint forwarding service
6. Strip unnecessary files
7. Package:
   - macOS/Linux: vmlinuz + initrd + rootfs.img/qcow2
   - Windows: tar export for WSL import
8. Compress with zstd
9. Upload to GitHub Release
```

## Appendix B: Troubleshooting

| Issue | Platform | Resolution |
|-------|----------|------------|
| "KVM not available" | Linux | Enable virtualization in BIOS/UEFI |
| "VZ.framework error" | macOS | Ensure macOS 13+, check System Preferences > Security |
| "WSL2 not available" | Windows | Run `wsl --install` in admin PowerShell |
| Engine endpoint timeout | All | Check `~/.cratebay/runtime/` for socket file and CrateBay Engine adapter logs; restart runtime |
| VM won't start | All | Delete `~/.cratebay/runtime/` and re-provision |
| Port forwarding fails | Linux | Check iptables rules inside VM |
