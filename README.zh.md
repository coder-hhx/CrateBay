# CrateBay

开源、跨平台的容器与镜像管理工具。

CrateBay 是 Docker Desktop 和 OrbStack 的桌面替代方案，专注本地容器工作流：浏览和删除镜像、搜索仓库、拉取新镜像、创建容器、查看日志、管理存储卷和网络，并在 macOS、Linux、Windows 上运行内置运行时。

CLI 与内置运行时构成最小可用单元；桌面应用是同一套容器能力之上的可视化控制面。
外部 Docker 兼容端点只作为显式兼容入口支持，默认路径始终是内置运行时。

## 为什么选 CrateBay？

- **开源** — MIT 协议，免费使用
- **跨平台** — 支持 macOS、Linux、Windows
- **内置运行时** — 需要时自动启动本地 VM 容器引擎
- **镜像优先** — 在一个界面里完成搜索、拉取、查看、打标签和删除
- **容器管理** — 创建、启动、停止、查看详情、执行命令和读取日志
- **Pod 分组** — 用 CrateBay/CNI 托管网络管理相关容器分组
- **存储卷和网络** — 可在 GUI 或 CLI 中创建、查看并删除持久化存储卷和托管网络
- **镜像加速源** — 可配置 Docker Hub 镜像源，加快拉取速度

## 快速开始

```bash
cd crates/cratebay-gui
pnpm install
pnpm tauri dev
```

CLI 示例：

```bash
cratebay runtime start
cratebay image search alpine
cratebay image pull alpine:latest
cratebay run alpine:latest -- echo hello
cratebay run --network none --read-only --memory 512 alpine:latest -- sh -lc "pwd && id"
cratebay run --entrypoint /bin/sh alpine:latest -- -lc "echo from custom entrypoint"
cratebay --json run --max-output-bytes 1048576 alpine:latest -- sh -lc "echo bounded output"
cratebay --json run --no-propagate-exit-code alpine:latest -- sh -lc "exit 42"
cratebay image preload-bundled
cratebay image export --output alpine.tar alpine:latest
cratebay image import alpine.tar
cratebay pod create demo-pod
cratebay volume create demo-cache
cratebay network create demo-net
cratebay container create demo --image alpine:latest --entrypoint /bin/sh --command "sleep 3600" --pod demo-pod --publish 8080:80 --volume "$PWD:/workspace:ro"
cratebay image pack-container demo cratebay/demo:latest
cratebay image tag cratebay/demo:latest cratebay/demo:dev
cratebay container list --all
```

## 桌面应用

CrateBay 桌面应用提供：

- **容器** — 生命周期操作、模板、日志、终端和资源详情
- **镜像** — 本地镜像列表、仓库搜索、拉取进度、查看详情、打标签、导出、导入和删除
- **Pod** — 基于网络的容器分组
- **存储卷** — 持久化 Engine 存储卷的生命周期和详情
- **网络** — 托管 Engine 网络的生命周期和详情
- **运行时** — 启动、停止、重启内置引擎，并配置 HTTP 代理
- **设置** — 语言、主题、镜像加速源和运行时连接

## 架构

```
┌─────────────────────────────────────────────────────┐
│  CrateBay                                            │
│                                                      │
│  ┌──────────────┐        ┌───────────────────────┐  │
│  │ GUI App      │        │ cratebay-cli           │  │
│  │ Tauri + React│        │ 命令行                 │  │
│  └──────┬───────┘        └──────────┬────────────┘  │
│         └───────────────┬───────────┘               │
│                         │                           │
│              ┌──────────▼──────────┐                │
│              │   cratebay-core     │                │
│              │ Engine + storage    │                │
│              └──────────┬──────────┘                │
│                         │                           │
│              ┌──────────▼──────────┐                │
│              │  Built-in Runtime   │                │
│              │  macOS: VZ          │                │
│              │  Linux: KVM/QEMU    │                │
│              │  Windows: WSL2      │                │
│              └─────────────────────┘                │
└─────────────────────────────────────────────────────┘
```

**技术栈**：Tauri v2 | React 19 | Rust | containerd | runc | CNI | bollard 兼容客户端 | SQLite

## 对比

| | CrateBay | Docker Desktop | OrbStack | Podman Desktop |
|---|---|---|---|---|
| 开源 | MIT | 否 | 否 | 是 |
| 跨平台 | macOS/Win/Linux | macOS/Win/Linux | 仅 macOS | macOS/Win/Linux |
| 内置运行时 | 是 | 是 | 是 | 是 |
| 镜像管理 | 是 | 是 | 是 | 是 |
| 容器日志/终端 | 是 | 是 | 是 | 是 |
| 成本 | 免费 | 免费 / 付费档 | 免费 / 付费档 | 免费 |

## 状态

v0.9.0 聚焦镜像管理、Pod 分组、存储卷与网络生命周期、容器生命周期、CLI 一次性执行和内置运行时。

## 许可证

[MIT](LICENSE)
