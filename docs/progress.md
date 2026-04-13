# CrateBay 开发进度

## 当前状态
- **版本**: v0.9.0 → v1.0.0（ChatPage v1 开发中）
- **产品定位**: **本地 AI Sandbox** — AI 聊天 + 开箱即用的代码执行沙盒
- **日期**: 2026-03-30
- **Git HEAD**: `feat/chatpage-v1` 分支（基于 master）
- **执行计划**: 见 `docs/chatpage-v1-plan.md`

## 产品方向（AI 必读 — CRITICAL）

> **CrateBay = 开源跨平台容器管理 + AI Sandbox**。两个支柱，都重要。
>
> **支柱一**：容器管理 — 对标 Docker Desktop / OrbStack，开源、跨平台、内置 Runtime
> **支柱二**：AI Sandbox — 对标 E2B / Modal，本地执行、零成本、隐私优先
>
> 导航结构：Chat（AI 聊天）、Containers（容器管理）、Images（镜像管理）、Settings（配置，含 MCP/LLM/Runtime）
>
> **不要移除容器/镜像管理功能，它们是核心功能。**

## Runtime 策略（AI 必读）
- **built-in runtime 是唯一路径**：已移除所有外部 Docker 支持（Colima/OrbStack/Docker Desktop/Podman）
- **不再有 fallback**：CrateBay 只使用自研内置 runtime
- **控制面边界保持 Docker-compatible**：继续围绕 `bollard`、Docker socket/host 语义实现
- **恢复会话时必须先读**：`AGENTS.md`、`docs/specs/runtime-spec.md`、`docs/references/tech-decisions.md` 中的 runtime 策略

## 已完成 ✅
- [x] Step 0: 分支准备 (v1-archive tag, rewrite/v2 分支, 旧代码清理) — 2026-03-20
- [x] Step 1a: 目录结构 + 持久化配置 — 2026-03-20
- [x] Step 1b: 16份技术文档编写 — 2026-03-20
- [x] Step 1c: AGENTS.md 优化 (Spec Loading Protocol + Agent Team 强制规则) — 2026-03-20
- [x] LICENSE 改为 MIT — 2026-03-20
- [x] Step 2: 项目骨架初始化 — 2026-03-20
  - ✅ Rust workspace (4 crate) + 前端 (React 19 + Vite 6.4 + Tailwind v4 + shadcn/ui) + 测试框架
- [x] Step 3: Core 基础设施 — 2026-03-20
  - ✅ backend-dev: SQLite 存储层 (12 张表 + 迁移系统 + CRUD) + Docker 集成 + Container CRUD + Provider/Model 管理
  - ✅ ai-engineer: LLM Proxy (3 种 API 格式 + 双 header + SSE 流式 + /v1/models) ~950 行
  - ✅ backend-dev: 30 个 Tauri Commands 全部注册 + AppState 初始化
  - ✅ tester: 85 个集成测试全部通过 (存储层 30 + LLM 15 + 审计 12 + 内联 28 + Docker 5 ignored)
  - ✅ ai-spec-writer: 5 份 spec 文档更新 (api/agent/database/frontend/backend-spec → v1.1.0)
- [x] Step 4: 前端基础 — 2026-03-20
  - ✅ 第一轮实现 + Spec 偏差修复（step4-fix 团队）
  - ✅ frontend-dev: 4 类型文件 + 3 Chat 组件 + 2 Container 组件 + 3 MCP 组件 + McpPage 完整实现
  - ✅ frontend-dev: Streamdown 集成 + @mention 自动补全 + useContainerActions hook + 6 Store 补齐 + i18n 系统
  - ✅ ai-engineer: 17 个 Agent Tools (8 container + 3 filesystem + 1 shell + 2 mcp + 3 system) + systemPrompt 独立文件
  - ✅ backend-dev: 8 个新 Tauri Commands + 5 项签名/命名修复 + llm_proxy_cancel (CancellationToken)
  - ✅ architect: 8/8 大类零偏差验收通过
  - ✅ tester: 85 cargo tests + 50 vitest tests 全部通过，修复 4 个问题
  - ✅ Spec 文档同步更新: agent-spec.md v1.1.0→v1.2.0, api-spec.md 命名统一, AGENTS.md/frontend-spec.md Vite 6.x
- [x] Step 5: 内置 Runtime (VZ/KVM/WSL2) — 2026-03-20
  - ✅ runtime-dev: RuntimeManager trait + 核心类型 (RuntimeState/ProvisionProgress/HealthStatus/RuntimeConfig) + detect_external_docker + start_health_monitor
  - ✅ runtime-dev: MacOSRuntime (VZ.framework) — detect/provision/start/stop/health_check 完整框架
  - ✅ backend-dev: LinuxRuntime (KVM/QEMU) — build_qemu_args 完整命令行 + /proc 资源监控
  - ✅ backend-dev-2: WindowsRuntime (WSL2) — WSL2 distro 管理 + socket 转发
  - ✅ integrator: Tauri 集成 — AppState + runtime_status + docker.rs + health monitor
  - ✅ architect: 20/20 零偏差验收通过
  - ✅ tester: 121 cargo tests + 50 vitest tests 全部通过
- [x] Step 6: MCP (Server + Client) — 2026-03-20
  - ✅ mcp-dev: cratebay-mcp 独立二进制 — 11 个 sandbox 工具 + 4 模板 + 路径校验 + 审计日志 + JSON-RPC stdio
  - ✅ backend-dev: cratebay-core MCP Client — stdio/SSE 双传输 + .mcp.json 解析 + McpManager + 重试策略
  - ✅ integrator: Tauri MCP Commands — 5 个 stub 替换 + 3 个新命令 (add/remove/export) + AppState.mcp_manager
  - ✅ frontend-dev: MCP Tool Bridge — mcpTools.ts + useMcpToolSync.ts + mcpStore 修正
  - ✅ backend-dev: McpToolInfo 增加 server_name + serde camelCase
  - ✅ architect: 28/30 验收通过，2 个低严重度偏差已修复 spec (pi-agent-core API 适配)
  - ✅ tester: 253+ cargo tests + 67 vitest tests 全部通过
- [x] Step 7: 测试 + CI/CD + 打磨 — 2026-03-20
  - ✅ frontend-dev: 前端组件/Store/Hook 测试补充 (197 Vitest tests, 2 skipped + coverage 配置)
  - ✅ frontend-dev: E2E 测试 (68 个 Playwright 用例: navigation 9 + chat-flow 12 + settings 12 + containers 16 + mcp-servers 18 + example 1, 5 个 Page Object Models)
  - ✅ frontend-dev: Agent 测试 (Mock LLM/Tauri + golden file + canary tests)
  - ✅ backend-dev: 性能基准测试 (Criterion: startup_bench 5 个 + storage_bench 4 个)
  - ✅ tester: CI/CD Pipeline (ci.yml 4 阶段 + 三平台 matrix + coverage + canary)
  - ✅ tester: release.yml (v* tag 触发 + code signing + SHA256) + pages.yml (website 自动部署)
  - ✅ tester: 安全测试 (路径穿越 8 个 + JSON-RPC 注入 9 个 + SQL 注入 4 个 + API Key 泄露 3 个 = 24 个)
  - ✅ doc-keeper: Spec 文档一致性修复 (api-spec 1.2.0, backend-spec 1.2.0, architecture 1.1.0)
  - ✅ doc-keeper: CHANGELOG.md v2.0.0-alpha.1 + progress.md 最终更新
  - ✅ 最终测试结果: cargo test 269 passed / 5 ignored, vitest 197 passed / 2 skipped, Playwright 68 E2E

### Step 3 验证结果
```
cargo check --workspace     → ✅ 零错误 (3 个 dead_code 警告)
cargo test --workspace      → ✅ 85 passed, 0 failed, 5 ignored (Docker)
pnpm run test               → ✅ 4 passed (Vitest)
```

### Step 3 产出统计
- 30 个 Tauri Commands: Container(9) + LLM(9) + Storage(6) + MCP(5 stub) + System(3)
- LLM Proxy: Anthropic + OpenAI Responses + OpenAI Completions, 双 header, SSE 流式
- SQLite: 12 张表, WAL 模式, 迁移系统, Provider/Model/Conversation/Message CRUD
- 5 份 spec 文档更新至 v1.1.0 (AI 需求细化: 3 种 API 格式 + Provider 设置 + 模型勾选 + reasoning effort)

### Step 2 已知偏差
- AGENTS.md 写的 "Vite 7.x" 实际不存在，使用 Vite 6.4.1（稳定版）
- tauri-specta/specta 使用 rc 版本 (2.0.0-rc.21/rc.22)

## 已完成（Quick Resume 验证轮）✅ — 2026-03-21
- [x] Runtime 自动启动后端集成
  - ✅ AppState.docker 改为 `Arc<Mutex<Option<Arc<Docker>>>>` 支持动态更新
  - ✅ main.rs 后台线程自动启动 runtime (detect→provision→start→wait for Docker→fallback)
  - ✅ system.rs 新增 runtime_start, runtime_stop 命令 + docker_status source 检测
  - ✅ container.rs 所有命令适配新的 require_docker() 返回 Arc<Docker>
  - ✅ App.tsx 监听 docker:connected 事件刷新前端状态
  - ✅ Settings RuntimeTab 添加 Start/Stop 按钮 + runtimeLoading 状态
  - ✅ 编译通过：TypeScript 0 errors, Rust 0 errors (2 dead_code warnings)
- [x] 任务 A: GUI 验证 — 通过
  - ✅ pnpm build 成功 (Vite 6.4.1)
  - ✅ cargo build -p cratebay-gui 成功 (27s, 2 dead_code warnings: CONTAINER_STATUS_CHANGE, MCP_CONNECTION_CHANGE)
  - ✅ pnpm tauri dev 启动正常：runtime auto-start → 骨架失败 → fallback 外部 Docker → 前端正常渲染
  - ✅ Settings Runtime tab: Start/Stop 按钮、CPU/Memory 滑块均存在
  - ⚠️ CPU/Memory 滑块值仅保存在本地 state，未持久化到后端
- [x] 任务 B: 文档一致性审查 — 发现 7 个偏差
  - ⚠️ [高] runtime_start/runtime_stop 未在 api-spec.md 中定义 → 需更新 spec
  - ⚠️ [高] ImagesPage 代码存在但 frontend-spec 未定义 → 需人工决定保留或删除
  - ⚠️ [高] Settings 6 Tab vs spec 3 Tab → 需更新 spec
  - ⚠️ [高] AppState.docker 类型 Arc<Mutex<Option<Arc<Docker>>>> vs spec Option<Arc<Docker>> → 需更新 spec
  - ⚠️ [中] appStore 多出 runtimeLoading 字段 → 需更新 spec
  - ⚠️ [中] backend-spec 未列 runtime_start/stop 命令 → 需更新 spec
  - ✅ [低] provision() 回调 Box vs impl (async_trait 对象安全) → 可接受
- [x] 任务 C: 自研 Runtime 移植方案
  - ✅ master runtime 相关代码约 9857 行，v2 骨架 3017 行但核心逻辑缺失
  - ✅ 完全缺失：Hypervisor 实现 (macOS 1749/Linux 1941/Windows 1626 行) + OS 镜像系统 (738 行)
  - ✅ 推荐混合架构：保留 v2 RuntimeManager trait，内部委托 master 逻辑
  - ✅ 移植分 3 Phase，~10 文件，~5200 行，优先级 macOS → Linux → Windows

## 进行中 🔄

### Runtime 架构重构 + 端到端验证（2026-03-26）

**Phase 1: 移除外部 Docker 支持 ✅**
- ✅ 删除 Podman 引擎模块 (`engine/podman.rs`)
- ✅ 删除 Colima/OrbStack/Docker Desktop 检测逻辑
- ✅ 前端删除 CrateBay/External Docker 双标签页、`allowExternalDocker` toggle
- ✅ 简化为纯内置 runtime 单一代码路径

**Phase 2: 规范化 RuntimeManager trait ✅**
- ✅ 状态机从 8 个变体简化为 7 个（移除 `Provisioning`）
- ✅ `detect()` → `get_state()` 语义更清晰
- ✅ 三平台实现（macOS/Linux/Windows）适配完成

**Phase 3: 统一 GUI/CLI 启动流程 ✅**
- ✅ 新增 CLI `cratebay runtime start/stop/status/provision` 命令
- ✅ GUI/CLI 复用 `engine::ensure_docker()` 统一路径

**Phase 4: VM 网络修复 ✅**
- ✅ DNS 注入诊断日志全链路覆盖（macos.rs, common.rs, guest-agent, vz runner）
- ✅ DNS failsafe：空列表时强制注入 `1.1.1.1,8.8.8.8`
- ✅ aarch64 runtime 镜像构建（Alpine kernel + initramfs + Docker + guest-agent）
- ✅ 构建脚本支持 `CRATEBAY_ALPINE_MIRROR` 国内镜像加速
- ✅ vsock 内核模块缺失 → 默认改为 TCP 转发模式
- ✅ Docker 等待超时 45s → 120s
- ✅ VZ NAT 出口不通 → VZ runner 内置 HTTP CONNECT 代理（自动绑定 0.0.0.0:3128）
- ✅ GUI 重启不再杀容器 → `start()` 先 adopt 已运行的 VZ runner 进程

**Phase 5: 容器/镜像管理完善 ✅**
- ✅ 镜像搜索（Docker Engine API + Docker Hub HTTP API 双路径）
- ✅ Mirror 拉取后自动 re-tag + 清理 mirror tag
- ✅ `PullProgressCallback` 从 `Box` 改为 `Arc`，mirror 阶段也有进度回调
- ✅ 全局拉取任务列表（pullStore + PullTaskList 组件）
- ✅ 多镜像并行拉取，跨 tab 进度可见
- ✅ 拉取完成自动刷新本地镜像列表

**GUI 修复 ✅**
- ✅ 创建容器对话框宽度溢出 → `sm:max-w-lg`
- ✅ 容器卡片 CPU/MEM 字体统一为 `text-xs`
- ✅ 容器卡片底部按钮栏对齐（左右边距与内容一致）
- ✅ 主题下拉"system"→"跟随系统"中文显示
- ✅ 设置 Tab 增加下边距
- ✅ 运行时设置简化（删除桥接等高级选项）
- ✅ 搜索页删除多余的全局"拉取"按钮
- ✅ 镜像删除改为 `force: true` + 刷新列表

**端到端验证结果 ✅**
- ✅ `cratebay runtime start` — VM 启动 + Docker 就绪（9 秒）
- ✅ `cratebay image pull alpine:3.20` — 通过内置 HTTP 代理拉取成功
- ✅ `cratebay container create/exec/stop/delete` — 全流程成功
- ✅ GUI 打包安装（CrateBay.app）— 容器管理、镜像搜索/拉取均可用
- ✅ `cargo test` 344 passed / `pnpm test` 217 passed / `pnpm typecheck` 零错误

### GUI + Docker 集成优化（2026-03-22）

**已完成的修复：**
- ✅ Docker 镜像加速源：内置中国镜像源 + Settings 页自定义管理（添加/删除/恢复默认）
- ✅ image_pull 非阻塞：后端 `tokio::spawn` 后台拉取 + 前端 Tauri Event 监听完成
- ✅ 拉取超时保护：60s 整体超时 + 15s per-chunk 超时（tokio::time::timeout）
- ✅ 实时进度显示：容器卡片 placeholder 动态更新拉取百分比
- ✅ fetchContainers 不再卡死：AbortController 替代 loading guard + 8s 超时
- ✅ initRuntimeStatus 超时保护：5s Promise.race 超时
- ✅ 语言下拉修复：SelectValue 显示"简体中文"/"English"而非 value
- ✅ auto_start 优雅回退：容器创建成功但启动失败时返回容器信息，显示 warning 而非 error
- ✅ health_check 状态回退修复：后端+前端双重保护，防止 Docker ping 偶尔超时导致"运行中"→"启动中"
- ✅ 状态栏稳定性：health monitor 复用 AppState Docker client + 重试/阈值；`runtime_status` reconcile；前端 90s 降级宽限

### GUI 可用性修复（2026-03-23）

**用户阻塞问题修复：**
- ✅ Images 搜索不可用：新增 Settings > Runtime 的 HTTP Proxy 配置 + 重启提示；`runtime_start` 与 runtime auto-start 都会读取持久化设置并应用 `CRATEBAY_RUNTIME_HTTP_PROXY*` 环境变量
- ✅ 容器日志/终端为 mock：ContainerDetail 改为真实 `container_logs` + `container_exec_stream`（可输入命令并实时输出）
- ✅ 容器 CPU/MEM 展示语义纠正：列表卡片展示规格（CPU cores / Memory MB）；详情面板展示监控（`container_stats`，按分配规格计算占用百分比）
- ✅ 语言下拉显示修复：SelectValue 支持展示 label（简体中文/English）而非 raw value（zh-CN/en）
- ✅ LLM Providers / Chat 模型选择可用：修复 invoke 参数命名（snake_case），Providers/Models 在启动时加载；默认 provider/model 通过 settings key 持久化（`default_provider`/`default_model`）
- ✅ Agent 内置镜像工具：补齐 `image_*` AgentTools 并注册到工具表

**回归验证：**
- ✅ `cargo test --workspace`
- ✅ `pnpm -C crates/cratebay-gui typecheck`
- ✅ `pnpm -C crates/cratebay-gui test`
- ✅ `pnpm -C crates/cratebay-gui test:e2e`
- ✅ `pnpm -C crates/cratebay-gui build`
- ✅ macOS runtime HTTP 代理支持：`CRATEBAY_RUNTIME_HTTP_PROXY` + 可选 `CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE=1`
- ✅ Images API 补齐：image_list/search/inspect/remove/tag/pull + GUI commands 注册
- ✅ E2E 稳定性：统一浏览器模式 Tauri mock（`__MOCK_TAURI_INVOKE__` + `listen()` fallback）+ 补全 data-testid
- ✅ TypeScript typecheck：容器状态 badge 映射全覆盖 + i18n 增加 overview/ports
- ✅ 全量验证通过：`cargo test --workspace` / `pnpm test` / `pnpm test:e2e` / `pnpm typecheck` / `pnpm build`

**待处理的问题：**
1. ⚠️ **容器端到端真实运行仍需环境验证** — 若 VM 无法直连拉取，可配置 `CRATEBAY_RUNTIME_HTTP_PROXY`（必要时启用 `CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE=1`）或使用可用的 registry mirrors；需要一个包含 `/bin/sh` 的有效镜像来验证 create→start→exec→stop→delete 全流程

**关键代码变更文件：**
- `crates/cratebay-core/src/container.rs` — PullProgress/PullProgressCallback, image_pull 超时+mirrors+callback, auto_start 优雅回退
- `crates/cratebay-gui/src-tauri/src/commands/container.rs` — image_pull 非阻塞（tokio::spawn + event）
- `crates/cratebay-gui/src-tauri/src/events.rs` — ImagePullProgress struct + event 命名
- `crates/cratebay-gui/src/App.tsx` — initRuntimeStatus 超时, runtime:health 防降级, docker:connected 处理
- `crates/cratebay-gui/src/stores/containerStore.ts` — AbortController, createContainer 重写（pull event + placeholder 动态更新）
- `crates/cratebay-gui/src/pages/SettingsPage.tsx` — 语言修复 + RegistryMirrorsSection
- `crates/cratebay-gui/src/types/settings.ts` — registryMirrors + DEFAULT_REGISTRY_MIRRORS
- `crates/cratebay-core/src/runtime/macos.rs` — health_check 防降级（prev state = Ready → 保持 Ready）

### Engine Ensure + CLI/GUI（2026-03-24）

- ✅ `cratebay-core`: 新增 `engine::ensure_docker()` + `engine.lock` 跨进程互斥，统一“外部 Docker 优先 / 内置 runtime 自动启动”
- ✅ `cratebay-core`: Windows runtime Docker 连接改为 `tcp://127.0.0.1:<CRATEBAY_WSL_DOCKER_PORT>`（WSL localhost forwarding），避免 named pipe 连接失败
- ✅ `cratebay-core`: `DOCKER_HOST` 支持 unix/tcp/http/https/npipe；探测 ping 超时 5s（成功后返回 120s client），避免无效 DOCKER_HOST 卡住启动
- ✅ `cratebay-core`: Linux runtime helper（QEMU）资产发现改为 `runtime-linux/`（与 `scripts/build-runtime-assets-linux.sh` 产物一致）
- ✅ `cratebay-core`: WindowsRuntime health_check 对缓存端点失效自动回退到 localhost，避免“容器可用但 GUI 显示未连接”
- ✅ `cratebay-gui`(Tauri bundle): 资源打包加入 `runtime-linux/**/*` 与 `runtime-wsl/**/*`，为 Win/Linux “安装即用”准备完整 runtime 资产目录
- ✅ `cratebay-gui`(Tauri 后端): `AppState.ensure_docker()`，所有 container/image 命令自动 ensure；GUI 启动 auto-start 复用 core engine ensure
- ✅ `cratebay-cli`: 补齐 `container/*`、`image/*`、`system docker-status` 子命令；`container create` 缺镜像时自动 pull；支持 `image search`
- ✅ 回归验证：`cargo test --workspace`

### 状态栏抖动 + 并发阻塞修复（2026-03-25）

**问题 1：状态栏从"引擎就绪"间歇性回退"启动中"**

根因：health_check() 每轮新建 Docker 连接（非复用共享 client），瞬时 socket 抖动被误判为 runtime 不可用。

修复：
- ✅ **三平台降级阈值上调 2→3**：`READY_DOWNGRADE_FAILURE_THRESHOLD` 更新至 3（macos.rs:67 / linux.rs:65 / windows.rs:52），容忍更多瞬时 ping 失败
- ✅ **health monitor 改"共享连接优先"**：`main.rs` 中 `start_runtime_health_monitor()` 重构；共享 client 可用时直接广播 Ready，跳过 health_check() 新建连接；共享 client 不可用才走 health_check() 慢路径
- ✅ **health monitor 周期 30s→20s**：更快感知真实故障
- ✅ **重试策略增强**：`get_responsive_shared_docker()` 重试次数 3→5，间隔 300ms→200ms

**问题 2：容器操作、镜像操作、状态刷新互相阻塞**

根因：所有命令都调 `ensure_docker()` → 内部 `engine::ensure_docker()` 有跨进程文件锁，默认等待 10 分钟；并发命令在锁上排队；同时每次命令都 ping Docker 导致 ~5s 延迟。

修复：
- ✅ **AppState 新增 `docker_init: Arc<OnceCell<...>>`**：`state.rs` 增加字段，确保同进程内 Docker 初始化只执行一次
- ✅ **新增 `ensure_docker_once()` 方法**：`state.rs` 新方法；已有 client 时零延迟直接返回（无 ping）；无 client 时单例化初始化，并发调用等待同一 future；锁等待超时从 10 分钟缩短到 60 秒
- ✅ **container.rs 所有命令改用 `ensure_docker_once()`**：15 处 `ensure_docker()` 全部替换，消除并发排队
- ✅ **main.rs 初始化补入 `docker_init`**

**回归验证：**
- ✅ `cargo test --workspace`：332 passed / 5 ignored（0 failed）
- ✅ `pnpm typecheck`：0 错误
- ✅ `pnpm test`：216 passed / 2 skipped（0 failed），18 个测试文件
- ✅ `pnpm build`：构建成功；顺带修复 `tw-animate-css` 依赖缺失问题

**关键代码变更文件：**
- `crates/cratebay-core/src/runtime/macos.rs:67` — READY_DOWNGRADE_FAILURE_THRESHOLD 2→3
- `crates/cratebay-core/src/runtime/linux.rs:65` — READY_DOWNGRADE_FAILURE_THRESHOLD 2→3
- `crates/cratebay-core/src/runtime/windows.rs:52` — READY_DOWNGRADE_FAILURE_THRESHOLD 2→3
- `crates/cratebay-gui/src-tauri/src/main.rs` — health monitor 共享连接优先 + 周期 20s + 重试增强
- `crates/cratebay-gui/src-tauri/src/state.rs` — docker_init OnceCell + ensure_docker_once()
- `crates/cratebay-gui/src-tauri/src/commands/container.rs` — 15 处改用 ensure_docker_once()

### Spec 对齐 + 提交钩子修复（2026-03-25）

- ✅ `frontend-spec.md` 补齐 Settings 6 Tab、`appStore` 新字段（`builtinRuntimeReady` / `dockerSource`）、`allowExternalDocker`
- ✅ `backend-spec.md` 补齐 `AppState.docker_source`、`docker_init`、`ensure_docker_once()`、`set_docker_source()`
- ✅ `api-spec.md` 补齐 `allowExternalDocker` 设置键说明
- ✅ `.githooks/pre-commit` 将 `cargo test -p cratebay-cli --lib` 修正为 `cargo test -p cratebay-cli --bins`

### GUI 打磨（2026-03-28）✅

**容器页面**
- ✅ 容器详情面板改用 React Portal，不再推走容器列表
- ✅ 操作按钮移到面板 header 标题栏（图标按钮：停止/启动/删除）
- ✅ 日志区域删除，终端改为展示 `docker exec -it <id> /bin/sh` 登录指令
- ✅ 规格/监控去掉独立边框，改为普通文本行
- ✅ 容器列表（table 模式）改为 OrbStack 风格行卡片
  - 左侧状态圆点（运行中绿色发光）
  - 名称 + ID + 镜像 + 端口紧凑排列
  - 状态标签/CPU/内存响应式显示
  - 操作按钮常显（停止/启动/删除）
  - 行之间 border 分隔线

**镜像页面**
- ✅ 搜索框和操作按钮提升到顶部统一工具栏
- ✅ 根据 Tab 切换右侧显示不同控件（本地：筛选+刷新+批删 / 搜索：搜索框+搜索）
- ✅ 拉取任务下拉框从 `left-0` 改为 `right-0`，不再溢出右边界
- ✅ 镜像时间格式改为中文

**MCP 页面**
- ✅ 卡片网格最小宽度 380px → 300px
- ✅ `toolCount ?? 0` 防止显示 NaN
- ✅ 命令行展示加 `filter(Boolean)` 防 undefined 显示

**统一头部风格**
- ✅ 三个页面（容器/镜像/MCP）头部统一为单层工具栏
- ✅ 容器页删除重复的统计行
- ✅ Sidebar 对话时间改为中文（刚刚/X分钟前/X小时前/X天前）

**Bug 修复**
- ✅ `image_pull` 显式设置 `tag` 字段，防止拉取仓库所有 tag（redis 拉出几十个版本的问题）
- ✅ mirror re-tag 后删除 mirror tag 改用 `force=true`，防止重复镜像
- ✅ MCP `NaN 个工具可用` 问题修复

**验证结果**
- ✅ `cargo test --workspace`: 205 passed
- ✅ `pnpm typecheck`: 0 errors
- ✅ `pnpm test`: 216 passed / 2 skipped
- ✅ 打包安装 `/Applications/CrateBay.app` 验证通过

## 待开始 📋 — v2.1-Alpha 发布准备

**产品定位已调整**：从"Chat-First 控制面板"改为"本地 AI Sandbox"

### v2.1-Alpha 优先项（用户可用版，18-22 天）

🔴 **P0 必做**：
1. **MCP Server 工具扩展** — `sandbox_run_code`, `sandbox_install`, 文件 API 增强 — 5-7 天
2. **离线镜像打包与自动导入** — Python/Node/Rust/Ubuntu 镜像预装 — 3-4 天
3. **QA 与跨平台验证** — e2e 测试、性能验收、发布准备 — 4-5 天

🟡 **P1 应做**：
4. **Desktop App 优化** — Sandbox 专属 Tab + Dashboard — 3 天
5. **文档与示例** — Getting Started, Examples, Troubleshooting — 2-3 天

详见 **ROADMAP.md** （新建）

### 后续不做（v2.1+ 或不做）
- ~~Apple Developer ID 签名~~（用户不关键，当前 HTTP 代理可用）
- ~~vsock 内核模块~~（性能优化，非 MVP 必须）
- ~~GUI 端口映射 UI~~（用户很少需要）

## 阻塞/问题 ⚠️
- **VZ NAT 出口限制**: macOS VZ.framework 的 NAT 需要 Apple Developer ID 签名才能获取 `com.apple.vm.networking` entitlement。当前通过内置 HTTP CONNECT 代理桥接解决，正式发布需要 Developer ID 签名。

## 文档完成明细

16 份文档（以文件头 Version 为准）：

| 文档 | 版本 | 路径 |
|------|------|------|
| architecture.md | **1.1.2** | docs/specs/architecture.md |
| backend-spec.md | **1.3.4** | docs/specs/backend-spec.md |
| runtime-spec.md | **1.2.5** | docs/specs/runtime-spec.md |
| api-spec.md | **1.5.4** | docs/specs/api-spec.md |
| database-spec.md | **1.1.0** | docs/specs/database-spec.md |
| frontend-spec.md | **1.2.6** | docs/specs/frontend-spec.md |
| agent-spec.md | **1.2.2** | docs/specs/agent-spec.md |
| mcp-spec.md | **1.1.0** | docs/specs/mcp-spec.md |
| testing-spec.md | **1.1.0** | docs/specs/testing-spec.md |
| dev-workflow.md | 1.0.0 | docs/workflow/dev-workflow.md |
| agent-team-workflow.md | 1.0.0 | docs/workflow/agent-team-workflow.md |
| knowledge-base.md | 1.0.0 | docs/workflow/knowledge-base.md |
| tech-decisions.md | **1.1.1** | docs/references/tech-decisions.md |
| glossary.md | **1.0.1** | docs/references/glossary.md |
| docs/README.md | **1.0.1** | docs/README.md |
| progress.md | — | docs/progress.md (this file) |

---

## 用户永久规则（MUST FOLLOW — 跨会话/跨机器生效）

> **所有 AI Agent 必须遵守以下规则，无需用户每次重复要求。**

### 规则 1: 固定开发团队
每次新会话启动时，**必须**按照 `docs/workflow/agent-team-workflow.md` §1.1 创建固定开发团队：
- 当前阶段：开发阶段（Phase 2）
- 团队组成（6 人）：team-lead、frontend-dev、backend-dev、ai-engineer、runtime-dev、tester
- 团队名称：`cratebay-dev`
- 团队是项目全生命周期的，**不要在一轮任务完成后关闭**
- 只有用户明确要求或项目阶段切换时才调整团队

### 规则 2: 按 spec 执行
所有开发工作严格遵循 `docs/specs/` 下的 spec 文档，spec 是唯一的真理来源。

### 规则 3: Runtime 策略固定
- built-in runtime 是产品主线
- Podman 只作为 fallback / escape hatch
- runtime、container、image 相关问题，优先修 built-in runtime 主链路
- 非人工明确批准，不新增 Podman-only 产品能力或把 Podman 提升为默认路径

---

## 下次继续（Quick Resume）

> **给 AI 的可执行指令** — 新会话启动后读取此段，按步骤执行。

### 当前阶段：ChatPage v1.0 开发（2026-04-01）

**版本**: v0.9.0 → v1.0.0 | **分支**: feat/chatpage-v1 | **定位**: 开源容器管理 + AI Sandbox

### ChatPage v1.0 已完成（本次会话 2026-03-30）

**Phase A: 核心能力** ✅
- ✅ 提取 sandbox 逻辑到 cratebay-core（run_code/install_packages/templates）
- ✅ 新增 Tauri 命令：sandbox_run_code、sandbox_install
- ✅ 新增 Sandbox Agent Tools（sandboxTools.ts）
- ✅ 新增 sandboxStore（会话→沙盒绑定）
- ✅ 重写 System Prompt（代码执行助手定位）
- ✅ 升级 pi-agent-core + pi-ai → 0.63.2
- ✅ MCP 层改为调用 core 共享逻辑

**Phase B: 供应商配置** ✅
- ✅ settingsStore 增强（requestFormat、reasoning level 6 档）
- ✅ Provider URL 自动推断 API 格式（completions/responses/anthropic）
- ✅ ProviderForm 配置 UI 改进
- ✅ ChatInput 模型选择器按 Provider 分组
- ✅ 后端零改动（现有 llm 命令完全够用）

**Phase C: UI 增强** ✅
- ✅ ToolCallItem 组件（状态圆点、Bash 命令内联、参数截断、折叠展开）
- ✅ ThinkingBlock 组件（可折叠思考过程、流式自动打开）
- ✅ SandboxBar 组件（沙盒状态横条）
- ✅ MessageBubble 集成新组件（替换旧的 AgentThinking/ToolCallCard）
- ✅ 欢迎页改造（代码执行场景建议卡片）
- ✅ i18n 更新（en + zh-CN）

**Phase D: 测试 + UI 打磨** ✅
- ✅ cargo test --workspace: 348 passed, 0 failed
- ✅ pnpm build: 0 errors
- ✅ cargo check --workspace: clean
- ✅ MCP 页面合并到 Settings Tab（导航 5→4 页）
- ✅ 产品定位更新（开源容器管理 + AI Sandbox）
- ✅ APP_VERSION 修复为 0.9.0
- ✅ 系统镜像保护（不可删除、不可勾选、System 标签）
- ✅ 镜像页分区（沙盒镜像 / 用户镜像）+ i18n
- ✅ Sidebar Logo 间距优化

### v1.0.0 剩余工作

1. **合并到 master** — feat/chatpage-v1 → master
2. **跨平台验证** — Linux/Windows 测试
3. **版本号更新** — 0.9.0 → 1.0.0
4. **系统镜像打包** — bundle-images tar.gz 打入发布包

### 可执行步骤

```
1. 读取 AGENTS.md（产品方向 + 技术栈）
2. 读取本文件（当前进度）
3. 读取 docs/chatpage-v1-plan.md（ChatPage 开发方案）
4. 确认 runtime 可用：cratebay runtime status
5. 合并分支或继续开发
```

### Runtime 移植方案概要（来自 runtime-dev 分析）

```
架构选择：混合架构
  - 保留 v2 的 RuntimeManager trait（async, 进度回调, 健康检查）
  - 不移植 master 的 Hypervisor trait（避免两层抽象）
  - 将 master 的实现逻辑直接嵌入 RuntimeManager impl

master runtime 代码: ~9857 行
v2 骨架代码: ~3017 行（核心逻辑全缺失）
移植新增: ~5200 行，~10 文件

缺失模块:
  - Hypervisor 实现: macOS 1749 + Linux 1941 + Windows 1626 行
  - OS 镜像目录系统: 738 行
  - 文件工具: 39 行
  - runtime.rs 协调层: 2839 行（全局配置+资产查找+镜像安装+VM 生命周期+Docker ping）

关键风险:
  - macOS VZ.framework 通过外部 runner 进程（Swift）桥接，需确认是否继续使用
  - master 是同步代码，v2 是 async trait，需 spawn_blocking 包装
  - VM 状态存储: master 用 JSON 文件，v2 用 SQLite，需决定方案
```

### 文档偏差修复记录（2026-03-25）

| # | 严重性 | 描述 | 当前状态 |
|---|--------|------|----------|
| 1 | 高 | runtime_start/runtime_stop 未在 api-spec.md 中定义 | ✅ 已修复 |
| 2 | 高 | ImagesPage 代码存在但 frontend-spec 未定义 | ✅ 已修复 |
| 3 | 高 | Settings 6 Tab vs spec 3 Tab | ✅ 已修复 |
| 4 | 高 | AppState.docker 类型不匹配 spec | ✅ 已修复 |
| 5 | 中 | appStore 新字段未写入 spec | ✅ 已修复 |
| 6 | 中 | backend-spec 未列 runtime_start/stop 与 ensure 相关变更 | ✅ 已修复 |
| 7 | 低 | provision() 回调 Box vs impl | 可接受 |

### 历史偏差修复记录

> Step 3/4 偏差已在 Step 4 修复轮中全部解决（step4-fix 团队）。
> Step 5 架构师验收 20/20 零偏差。
> Step 6 架构师验收 28/30，2 个低严重度 spec 偏差已修复 (mcp-spec.md v1.1.0)。
> Step 7 所有测试通过，CI/CD 管线就绪，文档一致性验证完毕。
