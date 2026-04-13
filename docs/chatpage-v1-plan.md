# ChatPage v1.0 开发方案

> CrateBay v0.9.0 → v1.0.0：让 ChatPage 成为"内置沙盒能力的 AI 聊天"

## 一、定位

CrateBay ChatPage = **AI 聊天 + 开箱即用的代码执行沙盒**

和 Claude Desktop 的核心区别：

| | Claude Desktop + MCP | CrateBay ChatPage |
|---|---|---|
| 代码执行 | 需要手动配 MCP | **开箱即用**，内置沙盒 |
| LLM | 只能用 Claude（订阅制） | **接自己的 API Key**，支持多供应商 |
| 成本 | Claude 订阅费 | **按 API 用量付费**（或接本地模型免费） |
| 隐私 | 代码上传到云端 | **代码在本地 VM 执行** |
| 工具 | 通用 | **沙盒专用**（run_code 一步到位） |

**一句话**：CrateBay ChatPage 是一个自带代码执行能力的 AI 聊天界面，用户不需要理解 Docker/MCP/容器，配个 API Key 就能让 AI 帮你跑代码。

## 二、从 LiveAgent 复用

### 2.1 供应商配置系统（95% 复用）

**来源**: LiveAgent `crates/agent-gui/src/lib/settings/index.ts`

复用内容：
- `CustomProvider` 类型（id/name/type/baseUrl/apiKey/models/activeModels/requestFormat/reasoning）
- `normalizeCustomProvider()` — URL 后缀自动推断 API 格式（completions vs responses）
- `/v1/models` 动态获取模型列表
- `createModelFromConfig()` — 创建 pi-ai Model 实例
- 双认证头（Bearer + x-api-key）

改动点：
- ProviderId 扩展为更通用类型（不限于 codex/claude_code）
- UI 用 CrateBay 的 shadcn/ui 重写
- API Key 存储保留 Rust SQLite 后端（比 localStorage 更安全）

### 2.2 Chat UI 组件

**来源**: LiveAgent `crates/agent-gui/src/pages/ChatPage.tsx`

复用内容：
- **Round 分组**（行 676-750）：多轮工具调用分组展示
- **ToolCallItem**（行 540-674）：状态圆点 + 参数截断 + 折叠展开
- **ThinkingBlock**（行 490-537）：可折叠思考过程

### 2.3 Agent 增强

**来源**: LiveAgent `crates/agent-gui/src/lib/chat/agentRunner.ts`

复用内容：
- **Bash 并行执行**（行 264-301）：连续 Bash 调用最多 4 个并行
- **工具摘要生成**（行 26-96）：为工具调用生成简洁描述

不复用：
- Agent 循环本身（继续用 pi-agent-core）
- @Mention、Skills 系统

## 三、从 pi-mono 升级

升级 pi-agent-core + pi-ai 到 0.63.2：
- 多编辑支持（单个工具调用修改多个位置）
- Bug 修复 + 更好的取消操作
- 新模型支持（Gemini 3.1 Pro）

继续只用 pi-agent-core + pi-ai，不引入其他包。

## 四、新增开发

### 4.1 Tauri Sandbox 命令

新增 `commands/sandbox.rs`，复用 cratebay-mcp 的 sandbox 逻辑：

```rust
#[tauri::command]
pub async fn sandbox_run_code(
    state: State<'_, AppState>,
    language: String,
    code: String,
    sandbox_id: Option<String>,
    timeout_seconds: Option<u64>,
) -> Result<RunCodeResult, AppError>

#[tauri::command]
pub async fn sandbox_install(
    state: State<'_, AppState>,
    sandbox_id: String,
    package_manager: String,
    packages: Vec<String>,
) -> Result<InstallResult, AppError>
```

提取 cratebay-mcp/src/sandbox.rs 的 run_code/install_packages 到 cratebay-core 共享。

### 4.2 Sandbox Agent Tools

新增 `tools/sandboxTools.ts`：

```typescript
sandbox_run_code  — 一键执行代码（自动创建沙盒）
sandbox_install   — 安装依赖包
```

注册到 Agent 工具集，和现有的 container/file/shell 工具并存。System prompt 引导 AI **优先使用 sandbox 工具**。

### 4.3 System Prompt 重写

从"容器管理助手"改为"代码执行助手"：

```
你是 CrateBay AI 助手，擅长在安全沙盒中执行代码和解决编程问题。

核心工具：
- sandbox_run_code — 执行 Python/JavaScript/Bash/Rust 代码
- sandbox_install — 安装依赖（pip/npm/cargo/apt）
- file_read/file_write — 读写沙盒内文件

行为规则：
- 用户要求运行代码时，直接调用 sandbox_run_code
- 需要安装包时，先调用 sandbox_install
- 执行失败时分析错误并建议修复
- 保持简洁，代码结果直接展示

当前沙盒状态：{动态注入}
```

### 4.4 会话绑定沙盒

新增 `stores/sandboxStore.ts`：
- 每个会话可绑定一个持久沙盒（cleanup=false）
- 后续操作复用同一沙盒（保留变量、已装包）
- 会话删除时自动清理沙盒

### 4.5 欢迎页改造

首次进入 ChatPage 显示引导：

```
欢迎使用 CrateBay

[快速开始]
- "帮我写一个 Python 脚本分析 CSV 数据"
- "用 Node.js 创建一个 HTTP 服务器"
- "运行 Rust 计算斐波那契数列"

[配置提示]
需要先在 Settings 中配置 LLM 供应商（OpenAI / Anthropic / 自定义）
```

## 五、执行计划

### Phase A: 核心能力（1 周）

| 任务 | 来源 | 天数 |
|------|------|------|
| 提取 sandbox 逻辑到 cratebay-core | 重构 | 1 |
| 新增 Tauri sandbox 命令 | 新代码 | 0.5 |
| 新增 sandboxTools Agent Tools | 新代码 | 0.5 |
| 新增 sandboxStore + 会话绑定 | 新代码 | 0.5 |
| 重写 System Prompt | 新代码 | 0.5 |
| 升级 pi-agent-core + pi-ai → 0.63.2 | 升级 | 0.5 |
| ChatPage 集成 sandbox 工具 | 修改 | 0.5 |

### Phase B: 供应商配置（1 周）

| 任务 | 来源 | 天数 |
|------|------|------|
| 迁移 CustomProvider + normalize | LiveAgent | 0.5 |
| 迁移 createModelFromConfig | LiveAgent | 0.5 |
| 迁移 /v1/models 动态获取 | LiveAgent | 0.5 |
| Settings 页面 Provider UI 重构 | shadcn/ui | 2 |
| ChatPage 模型选择器 | 新代码 | 0.5 |
| API Key 后端存储适配 | 修改 | 0.5 |
| 测试 | 新代码 | 0.5 |

### Phase C: UI 增强（1 周）

| 任务 | 来源 | 天数 |
|------|------|------|
| Round 分组逻辑 | LiveAgent 参考 | 1 |
| ToolCallItem 组件 | LiveAgent 参考 | 1 |
| ThinkingBlock 组件 | LiveAgent 参考 | 0.5 |
| 沙盒状态栏 | 新代码 | 0.5 |
| 欢迎页改造 | 新代码 | 0.5 |
| Bash 并行执行 | LiveAgent 参考 | 0.5 |

### Phase D: 收尾（0.5 周）

| 任务 | 天数 |
|------|------|
| 全量测试（cargo test + pnpm test） | 1 |
| 文档 + Spec 同步 | 1 |

**总计：3.5 周**

## 六、验收标准

```
场景 1：首次使用
  打开 CrateBay → Settings 配置 OpenAI API Key
  → ChatPage 自动加载模型列表
  → 告诉 AI "用 Python 计算 1 到 100 的和"
  → AI 调用 sandbox_run_code → 返回 5050
  
场景 2：多轮开发
  → "安装 pandas 和 matplotlib"
  → AI 调用 sandbox_install
  → "生成随机数据并画个图表"
  → AI 写代码 → sandbox_run_code 执行 → 返回结果
  → 整个过程复用同一个沙盒

场景 3：多供应商
  → Settings 添加 Anthropic 供应商
  → 动态获取模型列表
  → ChatPage 切换到 Claude Sonnet
  → 继续对话，沙盒状态保持
```

## 七、文件变更清单

### 新增
- `crates/cratebay-core/src/sandbox.rs` — 共享 sandbox 逻辑（从 mcp 提取）
- `crates/cratebay-gui/src-tauri/src/commands/sandbox.rs` — Tauri sandbox 命令
- `crates/cratebay-gui/src/tools/sandboxTools.ts` — Sandbox Agent Tools
- `crates/cratebay-gui/src/stores/sandboxStore.ts` — 沙盒状态管理
- `crates/cratebay-gui/src/components/chat/ToolCallItem.tsx` — 工具卡片
- `crates/cratebay-gui/src/components/chat/ThinkingBlock.tsx` — 思考块
- `crates/cratebay-gui/src/components/chat/SandboxBar.tsx` — 沙盒状态栏

### 修改
- `crates/cratebay-gui/src/lib/systemPrompt.ts` — 重写
- `crates/cratebay-gui/src/lib/agent.ts` — 注册 sandboxTools
- `crates/cratebay-gui/src/pages/ChatPage.tsx` — 沙盒状态 + 欢迎页
- `crates/cratebay-gui/src/stores/settingsStore.ts` — Provider 配置迁移
- `crates/cratebay-gui/src/stores/chatStore.ts` — boundSandboxId
- `crates/cratebay-gui/src/tools/index.ts` — 导出 sandboxTools
- `crates/cratebay-gui/src/components/chat/MessageBubble.tsx` — Round 分组
- `crates/cratebay-gui/package.json` — 升级依赖
