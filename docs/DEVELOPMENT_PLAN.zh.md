# CrateBay 完整开发方案

状态: Draft v0.1  
目标读者: 项目维护者、人类开发者、AI coding agent  
项目阶段: 空仓库从零开发  
核心方向: 本地 VM / container / agent sandbox runtime  

## 1. 一句话目标

CrateBay 是一个开源的本地开发运行时，提供类似 OrbStack 的轻量 VM 和容器体验，并把 AI Agent 所需的受控 sandbox 作为一等能力。

English positioning:

> CrateBay is an open-source local VM, container, and agent sandbox runtime for developers and AI agents.

核心路线不要追求完整复刻 OrbStack，而是直接围绕最终目标开发。核心目标是:

- 人可以用 GUI 和 CLI 启动本地 Linux 运行环境。
- AI Agent 可以通过 CLI JSON mode 或本地 API 在受控 sandbox 中执行命令。
- macOS 先用 Lima 起步，后续迁移到 Apple Virtualization.framework。
- Windows 先用 WSL2 起步，后续迁移到 Hyper-V。
- 所有核心能力都有全自动测试，不依赖人工逐个功能点验证。

## 2. 产品原则

### 2.1 核心原则

1. Provider 可替换  
   Lima、WSL2、Apple Virtualization.framework、Hyper-V 都只是 `VmProvider` 的实现，不进入业务层。

2. Sandbox 是主线  
   VM 和容器只是执行基础设施。核心路线先稳定 agent sandbox 的策略、审计、diff 和 session model；完整路线继续实现 snapshot/restore、可恢复执行和更强隔离。

3. GUI 与 CLI/API 同步纳入核心路线  
   GUI scaffold 从第一验收闭环开始创建，但 GUI 只能调用 daemon API，不直接管理 VM；复杂交互在 API 稳定后逐步扩展。

4. 自动化测试优先  
   每个能力在设计时就要配套自动化测试。没有自动测试的功能不能视为完成。

5. 性能可度量  
   追求类似 OrbStack 的启动速度和顺滑体验，但所有优化必须有 benchmark 和回归阈值。

6. 中英双语但协议稳定  
   UI 和人类 CLI 文本支持 `zh-CN` 和 `en-US`。JSON 字段、错误码、事件名永远使用英文稳定 key。

7. 安全边界诚实  
   核心路线的容器级 sandbox 适合可信开发和半可信 agent，不宣传成强安全边界。真正的强隔离通过 per-task VM / Hyper-V / Apple VZ 模式逐步增强。

### 2.2 可选插件

以下能力不进入核心路线，可以在核心稳定后作为 optional plugin:

- Kubernetes / k3s
- 完整 Docker Desktop 兼容
- 自动本地域名和 HTTPS
- 多 Linux 发行版 machine 管理
- USB、音频、GPU passthrough
- 企业级策略管理
- 多用户远程调度平台
- 云端同步

这些能力可以进入后续 roadmap，但不能阻塞核心路线。

## 3. 目标用户和使用场景

### 3.1 人类开发者

开发者希望:

- 一键启动本地 Linux 环境。
- 快速运行容器或命令。
- 用 GUI 查看 VM、sandbox session、日志、资源占用。
- 用 CLI 脚本化常见动作。
- 不手动维护复杂的虚拟机、端口、镜像缓存。

典型命令:

```bash
cratebay vm start
cratebay vm status
cratebay sandbox run --profile dev-open -- npm test
cratebay sandbox logs sbox_123
cratebay sandbox diff sbox_123
```

### 3.2 AI Agent

Agent 希望:

- 在受控环境中执行命令。
- 使用稳定 JSON 输入输出。
- 不读取宿主机敏感目录。
- 网络、环境变量、文件系统权限可被策略限制。
- 执行后能拿到 stdout、stderr、exit code、duration、changed files、policy events。
- 命令失败时能知道是命令失败、策略拒绝、VM 未启动、超时还是内部错误。

典型命令:

```bash
cratebay --json sandbox run \
  --mount .:/workspace \
  --profile agent-restricted \
  -- npm test
```

典型输出:

```json
{
  "session_id": "sbox_01JZ0000000000000000000000",
  "status": "exited",
  "exit_code": 0,
  "stdout": "test result: ok\n",
  "stderr": "",
  "duration_ms": 1842,
  "changed_files": ["src/lib.rs"],
  "policy_events": [],
  "artifacts": []
}
```

## 4. 总体架构

### 4.1 组件图

```text
Tauri Desktop GUI
CLI: cratebay
Agent Adapter / MCP Server
        |
        v
Local API: JSON-RPC over Unix socket / Windows named pipe
        |
        v
Host Daemon: cratebayd
        |
        +-- state store
        +-- policy compiler
        +-- session manager
        +-- event stream
        +-- image/cache manager
        |
        v
VmProvider trait
        |
        +-- LimaProvider          macOS bootstrap provider
        +-- Wsl2Provider          Windows bootstrap provider
        +-- AppleVzProvider       macOS final provider
        +-- HyperVProvider        Windows final provider
        +-- FakeProvider          tests
        |
        v
Linux Utility VM / Linux Guest
        |
        +-- cratebay-guestd
        +-- sandbox runner
        +-- containerd
        +-- runc / crun
        +-- cgroups v2
        +-- overlayfs
        +-- nftables
        +-- image cache
```

### 4.2 控制面和数据面

控制面:

- CLI 到 `cratebayd`
- GUI 到 `cratebayd`
- Agent 到 CLI 或 `cratebayd`
- `cratebayd` 到 `cratebay-guestd`
- `cratebayd` 到 provider

数据面:

- workspace mount
- container image layers
- command stdout/stderr stream
- file diff
- logs
- artifacts

设计约束:

- GUI 不直接调用 `limactl`、`wsl.exe`、Hyper-V 或 Apple VZ。
- CLI 不直接进入 VM 执行业务命令，必须通过 daemon。
- daemon 不直接在 guest 内拼 shell 脚本执行业务逻辑，必须调用 `cratebay-guestd` 的结构化 API。
- guest agent 不假设自己运行在哪个 provider 里，只关心 Linux 能力。

## 5. 技术栈

### 5.1 Rust 后端

| 模块 | 技术选择 | 原因 |
|---|---|---|
| workspace | Cargo workspace | 多 crate 管理清晰 |
| async runtime | `tokio` | VM、进程、socket、stream 都需要异步 |
| CLI | `clap` | Rust CLI 标准选择 |
| 序列化 | `serde`, `serde_json`, `toml` | JSON API、配置、policy |
| 错误 | `thiserror`, `anyhow` | library 和 binary 分层 |
| 日志 | `tracing`, `tracing-subscriber` | 结构化日志和测试断言 |
| 本地 API | JSON-RPC 2.0 over socket | AI 和人类都容易调试 |
| 状态存储 | SQLite + `sqlx` 或 `rusqlite` | session、审计、镜像元数据 |
| schema | `schemars` | 为 policy 和 API 输出 JSON Schema |
| 测试 | `cargo test`, `assert_cmd`, `insta`, `proptest` | 单测、CLI、快照、属性测试 |
| xtask | `cargo xtask` | 统一本地和 CI 验证入口 |

核心推荐:

- API 先用 JSON-RPC，不急着上 gRPC。
- JSON-RPC 的好处是容易被 agent 调用、记录、mock、debug。
- 后续如果需要 IDE 插件或高性能 streaming，可以加 gRPC，但不能破坏 CLI JSON contract。

### 5.2 GUI

| 模块 | 技术选择 | 原因 |
|---|---|---|
| Desktop shell | Tauri v2 | Rust 生态统一、体积小、跨平台 |
| Frontend | React + TypeScript + Vite | 生态成熟，AI 生成和维护成本低 |
| 状态 | Zustand 或 TanStack Query | 远程 daemon 状态管理 |
| i18n | i18next | 中英双语 |
| UI 测试 | Vitest + Testing Library | 组件测试 |
| E2E | Playwright | 跨平台 UI 自动化 |

GUI 页面核心范围:

- Dashboard: VM 状态、daemon 状态、最近 session
- Sandboxes: session 列表、状态、命令、耗时、退出码
- Logs: stdout/stderr/event stream
- Policies: profile 查看和选择
- Images: sandbox base image 列表和缓存状态
- Settings: 语言、provider、资源限制、自动启动

### 5.3 macOS VM

Bootstrap provider:

- LimaProvider
- 通过 `limactl` 创建和启动 Linux VM
- 使用 Lima 的 mount、SSH、端口能力快速验证产品模型
- Apple Silicon 机器优先使用 Lima 的 VZ driver、`aarch64` guest，并在需要运行 `linux/amd64` 程序或容器时启用 Rosetta。
- Intel Mac 使用 `x86_64` guest，不需要 Rosetta。

正式:

- AppleVzProvider
- 使用 Apple Virtualization.framework
- Swift helper 负责直接调用系统框架
- Rust daemon 通过 JSON over stdio 或 FFI 调用 Swift helper
- Apple Silicon 上通过 Apple Virtualization.framework 的 Rosetta for Linux 支持 `linux/amd64` 用户态程序和容器。
- Intel Mac 上使用原生 `x86_64` Linux VM。

建议顺序:

1. LimaProvider 让产品先跑起来。
2. Provider trait 稳定后做 AppleVzProvider。
3. AppleVzProvider 进入后，LimaProvider 保留为 dev backend 或 fallback。

#### 5.3.1 macOS Rosetta 策略

结论:

- macOS VM 本身不是“用 Rosetta 跑”。VM 仍然是原生虚拟化。
- 在 Apple Silicon Mac 上，默认 guest architecture 应该是 `aarch64/arm64`。
- Rosetta 只用于在 ARM Linux VM 里运行 `linux/amd64` 用户态程序或 amd64 容器。
- Intel Mac 不需要 Rosetta，默认 guest architecture 应该是 `x86_64/amd64`。
- VZ 不负责运行“x86_64 整机 VM on ARM host”。跨架构整机 VM 需要 QEMU，速度慢，不作为默认路线。

Bootstrap LimaProvider on Apple Silicon:

```yaml
vmType: "vz"
arch: "aarch64"
rosetta:
  enabled: true
  binfmt: true
```

Bootstrap LimaProvider on Intel Mac:

```yaml
vmType: "vz"
arch: "x86_64"
rosetta:
  enabled: false
```

使用规则:

- 如果 host 是 Apple Silicon 且 macOS 版本支持 VZ + Rosetta，则默认启用 Rosetta。
- 如果用户只运行 arm64 镜像，Rosetta 不参与执行。
- 如果用户运行 `linux/amd64` 镜像，containerd/runc 通过 binfmt 调用 Rosetta。
- 如果 Rosetta 未安装，CLI/GUI 提示用户运行 `softwareupdate --install-rosetta`，并返回 `ROSETTA_NOT_INSTALLED`。
- 如果 host 不支持 Rosetta，默认返回 `ROSETTA_UNSUPPORTED`。
- QEMU user-mode emulation 只作为用户显式开启的兼容慢路径，不作为默认路线。
- 不默认使用 QEMU system emulation 跑 x86_64 VM。

正式 AppleVzProvider:

- Apple Silicon: Swift helper 创建 ARM Linux VM，挂载 Rosetta directory，并在 guest 内注册 binfmt。
- Intel Mac: Swift helper 创建 x86_64 Linux VM，不挂载 Rosetta。
- 暴露 provider capability:

```json
{
  "provider": "apple-vz",
  "capabilities": {
    "rosetta_linux": true,
    "amd64_containers_on_arm64": true,
    "foreign_arch_vm": false
  }
}
```

Rosetta 测试:

- `ROSETTA_001`: Apple Silicon 上运行 arm64 容器，不触发 Rosetta，返回 `aarch64`。
- `ROSETTA_002`: Apple Silicon 上运行 `linux/amd64` 容器，返回 `x86_64`。
- `ROSETTA_003`: Rosetta 未安装时返回 `ROSETTA_NOT_INSTALLED`。
- `ROSETTA_004`: 不支持 Rosetta 的 host 返回 capability false。
- `ROSETTA_005`: amd64 warm run 性能低于阈值时生成 performance warning。

macOS 架构测试:

- `MAC_ARCH_001`: Intel Mac capability 显示 `host_arch=x86_64`、`guest_arch=x86_64`、`rosetta_linux=false`。
- `MAC_ARCH_002`: Apple Silicon capability 显示 `host_arch=arm64`、`guest_arch=aarch64`。
- `MAC_ARCH_003`: Apple Silicon 上不创建默认 x86_64 整机 VM。
- `MAC_ARCH_004`: Intel Mac 上运行 amd64 容器不经过 Rosetta。

### 5.4 Windows VM

Bootstrap provider:

- Wsl2Provider
- 使用自定义 WSL distro 或导入 rootfs tar
- 通过 `wsl.exe` 启动 guest
- 在 WSL 内运行 `cratebay-guestd`

正式:

- HyperVProvider
- 使用 Hyper-V VM、VHDX、虚拟交换机
- 初期可以通过 PowerShell module 管理
- 后续使用 HCS / hcsshim 或 Windows API 做更深集成

建议顺序:

1. WSL2 让 Windows 用户和 agent sandbox 快速可用。
2. Hyper-V 做强隔离路线，不阻塞核心路线。
3. WSL2 保留为 `dev-fast` provider，Hyper-V 作为 `agent-safe` provider。

### 5.5 Linux guest

Linux guest 内组件:

- `cratebay-guestd`: Rust 静态二进制
- `containerd`
- `runc` 或 `crun`
- `nftables`
- `cgroups v2`
- `overlayfs`
- `tar`, `gzip`, `zstd`
- 基础 CA certificates
- DNS resolver
- 可选: `buildkitd`

guest 不运行 Kubernetes。

### 5.6 Platform Support Matrix

核心路线必须覆盖以下平台:

| Host | CPU | Bootstrap provider | Final provider | Default guest arch | amd64 compatibility | Notes |
|---|---|---|---|---|---|---|
| macOS | Apple Silicon | Lima VZ | AppleVzProvider | `aarch64` | Rosetta for Linux | 不默认创建 x86_64 整机 VM |
| macOS | Intel | Lima VZ | AppleVzProvider | `x86_64` | native amd64 | 不需要 Rosetta |
| Windows | x86_64 | WSL2 | HyperVProvider | `x86_64` | native amd64 | Hyper-V 作为强隔离路线 |
| Windows | ARM64 | WSL2 experimental | HyperVProvider experimental | `aarch64` | 后续评估 | 实验性平台，不作为核心路线首批强承诺 |

Provider detection 规则:

- 默认配置使用 `provider = "auto"`。
- macOS Apple Silicon bootstrap 默认选择 `lima` + VZ + `aarch64`。
- macOS Intel bootstrap 默认选择 `lima` + VZ + `x86_64`。
- Windows bootstrap 默认选择 `wsl2`。
- 完整路线中，macOS 可切换到 `apple-vz`，Windows 可切换到 `hyperv`。
- 如果平台缺少依赖，`doctor` 和测试报告必须返回明确的 `PROVIDER_DEPENDENCY_MISSING` 或 skipped reason。

最低平台要求需要由 `cratebay doctor` 自动检测:

- macOS version。
- CPU architecture。
- Virtualization.framework availability。
- Lima availability。
- Rosetta availability on Apple Silicon。
- Windows version。
- WSL2 availability。
- Hyper-V availability。
- Windows ARM64 experimental capability。
- nested virtualization / self-hosted CI capability。

Windows ARM64 规则:

- `doctor` 必须报告 Windows ARM64 capability。
- Windows ARM64 测试默认标记为 experimental/skipped。
- Windows ARM64 不进入 release blocking tests，除非未来明确提升为 supported platform。

## 6. 仓库结构

从空仓库建议这样开始:

```text
CrateBay/
  .cargo/
    config.toml
  Cargo.toml
  README.md
  README.zh.md
  LICENSE
  docs/
    DEVELOPMENT_PLAN.zh.md
    architecture/
    testing/
    specs/
  crates/
    cratebay-core/
      src/
    cratebay-protocol/
      src/
    cratebay-daemon/
      src/
    cratebay-cli/
      src/
    cratebay-provider/
      src/
    cratebay-provider-lima/
      src/
    cratebay-provider-wsl2/
      src/
    cratebay-provider-fake/
      src/
    cratebay-provider-apple-vz/
      src/
    cratebay-provider-hyperv/
      src/
    cratebay-guestd/
      src/
    cratebay-mcp/
      src/
    cratebay-xtask/
      src/
  native/
    apple-vz-helper/
      Sources/
  apps/
    desktop/
      package.json
      pnpm-lock.yaml
      vite.config.ts
      src/
      src-tauri/
  images/
    utility-vm/
    sandbox/
  policies/
    dev-open.json
    agent-restricted.json
    agent-offline.json
  scripts/
    dev/
    ci/
  tests/
    fixtures/
    e2e/
```

crate 说明:

- `cratebay-core`: 业务模型、错误类型、配置、policy model。
- `cratebay-protocol`: Host API、guest API、event、JSON schema。
- `cratebay-daemon`: daemon binary。
- `cratebay-cli`: CLI binary。
- `cratebay-provider`: `VmProvider` trait 和 capability model。
- `cratebay-provider-lima`: macOS bootstrap provider。
- `cratebay-provider-wsl2`: Windows bootstrap provider。
- `cratebay-provider-fake`: 测试 provider。
- `cratebay-provider-apple-vz`: macOS final provider，调用 Apple Virtualization.framework helper。
- `cratebay-provider-hyperv`: Windows final provider，管理 Hyper-V VM。
- `cratebay-guestd`: Linux guest agent。
- `cratebay-mcp`: MCP server binary，不直接实现 sandbox，只调用 daemon API。
- `cratebay-xtask`: 本地和 CI 自动化入口。
- `native/apple-vz-helper`: Swift helper，封装 Apple Virtualization.framework。
- `.cargo/config.toml`: 定义 `xtask = "run -p cratebay-xtask --"` alias，保证 `cargo xtask ...` 在任意机器可执行。

`.cargo/config.toml` 必须包含:

```toml
[alias]
xtask = "run -p cratebay-xtask --"
```

## 7. 核心抽象

### 7.1 VmProvider

`VmProvider` 是迁移成功的关键。上层只允许依赖这个 trait。

```rust
#[async_trait::async_trait]
pub trait VmProvider: Send + Sync {
    async fn init(&self, config: VmInitConfig) -> Result<VmInfo>;
    async fn start(&self) -> Result<VmInfo>;
    async fn stop(&self, mode: StopMode) -> Result<()>;
    async fn status(&self) -> Result<VmStatus>;
    async fn ensure_guest_agent(&self, agent: GuestAgentBundle) -> Result<()>;
    async fn guest_endpoint(&self) -> Result<GuestEndpoint>;
    async fn mount_workspace(&self, mount: WorkspaceMount) -> Result<MountInfo>;
    async fn forward_port(&self, rule: PortForwardRule) -> Result<PortForwardInfo>;
    async fn collect_diagnostics(&self) -> Result<ProviderDiagnostics>;
}
```

Provider capability:

```json
{
  "provider": "lima",
  "platform": "macos",
  "capabilities": {
    "start_stop": true,
    "workspace_mount": true,
    "port_forward": true,
    "snapshot": false,
    "strong_isolation": false,
    "guest_agent": true
  }
}
```

### 7.2 Guest API

Host daemon 和 guest agent 使用结构化 API。

Guest methods:

- `guest.ping`
- `guest.info`
- `guest.diagnostics`
- `sandbox.create`
- `sandbox.exec`
- `sandbox.kill`
- `sandbox.logs`
- `sandbox.diff`
- `sandbox.destroy`
- `image.pull`
- `image.list`
- `image.prefetch`
- `diagnostics.collect`

`sandbox.exec` request:

```json
{
  "session_id": "sbox_123",
  "command": ["npm", "test"],
  "cwd": "/workspace",
  "env": {
    "CI": "1"
  },
  "timeout_ms": 600000,
  "stream": true
}
```

`sandbox.exec` response:

```json
{
  "session_id": "sbox_123",
  "status": "exited",
  "exit_code": 0,
  "stdout_ref": "log://sbox_123/stdout",
  "stderr_ref": "log://sbox_123/stderr",
  "duration_ms": 1842
}
```

### 7.3 Host API

Host daemon 本地 API 使用 JSON-RPC 2.0。

Host API target methods:

第一验收闭环必须实现:

- `daemon.version`
- `daemon.status`
- `doctor.run`
- `vm.status`
- `provider.capabilities`
- `sandbox.run`
- `sandbox.list`
- `sandbox.logs`
- `sandbox.diff`
- `sandbox.destroy`
- `policy.validate`

完整核心必须实现:

- `daemon.stop`
- `vm.init`
- `vm.start`
- `vm.stop`
- `vm.logs`
- `provider.list`
- `guest.ping`
- `guest.info`
- `guest.diagnostics`
- `sandbox.create`
- `sandbox.exec`
- `policy.list`
- `policy.show`
- `image.list`
- `image.pull`
- `image.prefetch`
- `diagnostics.collect`

完整路线继续实现:

- `provider.diagnostics`
- `provider.migrate`
- `snapshot.create`
- `snapshot.list`
- `snapshot.restore`
- `snapshot.destroy`
- `mcp.status`

事件流使用 NDJSON:

```jsonl
{"type":"sandbox.started","session_id":"sbox_123","ts":"2026-06-21T08:00:00Z"}
{"type":"sandbox.stdout","session_id":"sbox_123","data":"running tests\n"}
{"type":"sandbox.exited","session_id":"sbox_123","exit_code":0}
```

## 8. Sandbox 设计

### 8.1 Sandbox 分层

核心路线直接使用 container-level sandbox:

```text
guest VM
  containerd container
    mount namespace
    pid namespace
    network namespace
    cgroups
    overlayfs upperdir
    workspace bind mount
```

优点:

- 启动快。
- 实现复杂度可控。
- 适合 coding agent 跑测试、构建、格式化、代码修改。

缺点:

- 不应视为强安全边界。
- guest 内核共享。
- 需要明确文档说明安全等级。

后续强隔离:

```text
agent-isolated
  per-task VM
  independent disk overlay
  default no network
  explicit host mount
```

### 8.2 默认 profile

`dev-open`:

- 给人类开发者。
- workspace 读写。
- 网络默认开启。
- 环境变量允许常见开发变量。
- 记录日志但 diff 可选。

`agent-restricted`:

- 默认 AI agent profile。
- workspace 读写。
- home 不挂载。
- 网络默认关闭。
- 允许配置域名 allowlist。
- 只注入 allowlisted env。
- 记录命令、stdout、stderr、diff、policy events。

`agent-offline`:

- 完全离线。
- 不允许外部网络。
- 适合运行单元测试、静态分析、格式化。

`agent-isolated`:

- 后续 profile。
- 每个 session 独立 VM 或强隔离微虚拟机。

### 8.3 Policy schema

核心 policy 示例:

```json
{
  "version": "0.1",
  "name": "agent-restricted",
  "description": {
    "zh-CN": "默认 AI Agent 沙箱策略",
    "en-US": "Default AI agent sandbox policy"
  },
  "filesystem": {
    "workspace": {
      "mode": "rw",
      "guest_path": "/workspace"
    },
    "home": "none",
    "extra_mounts": []
  },
  "network": {
    "default": "deny",
    "allow_hosts": [
      "github.com",
      "registry.npmjs.org",
      "pypi.org",
      "crates.io",
      "index.crates.io"
    ],
    "allow_ports": [80, 443],
    "dns": "cratebay-proxy"
  },
  "resources": {
    "cpus": 4,
    "memory_mb": 4096,
    "pids": 2048,
    "timeout_sec": 600,
    "disk_mb": 20480
  },
  "process": {
    "user": "sandbox",
    "workdir": "/workspace",
    "drop_capabilities": "all",
    "no_new_privileges": true
  },
  "secrets": {
    "env_allowlist": [],
    "file_mounts": []
  },
  "audit": {
    "record_commands": true,
    "record_stdout": true,
    "record_stderr": true,
    "record_exit_code": true,
    "record_file_diff": true,
    "record_network_denials": true
  },
  "persistence": {
    "keep_session_on_failure": true,
    "auto_destroy_after_sec": 86400
  }
}
```

### 8.4 Policy enforcement

核心 enforcement:

- 文件系统: container mount namespace + explicit bind mounts。
- home: 默认不挂载宿主 home。
- workspace: 只挂载用户指定目录。
- 环境变量: daemon 过滤后传给 guest。
- CPU/memory/pids: cgroups v2。
- 超时: daemon 和 guest 双层 timeout。
- 网络: network namespace + nftables。
- DNS allowlist: guest 内 DNS proxy 或 daemon 侧 proxy。
- Linux capabilities: drop all，必要时最小化添加。
- seccomp: 使用默认 deny dangerous syscalls 的 profile。
- diff: 执行前后扫描或 overlayfs upperdir。

核心路线允许先交付可测试子集，但接口必须按最终策略设计:

1. 网络必须支持 `allow`、`deny` 和 `allow_hosts` 域名白名单。
2. `allow_hosts` 可以先用简单 guest DNS proxy 或 daemon-side proxy 实现，但从第一轮 policy enforcement 起就必须生效。
3. diff 先用文件 hash scanner。
4. overlayfs diff 作为后续增强替换。

## 9. 镜像策略

### 9.1 是否需要预制系统镜像

需要，但要区分两类镜像:

1. Utility VM image  
   用来启动 CrateBay 的 Linux guest。它承载 `cratebay-guestd`、containerd、runc、nftables、overlayfs 等基础设施。

2. Sandbox base images  
   用来给 sandbox 运行命令，例如 `cratebay/base:alpine`、`cratebay/dev:ubuntu`、`cratebay/lang-node:22`。

不要把所有东西塞进一个巨大系统镜像。推荐:

- Utility VM image 尽量小，负责运行 runtime。
- Sandbox base image 按用途拆分，按需拉取和缓存。

### 9.2 Utility VM image

Bootstrap provider 使用现成发行版:

- macOS Lima: Ubuntu LTS 或 Alpine。
- Windows WSL2: Ubuntu LTS rootfs 或自定义 tar rootfs。

后续正式镜像:

- 自建 minimal Linux rootfs。
- 内置 `cratebay-guestd`。
- 内置 containerd、runc/crun、nftables、cgroups v2 支持。
- 内置 optimized kernel 或 provider 推荐 kernel。
- 使用 zstd 压缩。
- 支持版本升级和回滚。

Utility VM image 内容:

```text
/usr/bin/cratebay-guestd
/usr/bin/containerd
/usr/bin/runc
/usr/sbin/nft
/etc/cratebay/guestd.toml
/var/lib/cratebay/
/var/log/cratebay/
```

Utility VM image 不应该内置:

- 大量语言工具链
- Kubernetes
- GUI 包
- 用户项目依赖

### 9.3 Sandbox base images

核心路线推荐预置 image catalog，而不是全部内置到安装包。

基础镜像:

- `cratebay/base:alpine`
- `cratebay/base:ubuntu`

语言镜像:

- `cratebay/lang-node:22`
- `cratebay/lang-python:3.12`
- `cratebay/lang-rust:stable`
- `cratebay/lang-go:1.23`

Agent 镜像:

- `cratebay/agent-base:latest`
- `cratebay/agent-full:latest`

策略:

- 默认 agent 镜像的 canonical ref 是 `cratebay/agent-base:latest`。
- CLI 可以接受省略 tag 的 `cratebay/agent-base`，但内部必须规范化为 `cratebay/agent-base:latest`。
- 安装包内置一个极小 `cratebay/agent-base:latest`，保证离线可跑 `echo`、简单 shell、文件 diff 测试。
- 常用语言镜像首次使用时拉取，之后本地缓存。
- 提供 `cratebay image prefetch --profile agent-restricted` 预热。
- 所有官方镜像 digest pinning。
- 生成 SBOM。
- 后续支持签名验证。

### 9.4 完整镜像还是轻量镜像

推荐组合:

```text
Utility VM: 轻量但完整 runtime
Sandbox base: 轻量默认镜像
Language images: 按需较完整
Dev full image: 可选，不作为默认
```

原因:

- 默认完整镜像会拖慢下载、升级和冷启动。
- 轻量默认镜像可以保证 sandbox session 极快启动。
- 语言镜像按需缓存，适合真实项目。
- Agent 可以根据项目类型选择镜像，而不是每次都用大而全镜像。

### 9.5 镜像构建自动化

需要 `cargo xtask image build` 或 scripts:

```bash
cargo xtask image build utility-vm --target macos-aarch64
cargo xtask image build utility-vm --target macos-x86_64
cargo xtask image build utility-vm --target wsl2-x86_64
cargo xtask image build utility-vm --target hyperv-x86_64
cargo xtask image build sandbox-base --name alpine
cargo xtask image test sandbox-base --name alpine
cargo xtask image sbom sandbox-base --name alpine
```

镜像测试:

- 能启动 shell。
- 能运行 `cratebay-guestd --version`。
- containerd 能启动。
- runc 能创建容器。
- cgroups v2 可用。
- nftables 可用。
- overlayfs 可用。
- CA certificates 可用。
- DNS 可用或能被策略关闭。

## 10. 极致启动速度和性能路线

### 10.1 性能目标

核心性能目标:

| 指标 | 目标 |
|---|---|
| daemon cold start | < 500 ms |
| CLI status command | < 150 ms |
| VM warm start | < 5 s |
| VM cold start | < 20 s |
| guest ping after VM ready | < 100 ms |
| sandbox warm exec echo | < 500 ms |
| sandbox warm exec with cached image | < 1.5 s |
| diff for 5k files | < 2 s |

正式优化目标:

| 指标 | 目标 |
|---|---|
| app visible startup | < 1 s |
| VM warm resume | < 2 s |
| first sandbox command after warm VM | < 300 ms |
| cached image container start | < 500 ms |
| idle CPU | near 0 |

所有指标必须由自动 benchmark 记录。

### 10.2 启动优化路线

基础优化:

- daemon 常驻。
- CLI 只发本地 socket 请求，不做重初始化。
- VM 状态缓存，但必须有 health check。
- guest agent 长连接复用。

预热优化:

- VM auto-start 可配置。
- 登录后预热 daemon。
- 最近使用项目预挂载。
- 常用 sandbox base image 预拉取。
- containerd 常驻。

恢复优化:

- VM suspend/resume。
- Apple VZ 快速恢复。
- Hyper-V checkpoint 或 saved state。
- image prefetch 和 cache warm path。
- overlayfs upperdir 复用。

镜像和 guest 优化:

- 自定义 Utility VM image。
- 精简 init。
- 并行启动 guest services。
- 减少 cloud-init 或完全不用 cloud-init。
- 自研 host-guest control channel。

### 10.3 性能测试

每个性能目标对应 benchmark:

```bash
cargo xtask bench daemon-start
cargo xtask bench vm-start --provider lima
cargo xtask bench sandbox-exec --image cratebay/agent-base:latest
cargo xtask bench diff --files 5000
cargo xtask bench cli-status
```

输出 JSON:

```json
{
  "benchmark": "sandbox-exec",
  "provider": "lima",
  "platform": "macos-aarch64",
  "p50_ms": 420,
  "p95_ms": 780,
  "threshold_ms": 1500,
  "status": "pass"
}
```

CI 中:

- PR 运行轻量 benchmark。
- nightly 运行完整 provider benchmark。
- self-hosted macOS/Windows 机器运行真实 VM benchmark。
- 结果保存在 `target/criterion` 和 JSON artifact。

## 11. CLI 设计

### 11.1 命令结构

```bash
cratebay --version
cratebay doctor
cratebay daemon start
cratebay daemon status
cratebay daemon stop

cratebay vm init
cratebay vm start
cratebay vm stop
cratebay vm status
cratebay vm logs

cratebay guest ping [--provider <PROVIDER>]
cratebay guest info [--provider <PROVIDER>]
cratebay guest diagnostics [--provider <PROVIDER>]

cratebay provider list
cratebay provider capabilities [--provider <PROVIDER>]
cratebay provider diagnostics [--provider <PROVIDER>]
cratebay provider migrate --from <PROVIDER> --to <PROVIDER>

cratebay sandbox run [OPTIONS] -- <COMMAND>...
cratebay sandbox create [OPTIONS]
cratebay sandbox exec <SESSION> -- <COMMAND>...
cratebay sandbox list
cratebay sandbox logs <SESSION>
cratebay sandbox diff <SESSION>
cratebay sandbox destroy <SESSION>

cratebay snapshot create <SESSION>
cratebay snapshot list
cratebay snapshot restore <SNAPSHOT>
cratebay snapshot destroy <SNAPSHOT>

cratebay policy list
cratebay policy show <NAME>
cratebay policy validate <FILE>

cratebay image list
cratebay image pull <IMAGE>
cratebay image prefetch --profile <PROFILE>

cratebay mcp serve
cratebay mcp status

cratebay diagnostics collect
```

### 11.2 开发期命令入口

文档中的验收命令统一写成 `cratebay ...`，但空仓库开发期不能假设用户已经安装 release binary。所有本地验收脚本必须先运行:

```bash
cargo xtask dev-bin
export PATH="$PWD/target/dev-bin:$PATH"
cratebay --version
```

`cargo xtask dev-bin` 必须:

- 构建 `cratebay-cli`、`cratebay-daemon` 和需要的本地 sidecar。
- 在 `target/dev-bin` 生成 `cratebay`、`cratebayd`、`cratebay-mcp` 的开发期 shim。
- macOS/Linux 使用 symlink 或小 wrapper；Windows 生成 `.cmd` 或 `.ps1` shim。
- 不修改用户全局 PATH，不要求 `cargo install`。
- 在 `cargo xtask verify --tier 0` 中验证 shim 可用。

### 11.3 全局参数

```bash
--json
--lang zh-CN|en-US
--config <PATH>
--socket <PATH>
--log-level trace|debug|info|warn|error
```

规则:

- 文档中的规范写法是 `cratebay --json <command> ...`。
- CLI 实现必须把 `--json` 设计成真正 global arg，并允许出现在任意子命令层级；文档示例统一使用前置写法。
- `--json` 下只输出 JSON 到 stdout。
- 人类日志输出到 stderr。
- 错误也必须是稳定 JSON。

错误输出:

```json
{
  "error": {
    "code": "VM_NOT_RUNNING",
    "message": "VM is not running",
    "details": {
      "provider": "lima"
    }
  }
}
```

### 11.4 Agent 兼容规则

Agent 只依赖:

- exit code
- stdout JSON
- stable error code
- JSON schema

不要让 agent 解析人类文本。

## 12. Agent 集成方案

### 12.1 CLI JSON mode

核心必做。

优点:

- 任意 agent 都能调用。
- 不需要 SDK。
- 容易调试和记录。

示例:

```bash
cratebay --json sandbox run \
  --profile agent-restricted \
  --mount "$PWD:/workspace" \
  -- npm test
```

### 12.2 Local daemon API

核心必做。

用途:

- GUI 调用。
- 高级 agent 直接调用。
- IDE 插件调用。

连接:

```text
macOS: unix://$HOME/Library/Application Support/CrateBay/cratebayd.sock
Windows: npipe:////./pipe/cratebayd
```

### 12.3 MCP Server

完整核心稳定后做，见 `16.13 Agent MCP and embedded sidecar`。

工具:

- `sandbox_create`
- `sandbox_exec`
- `sandbox_logs`
- `sandbox_diff`
- `sandbox_destroy`
- `policy_list`
- `image_prefetch`

MCP server 不直接实现 sandbox，只是调用 daemon API。

### 12.4 Embedded sidecar

完整核心稳定后做，见 `16.13 Agent MCP and embedded sidecar`。

目标:

- 让 agent 把 `cratebay` CLI 作为内置 sandbox backend。
- agent 不需要知道 Lima、WSL2、Apple VZ、Hyper-V。
- agent 只关心 `cratebay --json sandbox run`。

## 13. GUI 设计

### 13.1 页面

Dashboard:

- daemon 状态
- VM 状态
- provider
- CPU/memory
- 最近 sandbox
- 最近错误

Sandboxes:

- session id
- profile
- command
- status
- duration
- exit code
- changed files
- actions: logs, diff, destroy

Policies:

- profile 列表
- policy JSON viewer
- validate result
- default profile selector

Images:

- base images
- language images
- cached / missing / pulling
- prefetch action

Settings:

- language
- provider
- resource defaults
- auto-start VM
- telemetry off by default

Diagnostics:

- collect bundle
- provider checks
- guest checks
- copy summary

### 13.2 双语

语言:

- `zh-CN`
- `en-US`

原则:

- UI label 走 i18n key。
- CLI 人类文本走 i18n。
- JSON API 不翻译。
- 错误码不翻译。

i18n key 示例:

```json
{
  "dashboard.vmStatus": "VM 状态",
  "sandbox.status.exited": "已退出",
  "policy.agentRestricted.name": "Agent 受限模式"
}
```

## 14. 状态和文件布局

### 14.1 Host 文件布局

macOS:

```text
~/Library/Application Support/CrateBay/
  config.toml
  cratebayd.sock
  state.db
  logs/
  policies/
  images/
  providers/
```

Windows:

```text
%LOCALAPPDATA%\CrateBay\
  config.toml
  state.db
  logs\
  policies\
  images\
  providers\
```

### 14.2 Guest 文件布局

```text
/etc/cratebay/
  guestd.toml
/var/lib/cratebay/
  sandboxes/
  images/
  mounts/
  cache/
/var/log/cratebay/
  guestd.log
  sandboxes/
/run/cratebay/
  guestd.sock
```

### 14.3 State database

SQLite tables:

- `daemon_metadata`
- `vm_instances`
- `sandbox_sessions`
- `sandbox_commands`
- `snapshots`
- `policy_events`
- `images`
- `benchmarks`
- `diagnostics`

核心路线可以先用 SQLite，避免复杂服务依赖。

## 15. AI 单主线交付范围

这个项目由个人开发者主要依赖 AI 开发，所以文档不要把工作拆成过多产品阶段。AI 应该按一个连续主线开发，始终保持项目可构建、可测试、可运行。

执行原则:

- 先完成第一验收闭环，但所有接口和目录结构都按最终目标设计。
- 每完成一个能力，立即补齐自动化测试。
- 每次提交前运行 `cargo xtask verify --tier 0`。
- 涉及真实 provider 时，再运行对应 provider smoke test。
- 不因为 AppleVzProvider/HyperVProvider 较难就破坏上层抽象。
- 不因为 GUI 还没完成就阻塞 CLI/API/sandbox 主线。
- 不手动验证功能，所有验收都必须有命令或测试。

### 15.1 第一验收闭环

第一验收闭环是 AI 开发的第一个稳定检查点，不是缩减版产品。

第一验收闭环允许使用 FakeProvider 模拟 container-level sandbox contract，以便无 VM 环境也能自动验证 CLI/API/policy/diff/audit。`16.5 Guest agent contract and sandbox RPC` 必须打通 host daemon 到 guest 的控制协议；真实 containerd + runc/crun runner 和最小镜像必须在 `16.6 Minimal image and container runtime` 中完成。FakeProvider 不得成为产品执行路径的降级实现。

必须完成:

- Rust workspace。
- `cratebay-core`。
- `cratebay-protocol`。
- `cratebay-provider`。
- `cratebay-provider-fake`。
- `cratebay-daemon`。
- `cratebay-cli`。
- `cargo xtask dev-bin`。
- policy schema。
- `cratebay --json sandbox run`。
- diff scanner。
- audit event。
- GUI scaffold。
- `cargo xtask verify --tier 0`。

第一验收闭环验收:

```bash
cargo xtask verify --tier 0 --check-deps
pnpm --dir apps/desktop install --frozen-lockfile
cargo xtask dev-bin
export PATH="$PWD/target/dev-bin:$PATH"
cargo xtask verify --tier 0
cratebay --json sandbox run --provider fake --profile agent-restricted -- echo hello
SESSION_JSON="$(cratebay --json sandbox run --provider fake --profile agent-offline -- sh -c 'echo changed > changed.txt')"
SESSION_ID="$(printf '%s' "$SESSION_JSON" | jq -r '.session_id')"
test -n "$SESSION_ID" && test "$SESSION_ID" != "null"
cratebay --json sandbox list
cratebay --json sandbox logs "$SESSION_ID"
cratebay --json sandbox diff "$SESSION_ID"
cratebay --json sandbox destroy "$SESSION_ID"
pnpm --dir apps/desktop test
```

说明:

- 第一验收闭环脚本需要 `rustc`、`cargo`、`node`、`pnpm`、`jq` 和 Playwright package。`cargo xtask verify --tier 0 --check-deps` 必须检测这些依赖是否可用，缺失时返回明确的 dependency error。

### 15.2 完整核心交付范围

完整核心交付必须包含:

- CLI。
- daemon。
- local API。
- FakeProvider。
- guest agent。
- sandbox runner。
- policy enforcement。
- audit log。
- file diff。
- image catalog。
- minimal sandbox base image。
- LimaProvider。
- Wsl2Provider。
- Tauri GUI。
- Chinese/English i18n。
- automated verification。
- startup/performance benchmarks。
- diagnostics bundle。

### 15.3 完整路线交付范围

完整路线继续包含:

- AppleVzProvider。
- macOS Rosetta for Linux support。
- HyperVProvider。
- MCP server。
- embedded sidecar mode。
- provider migration tests。
- sandbox snapshot/restore。
- VM suspend/resume or saved state experiments。
- optimized utility VM image。
- stronger agent isolation profile。

这些不是“可有可无”，而是路线的一部分。区别只是执行顺序靠后。

### 15.4 可选插件范围

以下能力不进入核心路线，可以在核心稳定后作为 optional plugin:

- Kubernetes / k3s。
- Docker API compatibility。
- automatic local domain。
- automatic HTTPS certificates。
- USB、音频、GPU passthrough。
- 多发行版 Linux machines。
- 企业级策略管理。

## 16. AI 执行清单

AI 后续开发时从本章开始，从上到下执行。每个任务块都必须满足 Definition of Done，并保持所有已完成测试继续通过。

### 16.1 Bootstrap repository

目标:

- 空仓库变成可构建、可测试、可由 AI 持续开发的工程。

实现:

- 创建 Cargo workspace。
- 创建 `.cargo/config.toml`，包含 `xtask = "run -p cratebay-xtask --"` alias。
- 创建 `README.md`、`README.zh.md`、`LICENSE`。
- 创建 `cratebay-core`、`cratebay-protocol`、`cratebay-provider`。
- 创建 `cratebay-cli`、`cratebay-daemon`、`cratebay-provider-fake`。
- 创建 `cratebay-xtask`。
- 实现 `cargo xtask dev-bin`，生成开发期 `cratebay` / `cratebayd` / `cratebay-mcp` shim。
- 创建 `apps/desktop` Tauri v2 + React + TypeScript scaffold。
- 创建 `apps/desktop/package.json`、`apps/desktop/pnpm-lock.yaml`、`apps/desktop/vite.config.ts`。
- 创建 GUI daemon mock client。
- 创建 GUI i18n skeleton。
- 创建 `policies/dev-open.json`、`policies/agent-restricted.json`、`policies/agent-offline.json`。
- 创建基础 CI。

验收:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p cratebay-cli -- --version
cargo xtask verify --tier 0 --check-deps
pnpm --dir apps/desktop install --frozen-lockfile
cargo xtask dev-bin
export PATH="$PWD/target/dev-bin:$PATH"
cargo xtask verify --tier 0
pnpm --dir apps/desktop test
```

自动测试:

- workspace compiles。
- CLI version 输出稳定。
- `cargo xtask` alias 可用。
- `cargo xtask dev-bin` 生成的 `cratebay --version` 可用。
- policy files 是合法 JSON。
- xtask 能输出 JSON report。
- GUI scaffold tests pass。

### 16.2 Protocol and provider contracts

目标:

- 先固定上层协议，保证后续 Lima/WSL2/Apple VZ/Hyper-V 都只是 provider 替换。

实现:

- 定义 Host API request/response。
- 定义 Guest API request/response。
- 定义 event schema。
- 定义 error code。
- 定义 `VmProvider` trait。
- 定义 provider capability。
- 生成 JSON schema。
- 用 `insta` 做 schema snapshot。

验收:

```bash
cargo test -p cratebay-protocol
cargo test -p cratebay-provider
```

自动测试:

- JSON schema snapshot。
- error code 不重复。
- provider capability serialization。
- backward compatibility snapshot。

### 16.3 Daemon, CLI, and FakeProvider sandbox

目标:

- 不依赖真实 VM，先让 agent 能调用 `cratebay --json sandbox run`。

实现:

- daemon 启动本地 socket。
- CLI 自动发现 daemon。
- FakeProvider 模拟 VM ready。
- fake sandbox 在临时目录执行命令。
- stdout/stderr/exit code/duration 输出稳定 JSON。
- daemon down 时 CLI 返回 `DAEMON_UNAVAILABLE`。
- command timeout 时返回 `COMMAND_TIMEOUT`。

验收:

```bash
cratebay daemon start
cratebay --json daemon status
cratebay --json sandbox run --provider fake -- echo hello
cratebay --json sandbox run --provider fake -- sh -c 'exit 42'
```

自动测试:

- CLI JSON 合法。
- exit 0。
- exit 42。
- stdout/stderr capture。
- timeout。
- daemon unavailable。
- malformed request。

### 16.4 Policy, audit, and diff

目标:

- `agent-restricted` 能成为默认 agent sandbox 策略。

实现:

- policy parser。
- policy schema validation。
- env allowlist。
- filesystem allow/deny model。
- network `allow` / `deny`。
- timeout。
- CPU/memory/pid limit model。
- audit log。
- diff scanner。
- path traversal 和 symlink escape 检测。

核心执行子集:

- `allow_hosts` 必须在核心路线中强制执行；可以先用 guest DNS proxy 或 daemon-side proxy 实现。
- 第一轮网络支持 `allow`、`deny` 和 `allow_hosts` 域名白名单。
- diff 先用文件 hash scanner，overlayfs diff 后续替换。

验收:

```bash
cratebay policy validate policies/agent-restricted.json
SESSION_JSON="$(cratebay --json sandbox run --provider fake --profile agent-offline -- sh -c 'echo changed > changed.txt')"
SESSION_ID="$(printf '%s' "$SESSION_JSON" | jq -r '.session_id')"
cratebay --json sandbox run --provider fake --profile agent-offline -- env
cratebay --json sandbox diff "$SESSION_ID"
```

自动测试:

- invalid policy rejected。
- disallowed env not visible。
- network deny event generated。
- `allow_hosts` 只允许白名单域名访问。
- timeout kill。
- file create/update/delete diff。
- symlink escape rejected。
- audit row exists。

### 16.5 Guest agent contract and sandbox RPC

目标:

- 让 host daemon 通过结构化协议控制 Linux guest，并把 sandbox 生命周期建成稳定 RPC contract。

实现:

- 创建 `cratebay-guestd`。
- 支持 `guest.ping`、`guest.info`。
- 支持 `sandbox.create`、`sandbox.exec`、`sandbox.logs`、`sandbox.diff`、`sandbox.destroy`。
- 支持 stdout/stderr stream。
- 支持 kill long-running process。
- 支持 guest diagnostics。
- 定义真实 container runner contract，16.6 接入 containerd + runc/crun。
- FakeProvider 只模拟同一套 contract，不作为产品降级路径。

验收:

```bash
cratebay guest ping --provider fake
cratebay --json sandbox run --provider fake -- pwd
```

自动测试:

- guest API contract。
- sandbox RPC contract。
- streaming order。
- process kill。
- timeout。
- diff。
- diagnostics。

### 16.6 Minimal image and container runtime

目标:

- 为 container-level sandbox 提供最小可用镜像、缓存和预取能力。

实现:

- image catalog。
- embedded tiny `cratebay/agent-base:latest` metadata。
- containerd integration。
- runc/crun integration。
- minimal sandbox base image。
- 每个 sandbox session 使用独立 mount namespace、pid namespace、network namespace、cgroups 和 overlay upperdir。
- `local-container` 测试 harness: 只在 Linux CI/dev host 上直接连接本机 containerd/runc/crun，用来验证真实 container runner，不作为面向用户的产品 provider。
- image cache status。
- `cratebay --json image list`。
- `cratebay image prefetch --profile agent-restricted`。

验收:

```bash
cratebay --json image list
cratebay image prefetch --profile agent-restricted
cargo xtask image test sandbox-base --name agent-base
cargo xtask verify --tier 1 --provider local-container
```

说明:

- `local-container` 只解决 16.6 的顺序问题，让 container runner 在 Lima/WSL2 接入前也有真实自动化测试。
- 如果当前机器不是 Linux 或缺少 containerd/runc/crun，该测试必须 skipped/blocked 并给出明确 reason，不能伪装 pass。
- macOS/Windows 的真实用户路径在 16.7 LimaProvider 和 16.8 Wsl2Provider 中继续验收。

自动测试:

- image catalog schema。
- missing image。
- cache hit/miss。
- local-container runner smoke。
- image pull failure。

### 16.7 macOS LimaProvider

目标:

- macOS 上真实 Linux VM 能跑 CrateBay sandbox。

实现:

- 检测 `limactl`。
- 生成 Lima template。
- Apple Silicon 优先 `vmType: vz`、`arch: aarch64`。
- Intel Mac 使用 `vmType: vz`、`arch: x86_64`。
- 支持 Rosetta capability 检测。
- 创建 instance。
- 启动/停止/status。
- 同步 `cratebay-guestd`。
- 启动 guestd service。
- workspace mount。
- host 到 guest 连接。
- provider diagnostics。

验收:

```bash
cratebay vm init --provider lima
cratebay vm start --provider lima
cratebay guest ping --provider lima
cratebay --json sandbox run --provider lima -- uname -a
cratebay --json sandbox run --provider lima --image cratebay/agent-base:latest -- echo hello
```

自动测试:

- Lima availability。
- VM lifecycle。
- guest ping。
- workspace read/write。
- sandbox exec。
- containerd sandbox smoke。
- diff。
- Intel Mac guest arch。
- Apple Silicon guest arch。
- Rosetta capability on Apple Silicon。

### 16.8 Windows Wsl2Provider

目标:

- Windows 上 WSL2 guest 能跑 CrateBay sandbox。

实现:

- 检测 WSL2。
- 创建或导入 CrateBay distro。
- 安装 `cratebay-guestd`。
- 启动 guestd。
- workspace mount。
- host 到 guest 连接。
- provider diagnostics。

验收:

```powershell
cratebay vm init --provider wsl2
cratebay vm start --provider wsl2
cratebay guest ping --provider wsl2
cratebay --json sandbox run --provider wsl2 -- uname -a
cratebay --json sandbox run --provider wsl2 --image cratebay/agent-base:latest -- echo hello
```

自动测试:

- WSL2 availability。
- distro import。
- guest ping。
- sandbox exec。
- containerd sandbox smoke。
- network deny。
- diff。

### 16.9 Tauri GUI

目标:

- GUI 只调用 daemon API，不直接控制 provider。

实现:

- 创建 Tauri v2 app。
- React + TypeScript + Vite。
- daemon client。
- Dashboard。
- Sandboxes。
- Logs。
- Policies。
- Images。
- Settings。
- Diagnostics。
- `zh-CN` / `en-US`。

验收:

```bash
cargo xtask gui test
cargo xtask gui e2e --provider fake
```

自动测试:

- component tests。
- store tests。
- i18n tests。
- Playwright fake API。
- logs streaming render。
- error state。

### 16.10 Performance and startup optimization

目标:

- 把“类似 OrbStack 的极致启动速度”变成可量化指标。

实现:

- daemon cold start benchmark。
- CLI status benchmark。
- VM warm/cold start benchmark。
- sandbox warm exec benchmark。
- diff benchmark。
- image cache benchmark。
- prewarm strategy。
- performance warning。

验收:

```bash
cargo xtask bench daemon-start
cargo xtask bench cli-status
cargo xtask bench sandbox-exec
cargo xtask bench vm-start --provider lima
```

自动测试:

- benchmark JSON report。
- threshold pass/fail。
- regression detection。

### 16.11 AppleVzProvider

目标:

- 替换 macOS 的长期底层 VM provider。

实现:

- 创建 `cratebay-provider-apple-vz`。
- 创建 `native/apple-vz-helper` Swift package。
- Swift helper。
- Rust daemon 到 Swift helper 的协议。
- VM config。
- disk image。
- VirtioFS mount。
- network。
- guest control channel。
- Intel Mac x86_64 guest。
- Apple Silicon arm64 guest。
- Rosetta for Linux。
- suspend/resume。
- migration from Lima where possible。

验收:

```bash
cratebay vm init --provider apple-vz
cratebay vm start --provider apple-vz
cratebay guest ping --provider apple-vz
cratebay --json sandbox run --provider apple-vz -- echo hello
```

自动测试:

- self-hosted macOS runner。
- VM lifecycle。
- VirtioFS mount。
- Intel Mac arch test。
- Apple Silicon arch test。
- Rosetta amd64 container。
- suspend/resume。
- migration from Lima。
- performance benchmark。
- diagnostics。

### 16.12 HyperVProvider

目标:

- 替换 Windows 的强隔离 provider。

实现:

- 创建 `cratebay-provider-hyperv`。
- Hyper-V availability detection。
- VHDX image。
- VM create/start/stop/status。
- virtual switch。
- workspace mount strategy。
- guest agent channel。
- saved state or checkpoint experiments。
- diagnostics。

验收:

```powershell
cratebay vm init --provider hyperv
cratebay vm start --provider hyperv
cratebay guest ping --provider hyperv
cratebay --json sandbox run --provider hyperv -- echo hello
```

自动测试:

- self-hosted Windows runner。
- VM lifecycle。
- workspace mount。
- port forwarding。
- policy enforcement。
- saved state/checkpoint。
- performance benchmark。

### 16.13 Agent MCP and embedded sidecar

目标:

- 让 AI Agent 可以把 CrateBay 当成标准 sandbox backend。

实现:

- `cratebay-mcp`。
- `sandbox_create`。
- `sandbox_exec`。
- `sandbox_logs`。
- `sandbox_diff`。
- `sandbox_destroy`。
- policy error propagation。
- embedded CLI sidecar mode。

验收:

```bash
cargo xtask mcp smoke --provider fake
cratebay --json sandbox run -- echo hello
```

自动测试:

- MCP tool list。
- sandbox create/exec/diff/destroy。
- JSON schema。
- policy denied。
- daemon unavailable。

### 16.14 Sandbox snapshot and restore

目标:

- 让 agent sandbox session 可以保存、恢复和销毁快照，支撑可恢复执行。

实现:

- `snapshot.create`、`snapshot.list`、`snapshot.restore`、`snapshot.destroy` Host API。
- `cratebay snapshot create <SESSION>`。
- `cratebay snapshot list`。
- `cratebay snapshot restore <SNAPSHOT>`。
- `cratebay snapshot destroy <SNAPSHOT>`。
- snapshot metadata 存入 SQLite。
- 第一版 snapshot 可以使用 overlay upperdir + metadata tarball；后续 provider 可替换为 VM snapshot/checkpoint。
- restore 后必须保持 workspace mount policy、network policy、env allowlist 和 audit chain 一致。

验收:

```bash
SESSION_JSON="$(cratebay --json sandbox run --provider fake --profile agent-offline -- sh -c 'echo before > state.txt')"
SESSION_ID="$(printf '%s' "$SESSION_JSON" | jq -r '.session_id')"
SNAPSHOT_JSON="$(cratebay --json snapshot create "$SESSION_ID")"
SNAPSHOT_ID="$(printf '%s' "$SNAPSHOT_JSON" | jq -r '.snapshot_id')"
test -n "$SNAPSHOT_ID" && test "$SNAPSHOT_ID" != "null"
cratebay --json snapshot list
cratebay --json snapshot restore "$SNAPSHOT_ID"
cratebay --json snapshot destroy "$SNAPSHOT_ID"
```

自动测试:

- snapshot create。
- snapshot list。
- snapshot restore。
- snapshot destroy。
- restore 后 policy 不被绕过。
- restore 后 audit chain 连续。
- missing snapshot 返回 `SNAPSHOT_NOT_FOUND`。

### 16.15 Packaging, release, and diagnostics

目标:

- 用户能安装、升级、卸载、诊断。

实现:

- macOS app package。
- Windows installer。
- CLI binary release。
- guestd release bundle。
- image metadata release。
- diagnostics bundle。
- upgrade test。
- uninstall test。

验收:

```bash
cargo xtask release-check
cratebay diagnostics collect
```

自动测试:

- artifact exists。
- signature/checksum。
- install smoke。
- upgrade smoke。
- uninstall smoke。

## 17. 从 Lima/WSL2 迁移到 Apple VZ/Hyper-V

### 17.1 迁移能否好做

能否好做取决于隔离程度。

容易迁移的前提:

- 上层只依赖 `VmProvider`。
- sandbox 只依赖 guest API。
- CLI/GUI/Agent 不知道 provider 细节。
- 所有 provider 都必须通过同一套 contract tests。

难迁移的情况:

- CLI 直接调用 `limactl shell`。
- GUI 直接调用 `wsl.exe`。
- sandbox 逻辑写在 provider 里。
- policy enforcement 依赖 WSL2 特有行为。
- session path 使用 provider-specific path。

### 17.2 迁移范围

迁移时需要替换:

- VM 创建。
- VM 启动/停止。
- VM 镜像。
- workspace mount。
- 端口转发。
- host 到 guest 的连接方式。
- guestd 安装方式。
- suspend/resume。

不应该替换:

- CLI 命令。
- JSON 输出。
- GUI 页面。
- policy schema。
- sandbox session model。
- guest API。
- diff 输出。
- audit log。

### 17.3 Provider contract tests

每个 provider 必须通过同一组测试:

```text
provider.init
provider.start
provider.status
provider.capabilities
provider.dependency_missing
provider.guest_ping
provider.workspace_mount_rw
provider.workspace_mount_ro
provider.port_forward
provider.sandbox_exec
provider.sandbox_timeout
provider.network_deny
provider.diff
provider.diagnostics
provider.unsupported_or_skipped_semantics
provider.stop
```

macOS provider 还必须通过:

```text
provider.macos_intel_guest_arch
provider.macos_apple_silicon_guest_arch
provider.rosetta_capability
provider.rosetta_amd64_container
provider.rosetta_missing_error
```

只有 contract tests 通过，provider 才能标记为 supported。

## 18. 自动化测试总纲

### 18.1 测试层级

Tier 0: No VM tests

- 任何机器都能跑。
- 不需要 Lima、WSL2、Hyper-V、Apple VZ。
- PR 必跑。

包括:

- Rust unit tests。
- protocol schema tests。
- policy validation tests。
- CLI JSON tests。
- FakeProvider e2e。
- GUI unit tests。

Tier 1: Local provider smoke

- 开发机器或 CI 有对应环境时跑。
- Linux 跑 `local-container` test harness。
- macOS 跑 Lima。
- Windows 跑 WSL2。

Tier 2: Real VM provider full

- self-hosted runners。
- Apple VZ。
- Hyper-V。
- 完整 VM lifecycle。

Tier 3: Performance and stress

- nightly 或手动 release 前自动跑。
- 启动速度。
- 多 session。
- 大 workspace diff。
- 镜像拉取。

Tier 4: Security regression

- policy bypass 测试。
- network deny。
- env leak。
- mount escape。
- symlink/path traversal。

### 18.2 统一测试入口

必须提供:

```bash
cargo xtask dev-bin
cargo xtask verify
cargo xtask verify --tier 0 --check-deps
cargo xtask verify --tier 0
cargo xtask verify --tier 1 --provider local-container
cargo xtask verify --tier 1 --provider lima
cargo xtask verify --tier 1 --provider wsl2
cargo xtask verify --tier 2 --provider apple-vz
cargo xtask verify --tier 2 --provider hyperv
cargo xtask image build utility-vm --target <TARGET>
cargo xtask image build sandbox-base --name <NAME>
cargo xtask image test sandbox-base --name <NAME>
cargo xtask image sbom sandbox-base --name <NAME>
cargo xtask gui test
cargo xtask gui e2e --provider fake
cargo xtask mcp smoke --provider fake
cargo xtask bench
cargo xtask bench daemon-start
cargo xtask bench cli-status
cargo xtask bench sandbox-exec [--image <IMAGE>]
cargo xtask bench vm-start --provider <PROVIDER>
cargo xtask bench diff --files <N>
cargo xtask release-check
```

输出:

- 人类可读 summary。
- 机器可读 JSON report。

示例:

```json
{
  "status": "pass",
  "platform": "macos-aarch64",
  "tests": [
    {
      "id": "SANDBOX_001",
      "name": "sandbox run echo",
      "status": "pass",
      "duration_ms": 120
    },
    {
      "id": "PROVIDER_LIMA_001",
      "name": "lima provider start",
      "status": "skipped",
      "reason": "limactl not installed"
    }
  ]
}
```

规则:

- 不支持的 provider 可以 skipped，但不能伪装 pass。
- release 前 supported provider 不允许 skipped。
- 如果用户机器缺少 Hyper-V，测试报告必须明确显示 blocked/skipped reason。

### 18.3 能力到测试矩阵

| 能力 | 测试 ID | 自动测试方式 | 验收 |
|---|---|---|---|
| daemon 启动 | DAEMON_001 | `cratebay daemon status` | 返回 running |
| daemon socket | DAEMON_002 | JSON-RPC ping | schema 正确 |
| doctor run | DOCTOR_001 | `cratebay --json doctor` | 平台能力和依赖检测正确 |
| local dependencies | DEPS_001 | `cargo xtask verify --tier 0 --check-deps` | `rustc`、`cargo`、`node`、`pnpm`、`jq`、Playwright package、`cargo xtask` alias 缺失时报明确错误 |
| dev binary shims | DEV_BIN_001 | `cargo xtask dev-bin` | `target/dev-bin/cratebay --version` 可执行 |
| CLI JSON | CLI_001 | `assert_cmd` | stdout 是合法 JSON |
| CLI 错误 | CLI_002 | daemon down | 返回 `DAEMON_UNAVAILABLE` |
| policy validate | POLICY_001 | schema test | valid pass |
| invalid policy | POLICY_002 | schema test | invalid fail |
| env allowlist | POLICY_003 | sandbox env | 未授权 env 不存在 |
| timeout | POLICY_004 | `sleep 999` | 超时 kill |
| resource limit | POLICY_005 | memory stress | 被限制或报错 |
| network deny | POLICY_006 | curl external | 被拒绝 |
| workspace mount rw | FS_001 | 写文件 | host 可见 |
| workspace mount ro | FS_002 | 写文件 | permission denied |
| path traversal | FS_003 | symlink escape | 被拒绝 |
| diff scanner | DIFF_001 | 修改文件 | changed_files 正确 |
| delete diff | DIFF_002 | 删除文件 | deleted_files 正确 |
| binary diff | DIFF_003 | 修改 binary | hash change 正确 |
| guest ping | GUEST_001 | guest API | pong |
| guest exec | GUEST_002 | `echo hello` | exit 0 |
| guest logs | GUEST_003 | stdout/stderr | stream 顺序正确 |
| kill command | GUEST_004 | long process | process killed |
| fake provider | PROVIDER_FAKE_001 | contract tests | 全 pass |
| provider capabilities | PROVIDER_CAP_001 | contract tests | capability schema 正确 |
| provider dependency missing | PROVIDER_DEP_001 | fault injection | 返回 `PROVIDER_DEPENDENCY_MISSING` |
| provider diagnostics | PROVIDER_DIAG_001 | contract tests | diagnostics bundle 正确 |
| provider migration | PROVIDER_MIGRATE_001 | migration contract test | 上层 CLI/API/GUI 输出不变 |
| Lima start | PROVIDER_LIMA_001 | provider smoke | VM running |
| Lima mount | PROVIDER_LIMA_002 | workspace rw | 文件同步 |
| WSL2 start | PROVIDER_WSL2_001 | provider smoke | distro running |
| WSL2 mount | PROVIDER_WSL2_002 | workspace rw | 文件同步 |
| Apple VZ start | PROVIDER_APPLE_VZ_001 | self-hosted macOS smoke | VM running |
| Apple VZ mount | PROVIDER_APPLE_VZ_002 | self-hosted macOS smoke | VirtioFS rw |
| Apple VZ sandbox | PROVIDER_APPLE_VZ_003 | self-hosted macOS smoke | sandbox echo pass |
| Hyper-V start | PROVIDER_HYPERV_001 | self-hosted Windows smoke | VM running |
| Hyper-V mount | PROVIDER_HYPERV_002 | self-hosted Windows smoke | workspace rw |
| Hyper-V sandbox | PROVIDER_HYPERV_003 | self-hosted Windows smoke | sandbox echo pass |
| macOS Intel arch | MAC_ARCH_001 | macOS provider smoke | guest arch 为 `x86_64` |
| macOS Apple Silicon arch | MAC_ARCH_002 | macOS provider smoke | guest arch 为 `aarch64` |
| no default x86 VM on Apple Silicon | MAC_ARCH_003 | macOS provider smoke | 不创建默认 x86_64 整机 VM |
| native amd64 on Intel Mac | MAC_ARCH_004 | macOS provider smoke | 不经过 Rosetta |
| Rosetta capability | ROSETTA_001 | Apple Silicon provider smoke | capability 正确 |
| amd64 container on arm64 | ROSETTA_002 | Apple Silicon provider smoke | 返回 `x86_64` |
| Rosetta missing | ROSETTA_003 | fault injection | 返回 `ROSETTA_NOT_INSTALLED` |
| Rosetta unsupported | ROSETTA_004 | provider capability test | capability false 或 `ROSETTA_UNSUPPORTED` |
| Rosetta performance warning | ROSETTA_005 | benchmark | 慢路径生成 warning |
| image list | IMAGE_001 | catalog test | schema 正确 |
| image prefetch | IMAGE_002 | fake registry | cache hit |
| container runner smoke | CONTAINER_001 | local-container/Lima/WSL2 containerd smoke | `cratebay/agent-base:latest` 能执行 `echo hello` |
| sandbox run | SANDBOX_001 | fake provider | exit 0 |
| sandbox failed command | SANDBOX_002 | `exit 42` | exit_code 42 |
| sandbox audit | SANDBOX_003 | run command | audit row 存在 |
| sandbox list | SANDBOX_LIST_001 | fake provider | session 列表正确 |
| sandbox logs | SANDBOX_LOGS_001 | fake provider | stdout/stderr 可读取 |
| sandbox destroy | SANDBOX_DESTROY_001 | fake provider | session 被销毁且不可再用 |
| snapshot create | SNAPSHOT_001 | fake provider snapshot test | 返回 snapshot_id |
| snapshot list | SNAPSHOT_002 | fake provider snapshot test | 能列出 snapshot |
| snapshot restore | SNAPSHOT_003 | fake provider snapshot test | 文件状态恢复 |
| snapshot destroy | SNAPSHOT_004 | fake provider snapshot test | snapshot 被销毁 |
| snapshot missing | SNAPSHOT_005 | fault injection | 返回 `SNAPSHOT_NOT_FOUND` |
| DNS allowlist | POLICY_DNS_001 | fake/guest network test | 只允许 `allow_hosts` 域名 |
| GUI scaffold | GUI_SCAFFOLD_001 | Vitest | scaffold tests pass |
| GUI dashboard | GUI_001 | Playwright fake API | 状态渲染 |
| GUI logs | GUI_002 | Playwright fake API | 日志渲染 |
| GUI i18n | GUI_003 | unit/e2e | 中英切换 |
| MCP tool list | MCP_001 | MCP protocol test | 工具列表完整 |
| MCP sandbox exec | MCP_002 | MCP fake provider e2e | sandbox exec/diff/destroy 可用 |
| embedded sidecar | SIDECAR_001 | sidecar integration test | agent 调用 CLI JSON contract |
| performance CLI | PERF_001 | bench | 小于阈值 |
| performance sandbox | PERF_002 | bench | 小于阈值 |
| diagnostics | DIAG_001 | collect | bundle 存在 |
| packaging | PACKAGING_001 | `cargo xtask release-check` | artifact/checksum/signature 存在 |
| upgrade | UPGRADE_001 | release CI install/upgrade smoke | state/schema 正确迁移 |
| uninstall | UNINSTALL_001 | release CI uninstall smoke | 服务和文件清理符合策略 |

### 18.4 Fault injection

必须覆盖:

- daemon crash 后 CLI 报错。
- guestd crash 后 daemon 能重新连接或清晰报错。
- VM stopped 时 sandbox run 返回 `VM_NOT_RUNNING`。
- policy deny 时返回 `POLICY_DENIED`。
- network deny 时返回 `POLICY_DENIED_NETWORK`。
- workspace 不存在时返回 `WORKSPACE_NOT_FOUND`。
- mount 失败时返回 `MOUNT_FAILED`。
- image 不存在时返回 `IMAGE_NOT_FOUND`。
- snapshot 不存在时返回 `SNAPSHOT_NOT_FOUND`。
- command timeout 时返回 `COMMAND_TIMEOUT`。
- provider 依赖缺失时返回 `PROVIDER_DEPENDENCY_MISSING`。
- Apple Silicon Rosetta 未安装时返回 `ROSETTA_NOT_INSTALLED`。
- host 不支持 Rosetta 时返回 `ROSETTA_UNSUPPORTED`。

### 18.5 GUI 自动化

GUI 测试不依赖真实 VM。

做法:

- Tauri command 使用 mock daemon。
- Playwright 连接 fake API。
- 固定 fixtures。

测试:

- 页面加载。
- VM 状态显示。
- sandbox 列表显示。
- 日志流追加。
- policy 切换。
- 语言切换。
- 错误 toast。

### 18.6 Provider 自动化限制

GitHub hosted runner 不一定支持嵌套虚拟化和 Hyper-V。策略:

- PR 必跑 Tier 0。
- macOS provider smoke 可在本地或 self-hosted macOS 跑。
- Windows WSL2 provider 可在支持 WSL2 的 runner 跑。
- Apple VZ 和 Hyper-V full tests 必须使用 self-hosted runner。
- 测试报告必须标记 unsupported/skipped，不允许静默跳过。

## 19. CI/CD 方案

### 19.1 PR CI

每个 PR:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask verify --tier 0 --check-deps
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop exec playwright install --with-deps
cargo xtask verify --tier 0
pnpm --dir apps/desktop test
pnpm --dir apps/desktop exec playwright test --project=fake
```

### 19.2 Nightly CI

每天:

- Tier 0 full。
- local-container smoke。
- LimaProvider smoke。
- WSL2Provider smoke。
- snapshot/restore smoke。
- MCP smoke。
- performance smoke。
- image build smoke。
- dependency audit。

### 19.3 Release CI

发布前:

- 所有 supported providers contract tests。
- provider migration tests。
- GUI E2E。
- packaging。
- image SBOM。
- benchmark threshold。
- snapshot/restore regression。
- upgrade test。
- uninstall test。

## 20. 安全模型

### 20.1 核心安全声明

Core sandbox:

- 适合运行开发者自己的项目和常见 coding agent 任务。
- 可以减少误操作和环境污染。
- 不应承诺可安全运行恶意代码。

强安全需要:

- per-task VM。
- 默认无网络。
- 没有 host writable mount。
- 严格 secrets broker。
- VM 级销毁。

### 20.2 Threat model

需要防:

- Agent 意外读取 home。
- Agent 意外泄露 env。
- Agent 在不该联网时联网。
- Agent 写出 workspace。
- Agent 命令无限运行。
- Agent 占满 CPU/memory。
- Agent 产生不可追踪修改。

暂不承诺防:

- Linux kernel 逃逸。
- malicious root inside guest。
- side channel。
- 硬件级攻击。

### 20.3 Secrets

规则:

- 默认不注入任何宿主机 secret。
- 只允许 env allowlist。
- secret 不写入日志。
- diagnostics 默认脱敏。
- policy 必须显式声明 secret mount。

## 21. 错误码

错误码必须稳定。

```text
DAEMON_UNAVAILABLE
INVALID_REQUEST
INVALID_POLICY
VM_NOT_INITIALIZED
VM_NOT_RUNNING
VM_START_FAILED
GUEST_UNAVAILABLE
GUEST_AGENT_VERSION_MISMATCH
SANDBOX_NOT_FOUND
SNAPSHOT_NOT_FOUND
COMMAND_TIMEOUT
COMMAND_FAILED
POLICY_DENIED
POLICY_DENIED_NETWORK
POLICY_DENIED_FILESYSTEM
WORKSPACE_NOT_FOUND
MOUNT_FAILED
IMAGE_NOT_FOUND
IMAGE_PULL_FAILED
ROSETTA_NOT_INSTALLED
ROSETTA_UNSUPPORTED
PROVIDER_UNSUPPORTED
PROVIDER_DEPENDENCY_MISSING
INTERNAL_ERROR
```

错误 JSON:

```json
{
  "error": {
    "code": "POLICY_DENIED_NETWORK",
    "message": "Network access is denied by policy",
    "localized_message": {
      "zh-CN": "当前策略禁止网络访问",
      "en-US": "Network access is denied by policy"
    },
    "details": {
      "profile": "agent-offline",
      "target": "https://example.com"
    }
  }
}
```

## 22. 配置文件

Host config:

```toml
[daemon]
auto_start = true
log_level = "info"

[ui]
language = "zh-CN"

[vm]
provider = "auto"
auto_start = false
cpus = 4
memory_mb = 8192
disk_gb = 80

[sandbox]
default_profile = "agent-restricted"
default_image = "cratebay/agent-base:latest"
keep_failed_sessions = true

[performance]
prewarm_vm = false
prefetch_default_image = true
```

## 23. 版本和兼容性

Protocol version:

```json
{
  "protocol_version": "0.1",
  "daemon_version": "0.1.0",
  "guest_agent_version": "0.1.0",
  "min_guest_agent_version": "0.1.0"
}
```

规则:

- daemon 和 guestd 启动时检查版本。
- minor version 可以向后兼容。
- breaking change 需要 protocol version bump。
- JSON schema 要跟随版本发布。

## 24. 文档计划

需要的文档:

```text
docs/
  DEVELOPMENT_PLAN.zh.md
  architecture/overview.zh.md
  architecture/provider-contract.zh.md
  specs/host-api.zh.md
  specs/guest-api.zh.md
  specs/policy-schema.zh.md
  specs/image-strategy.zh.md
  testing/testing-strategy.zh.md
  testing/provider-contract-tests.zh.md
  testing/performance-benchmarks.zh.md
  user/cli.zh.md
  user/gui.zh.md
  user/agent-integration.zh.md
```

文档要求:

- 中文主文档。
- 关键协议和命令保留英文。
- 每个 spec 都有示例 JSON。
- 每个能力都有测试编号。
- AI agent 可以根据文档生成任务。

## 25. Definition of Done

一个功能完成必须满足:

1. 有明确用户价值。
2. 有 CLI 或 API 入口。
3. 有稳定 JSON 输出或事件。
4. 有错误码。
5. 有日志。
6. 有单元测试。
7. 有 contract 或 e2e 测试。
8. 如果涉及 provider，有 FakeProvider 测试。
9. 如果涉及真实 VM，有 provider smoke 测试。
10. 如果涉及性能，有 benchmark。
11. 如果涉及 GUI，有 component/e2e 测试。
12. 文档更新。

## 26. 推荐第一批开发任务

以第 16 章 `AI 执行清单` 为唯一执行源。

规则:

- AI 开发时从 `16.1 Bootstrap repository` 开始按顺序做。
- 不再维护第二套任务顺序，避免路线漂移。
- 每完成一个任务块，都必须满足该块验收命令和自动测试要求。
- 如果第 16 章和其他章节发生冲突，以第 16 章为准。

## 27. 关键风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| Hyper-V 开发复杂 | Windows final provider 延迟 | WSL2 bootstrap provider 先可用 |
| Apple VZ 细节复杂 | macOS 正式 provider 延迟 | LimaProvider 先验证产品 |
| 文件共享性能差 | 用户体验差 | benchmark，后续 VirtioFS/自研优化 |
| sandbox 被误认为强安全 | 安全预期错误 | 文档明确 security level |
| GUI 复杂交互过早扩张 | 拖慢主线 | GUI scaffold 先行，复杂交互依赖 daemon API 稳定后扩展 |
| 测试依赖真实 VM | CI 不稳定 | FakeProvider + self-hosted 分层 |
| 镜像过大 | 冷启动慢 | utility image 和 sandbox image 分离 |
| Agent JSON 不稳定 | 集成困难 | schema snapshot tests |

## 28. 长期 Roadmap

完整路线后续交付:

- AppleVzProvider。
- HyperVProvider。
- MCP server。
- per-task VM。
- snapshot/restore。
- optimized utility VM image。
- stronger agent isolation profile。

可选插件能力:

- local domain。
- HTTPS dev certificates。
- Docker API compatibility。
- BuildKit。
- image lazy pull。
- rich GUI diff viewer。
- project profiles。
- remote cache。
- Kubernetes/k3s optional plugin。

说明:

- AppleVzProvider、HyperVProvider、MCP server 是完整路线的一部分，不是可有可无的插件。
- Kubernetes、Docker API compatibility、本地域名和 HTTPS 是 optional plugin，不进入核心路线的前置条件。

## 29. 官方参考

这些是架构设计需要持续跟踪的官方资料:

- Tauri: https://v2.tauri.app/
- Apple Virtualization.framework: https://developer.apple.com/documentation/virtualization
- Lima: https://lima-vm.io/docs/
- WSL: https://learn.microsoft.com/en-us/windows/wsl/
- Hyper-V: https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/
- Host Compute Service: https://learn.microsoft.com/en-us/virtualization/api/hcs/overview
- containerd: https://containerd.io/docs/
- OCI specifications: https://opencontainers.org/
- runc: https://github.com/opencontainers/runc

## 30. 最终判断

对个人开发者最现实的路线:

```text
先做自己的 daemon / CLI / guestd / sandbox protocol
用 FakeProvider 保证测试闭环
macOS 用 Lima 让真实 VM 先跑起来
Windows 用 WSL2 让真实 sandbox 先跑起来
等产品模型稳定，再做 Apple VZ 和 Hyper-V
```

CrateBay 的核心资产不是某个 VM backend，而是:

- agent sandbox protocol
- policy model
- audit and diff
- automated verification
- fast local runtime experience
- GUI/CLI/API 一致性

只要这些边界从第一天守住，后续从 Lima/WSL2 迁移到 Apple Virtualization.framework/Hyper-V 是可控的。
