# LiveAgent AI 能力全量移植到 CrateBay: 可执行任务单

> 目标: 将 `~/Personal/LiveAgent` 中 AI 相关能力(对话/供应商设置/cron/skills/MCP/执行模式/上下文压缩/前端流式对话/agent dev 调试/ hooks 等)在 CrateBay 中完整实现一遍, 并与 CrateBay 的容器与 sandbox 运行时能力正确集成, 最终做到可用、可维护、可测试。
>
> 范围: 本任务单覆盖从底层 IPC 契约修复到完整功能 parity 的开发/测试/文档工作。按 Phase 依赖顺序执行, 任何 Phase 未完成不要跳到后续 Phase。

---

## 0. 关键结论(迁移前必须认清的差异)

### 0.1 LiveAgent 的 AI 能力面(作为“金标准”)

以下能力在 LiveAgent 中是端到端闭环的, CrateBay 需要对齐:

1. 对话(多会话/持久化/标题/消息与 toolcalls/toolresults 的结构化展示)
2. 供应商(Provider)设置: baseUrl/apiKey/模型列表/启用模型/Responses vs Completions/Reasoning/Prompt caching/模型上下文窗口与 max output 配置
3. 执行模式: chat(text)/agent(tools)/agent-dev(调试与更强可观测)
4. 上下文压缩: 自动 checkpoint、节流、tool output 裁剪、mid-turn compaction、prune 策略, 并可持续运行
5. Skills: 固定技能目录扫描 + 元数据注入 + progressive disclosure(ReadSkill 工具按需读取技能文件)
6. MCP: server 配置(transport/timeout/headers/env/cwd 等) + 选择加载 + 安全 tool 命名与映射 + 调用闭环
7. Cron: 后端 scheduler + 前端 runner + 任务 CRUD + 执行日志 + prompt cron(需要 agent mode)闭环
8. Hooks: 对 agent 生命周期事件绑定 command/http hooks, 可配置可持久化, 有警告提示
9. 前端流式对话与 tool UI: token 流式渲染 + toolcall 可视化 + 状态条/进度/错误恢复
10. Agent Dev: Debug JSONL 记录请求/结果/错误与关键信息, 可追溯

LiveAgent 关键参考(用于移植对照):

1. 工具注册与分组: `~/Personal/LiveAgent/crates/agent-gui/src/lib/tools/builtinRegistry.ts`, `builtinTypes.ts`
2. Context Compaction: `~/Personal/LiveAgent/crates/agent-gui/src/lib/chat/contextCompaction.ts`
3. Skills: `~/Personal/LiveAgent/crates/agent-gui/src/lib/skills/index.ts`, `skillTools.ts`
4. Cron: `~/Personal/LiveAgent/crates/agent-gui/src/components/cron/CronPromptRunner.tsx`, `~/Personal/LiveAgent/crates/agent-gui/src-tauri/src/services/cron.rs`, `cronTools.ts`
5. Hooks: `~/Personal/LiveAgent/crates/agent-gui/src/lib/hooks/conversationHooks.ts`, `~/Personal/LiveAgent/crates/agent-gui/src-tauri/src/commands/hook.rs`
6. MCP: `~/Personal/LiveAgent/crates/agent-gui/src/lib/tools/mcpTools.ts`
7. Agent Dev Debug: `~/Personal/LiveAgent/crates/agent-gui/src/lib/debug/agentDebug.ts`
8. File 工具(带版本校验): `~/Personal/LiveAgent/crates/agent-gui/src/lib/tools/fsTools.ts`, `fileToolState.ts`

### 0.2 CrateBay 当前 ChatPage “不可用”的底层根因(必须 Phase 0 先修)

CrateBay 当前存在多处“前端 invoke 参数名/结构体 JSON casing/后端命令签名”不一致, 导致:

1. 对话持久化基本不可用(会话标题/消息保存调用参数名不匹配)
2. MCP tool call 失败(参数名不匹配, server list 结构与 UI types 不一致)
3. LLM 流式 tool-call 参数解析错误(后端把 tool arguments delta 当 Token 发出, 前端当成文本拼接)
4. streamFn 序列化字段名与 Rust `#[serde(rename_all = "camelCase")]` 不一致(导致 options/toolCalls/toolCallId/usage 等字段被忽略)
5. 文件/ shell 工具链不闭环: 前端声明了 container_file_* 命令但后端不存在; shell_exec 传了后端不存在的 timeout 参数; system prompt 约定与实现不一致

CrateBay 关键对照点(用于修复/移植):

1. 对话持久化: `crates/cratebay-gui/src/stores/chatStore.ts`, `crates/cratebay-gui/src-tauri/src/commands/storage.rs`
2. LLM 流式桥接: `crates/cratebay-core/src/llm_proxy.rs`, `crates/cratebay-gui/src/lib/streamFn.ts`
3. MCP: `crates/cratebay-gui/src/stores/mcpStore.ts`, `crates/cratebay-gui/src/tools/mcpTools.ts`, `crates/cratebay-gui/src-tauri/src/commands/mcp.rs`, `crates/cratebay-core/src/models.rs`
4. 文件/ shell 工具: `crates/cratebay-gui/src/tools/filesystemTools.ts`, `shellTools.ts`, `crates/cratebay-gui/src-tauri/src/commands/container.rs`
5. Sandbox 能力差距: GUI 只有 `sandbox_run_code`/`sandbox_install`, 但 `cratebay-mcp` 已有完整 sandbox 生命周期与 put/get_path 能力

---

## 1. 迁移总体策略(必须遵守)

1. Phase 0 先把 CrateBay 现有 Chat/MCP/LLM Streaming 的“契约”修到正确可用, 否则后续移植全部建立在沙子上。
2. 迁移不照搬 LiveAgent “前端持有 apiKey”的模式: CrateBay 必须保持 API keys 仅在 Rust 端存储与使用。
3. 以 CrateBay sandbox/container 为 workspace 根语义: 用“会话级 workspace sandbox”替代 LiveAgent 的 host workdir, 后续再做 host mount 与导入。
4. 对齐 LiveAgent 的能力面时优先复用其“设计与边界”: progressive disclosure、工具安全命名、版本校验的 FS 工具、compaction prompt 注入防 prompt injection 等。
5. 把“契约一致性”当做一等公民: 引入 tauri-specta types 输出或至少制定统一 IPC casing 规则并测试覆盖。

---

## 2. 能力清单到任务映射(自检用)

迁移完成的定义: 以下每一项都必须能在 CrateBay 中运行并通过测试。

1. Chat(text) 模式: 无 tools, 纯对话, 流式输出稳定, 支持取消, 持久化正确
2. Agent(tools) 模式: 能正确 toolcall -> 执行 -> toolresult -> 继续推理, tool UI 正确展示, 持久化正确
3. Agent-dev 模式: 具备 debug jsonl/可观测/更详细 UI, 且不泄露 secrets
4. Provider 设置: 增删改查, api key 保存删除, model fetch/enable, reasoning 与请求格式, 模型窗口与 max output 配置
5. MCP: server 增删改, transport/timeout/headers/env/cwd, 选择加载, safe tool 命名, 调用闭环
6. Skills: 扫描固定 skills 目录, 元数据展示, 选择启用, system prompt 注入, ReadSkill 工具可用
7. Context Compaction: 自动 checkpoint, 节流, mid-turn 与 post-tool, tool output 裁剪, 可恢复继续对话
8. Cron: 任务 CRUD, 三种类型(bash/http/prompt), prompt runner 前后端闭环, 执行日志, 超时与重试策略
9. Hooks: lifecycle 事件绑定, command/http 执行, 错误 warning, 可靠持久化
10. 与 CrateBay 容器能力集成: workspace sandbox 生命周期、文件与命令在 sandbox 内执行、与 runtime/containers/images 页面兼容

---

## 3. Phase 0: 让 CrateBay ChatPage 可用(契约修复 + 流式正确)

### [ ] CB-AI-000: 定义并落地 IPC 命名与 casing 规则(“唯一真相”)

Scope:
1. 明确 Tauri command 顶层参数采用 Rust 函数参数名(通常 snake_case): 例如 `session_id`, `server_id`
2. 明确 Rust structs 通过 `#[serde(rename_all = \"camelCase\")]` 暴露给前端时, JSON 字段应为 camelCase: 例如 `toolCalls`, `createdAt`, `promptTokens`
3. 在 `docs/specs/api-spec.md` 与 `docs/specs/agent-spec.md` 中把 JSON shape 明确写清(不能只写 Rust 字段名)

Acceptance:
1. spec 中每个关键结构体(Conversation*, SaveMessageRequest, ChatMessage, LlmOptions, UsageStats, McpServerStatus 等)明确标注“前端看到的字段名”
2. 新增一段“IPC contract lint checklist”: 任何 `invoke()` 必须匹配命令签名与 struct casing

Docs:
1. `docs/specs/api-spec.md`
2. `docs/specs/agent-spec.md`
3. `docs/specs/mcp-spec.md`(如涉及 MCP config 扩展)

---

### [ ] CB-AI-001: 修复对话持久化: conversation_save_message / conversation_update_title 调用契约

Current breakage:
1. `chatStore.ts` 传 `sessionId`, 后端命令参数是 `session_id`
2. `SaveMessageRequest`/`ConversationSummary`/`ConversationDetail` 等在前端按 snake_case 解析, 但 Rust models 实际 serialize 为 camelCase

Scope:
1. 更新 `crates/cratebay-gui/src/stores/chatStore.ts`:
2. `conversation_update_title` 改为 `{ session_id: id, title }`
3. `conversation_save_message` 改为 `{ session_id: sessionId, message: <camelCase SaveMessageRequest> }`
4. `conversation_list`/`conversation_get_messages` 的响应解析改为 camelCase 字段(`messageCount`, `createdAt`, `updatedAt`, `lastMessagePreview`, `toolCalls`, `toolCallId`)

Acceptance:
1. 真实 Tauri 环境下, 新建会话、发送消息、重启应用后会话与消息可完整恢复
2. 标题更新可持久化
3. tool_calls/tool_call_id 可正确序列化保存并从 DB 读回

Tests:
1. 新增或更新 `crates/cratebay-gui/src/__tests__/chat.test.tsx` 覆盖会话加载与消息保存契约
2. 增加一个“camelCase 反序列化”单测, 防止回归

Docs:
1. `docs/specs/api-spec.md` conversation 相关 JSON 字段名说明

---

### [ ] CB-AI-002: 修复 MCP tool call IPC 参数名与 server list 类型契约

Current breakage:
1. `mcp_client_call_tool` 后端参数为 `server_id/tool_name`, 前端传 `serverId/toolName`
2. `mcp_server_list` 返回 `McpServerStatus[]`, 前端 types/store 当作 `McpServerInfo[]` 使用
3. `env` 类型: 后端 `Vec<String>(KEY=VALUE)`, 前端当成 `Record<string,string>`

Scope:
1. 修复 `crates/cratebay-gui/src/tools/mcpTools.ts` 与 `crates/cratebay-gui/src/stores/mcpStore.ts`:
2. `mcp_client_call_tool` 参数名改为 snake_case
3. server list 改为以 `McpServerStatus` 为真实 source:
4. 前端 `McpServerInfo/McpServerConfig` types 与 UI 字段映射统一(不要“想象字段”)
5. env 转换: UI <-> API 之间提供稳定转换函数(Record <-> string[])

Acceptance:
1. MCP server 能新增/启动/停止/删除
2. 可列出 tools, 并可成功 call tool
3. UI 显示 running/pid/tools 数量正确

Tests:
1. 更新 `crates/cratebay-gui/src/__tests__/mcpStore.test.ts`
2. 更新 `crates/cratebay-gui/src/__tests__/mcpServerList.test.tsx`

Docs:
1. `docs/specs/api-spec.md` MCP section 的返回类型与 JSON 字段名说明

---

### [ ] CB-AI-003: 修复 streamFn 的请求序列化(camelCase)与 usage 字段读取

Current breakage:
1. `streamFn.ts` 发送 `tool_calls/tool_call_id/max_tokens/reasoning_effort` 等 snake_case, Rust structs 期望 camelCase
2. `UsageStats` 前端按 snake_case 读, Rust 实际 camelCase

Scope:
1. 更新 `crates/cratebay-gui/src/lib/streamFn.ts`:
2. ChatMessage: `toolCalls`, `toolCallId`
3. LlmOptions: `maxTokens`, `reasoningEffort`, `topP`
4. UsageStats: `promptTokens`, `completionTokens`, `totalTokens`
5. 同步更新 `crates/cratebay-gui/src/types/agent.ts` 里的 UsageStats

Acceptance:
1. reasoning effort 在 OpenAI Responses provider 下可生效
2. toolcall/toolresult 轮次中, 后端能正确接收 toolCalls/toolCallId 历史
3. UI 能展示 usage(至少 totalTokens)且不为 0

Tests:
1. 新增 `streamFn` 单测: 输入 context, 断言 invoke payload keys 为 camelCase

Docs:
1. `docs/specs/agent-spec.md` 里的 streamFn 示例需与实现一致(特别是 casing)

---

### [ ] CB-AI-004: 修复 llm_proxy.rs 的 tool-call 流式事件: 不能把 tool arguments delta 当 Token 发出

Current breakage:
1. `cratebay-core/src/llm_proxy.rs` 在 Anthropic/OpenAI Responses/OpenAI Completions 的 tool arguments delta 分支发 `LlmStreamEvent::Token`
2. 前端将其拼进 assistant 文本, 导致 UI 污染且 toolcall 参数不完整/解析失败

Scope:
1. 在后端对每个 tool call id 做 arguments 累积, 直到形成完整 JSON 字符串
2. 仅在完整 JSON 就绪时发出 `LlmStreamEvent::ToolCall { id, name, arguments }`
3. Token 仅用于真实输出文本 token

Acceptance:
1. 任一 provider(tool calling)下, UI 不出现“tool args 混进文本”的现象
2. toolcall arguments 在前端 JSON.parse 成功率接近 100%(除非 provider 真的发坏数据)

Tests:
1. Rust 单测覆盖三类 API format 的 tool-call 流式拼装
2. 前端 agentFlow 增加一个“真实 ToolCall event”驱动的工具调用回归用例

Docs:
1. `docs/specs/api-spec.md` Streaming Events 的 ToolCall.arguments 明确为“完整 JSON string”

---

### [ ] CB-AI-005: 修复 ChatPage 流式消息持久化策略(placeholder 与最终消息一致)

Current breakage:
1. `useAgent.ts` 在 `message_start` 立刻持久化一个空 assistant message
2. stream 更新不持久化, 重启后会丢失/错乱

Scope:
1. 设计并实现“流式期间内存更新, message_end 时一次性持久化最终 assistant 消息(含 toolCalls)”
2. 或者实现增量持久化(需 DB schema 支持), 但必须保证回放一致性

Acceptance:
1. 真实 Tauri 环境下, 正在流式时崩溃/重启后不会出现 DB 中大量空 assistant message
2. 对话恢复后 assistant 最终内容正确

Tests:
1. 前端 store + useAgent 集成测试: message_start/update/end 的持久化调用次数与 payload 校验

Docs:
1. `docs/specs/agent-spec.md` 增补“消息持久化策略”说明

---

### [ ] CB-AI-006: 补齐 container_file_read/write/list 的后端 Tauri commands 或替换为 cratebay-mcp sandbox put/get 能力

Current breakage:
1. `filesystemTools.ts` 调用 `container_file_read/container_file_write/container_file_list`, 但后端不存在

Decision:
1. 方案 A: 在 GUI 后端增加 `container_file_*` commands, 基于 `cratebay_core::container::exec_get_file/exec_put_text`
2. 方案 B: 统一走 sandbox 生命周期能力, 提供 `sandbox_get_path/sandbox_put_path` 并在 file tools 中使用

Acceptance:
1. file_read/file_write/file_list 工具可用并且 scoped 到容器/sandbox
2. 能支持后续 LiveAgent fsTools 的分页读取/版本校验扩展(至少不阻塞)

Tests:
1. Rust: container 文件读写错误处理(文件不存在/权限/目录)测试
2. Frontend: filesystemTools invoke payload 与返回值渲染测试

Docs:
1. `docs/specs/api-spec.md` 增加 container_file_* 或 sandbox_* 文件 API
2. `docs/specs/agent-spec.md` 更新文件工具描述

---

### [ ] CB-AI-007: 修复 shell_exec 与 container_exec 的参数契约(删除无效 timeout 或实现 timeout 版本)

Current breakage:
1. `shellTools.ts` 调用 `container_exec` 时传 `timeout`, 但后端 `container_exec` 不接收该参数

Scope:
1. 方案 A: shell_exec 去掉 timeout, 只提供工作目录字段
2. 方案 B: 新增 `container_exec_with_timeout`(或扩展 container_exec)并在后端用 `exec_with_timeout`

Acceptance:
1. shell_exec 在真实环境不报参数反序列化错误
2. timeout 行为与工具描述一致

Tests:
1. 前端 shellTools 单测断言 invoke 参数正确
2. Rust 单测或集成测试覆盖 timeout 超时路径

Docs:
1. `docs/specs/api-spec.md` container_exec 参数表更新

---

### [ ] CB-AI-008: 修复 systemPrompt 中关于 sandbox “cleanup=false” 的不一致描述

Current breakage:
1. `buildSystemPrompt` 要求使用 cleanup=false 保持 sandbox, 但 `sandbox_run_code` 命令不支持该参数

Scope:
1. 短期: 修改 prompt 文案匹配现状(不要误导模型)
2. 中期: 实现 GUI 侧完整 sandbox 生命周期(见 Phase 1), 让 prompt 与能力一致

Acceptance:
1. system prompt 不再包含无法实现的操作说明

Docs:
1. `docs/specs/agent-spec.md` 的 sandbox 行为说明与实现一致

---

## 4. Phase 1: 在 CrateBay 建立 LiveAgent 同等级 AI 平台能力(ExecutionMode + Workspace + Tool Registry)

### [ ] CB-AI-100: 引入“会话级 Workspace Sandbox”模型并持久化到 conversation.metadata

Goal:
1. 用 sandbox(container)作为每个 conversation 的 workspace 根, 替代 LiveAgent 的 host workdir

Scope:
1. 定义 `WorkspaceSession` 数据结构(会话 id -> sandbox_id/template_id/workspace_root/ttl 等)
2. 将其存入 conversations.metadata(JSON)
3. 增加/扩展 Tauri commands:
4. `workspace_get(conversation_id)` / `workspace_set_template(...)` / `workspace_ensure_started(...)` / `workspace_reset(...)`

Acceptance:
1. 新开会话可创建 workspace sandbox
2. 切换会话能切换 workspace
3. 重启后能恢复 workspace 绑定(若 sandbox 仍存在), 否则能自动重建

Tests:
1. Rust: metadata 读写 + 兼容旧数据
2. Frontend: chat 切换会话后的 workspace 行为

Docs:
1. `docs/specs/database-spec.md`(metadata schema 约定)
2. `docs/specs/agent-spec.md`(workspace 概念)

---

### [ ] CB-AI-101: 将 cratebay-mcp 的 sandbox 生命周期能力抽到 cratebay-core 复用, 并在 GUI 暴露完整 sandbox commands

Goal:
1. GUI 获得与 `cratebay-mcp` 等价的 sandbox 管理能力: list/inspect/create/start/stop/delete/exec/put_path/get_path/cleanup_expired/templates

Scope:
1. 识别 `crates/cratebay-mcp/src/sandbox.rs` 与 `tools.rs` 中可复用逻辑
2. 抽取到 `crates/cratebay-core/src/sandbox_*` 或新 module
3. GUI 增加对应 Tauri commands(命名与 api-spec 对齐)

Acceptance:
1. GUI 可创建长生命周期 sandbox 并可执行命令/读写文件
2. sandbox TTL 与 cleanup_expired 可工作

Tests:
1. Rust: sandbox label/TTL 计算/cleanup 逻辑单测
2. GUI: e2e(Playwright)最小用例: 创建 sandbox -> exec -> delete

Docs:
1. `docs/specs/api-spec.md` 增加 sandbox commands
2. `docs/specs/mcp-spec.md` 确保 GUI 与 MCP server 的 sandbox 行为一致

---

### [ ] CB-AI-102: 实现 Execution Mode: chat(text) / agent(tools) / agent-dev(调试)

Goal:
1. 对齐 LiveAgent 的 `ExecutionMode = text | tools | agent-dev`

Scope:
1. settings 增加 `executionMode`(以及与之相关的选项: selectedMcpServers, skillsEnabled 等)
2. ChatPage 根据 mode 决定:
3. text: 不注入 tools, 禁止 toolcalls(或 tool list 为空)
4. tools: 注入 builtin + selected MCP + skills tool 等
5. agent-dev: tools 模式 + debug logging + 更多 UI

Acceptance:
1. 三种模式切换实时生效
2. text 模式下模型不出现 toolcalls(或被拦截并提示用户切换)
3. agent-dev 模式下 debug 日志写入工作

Tests:
1. 前端: mode 切换时 agent.setTools 行为断言

Docs:
1. `docs/specs/frontend-spec.md` Settings/Chat mode UI
2. `docs/specs/agent-spec.md` ExecutionMode 行为

---

### [ ] CB-AI-103: 建立“工具注册表 v2”(bundles + metadata + per-mode 选择加载)

Goal:
1. 对齐 LiveAgent 的 builtinRegistry: tool bundles + executeToolCall 路由 + metadataByName

Scope:
1. 在 CrateBay 侧创建 tool registry 模块(建议新目录: `crates/cratebay-gui/src/lib/tools-v2/`)
2. 维护工具元数据: groupId, displayCategory, isReadOnly, 风险等级, 运行域(runtimeScope)
3. 支持按 mode/ settings 选择加载: fs/shell/skills/system/mcp/sandbox/container 等

Acceptance:
1. agent 的 tools 集合只由 registry 产出, 不再“到处拼 array”
2. MCP tools 安全命名与映射在 registry 内闭环

Tests:
1. 工具去重/命名冲突检测单测
2. registry 输出快照测试(确保加载集稳定)

Docs:
1. `docs/specs/agent-spec.md` Tool registry 标准

---

### [ ] CB-AI-104: MCP Settings 全量对齐 LiveAgent 能力(transport/http/sse/stdio + headers + timeout + selected)

Goal:
1. 将 MCP server 配置项对齐 LiveAgent: transport, url, messageUrl, headers, env, cwd, timeoutMs, enabled, selected

Scope:
1. 扩展 DB schema(如必要): mcp_servers 表增加 url/headers/timeout/message_url/transport 等字段
2. 扩展 Rust MCP Client 支持 streamable HTTP transport(除 stdio/SSE 外)
3. Settings UI 支持:
4. server CRUD + enable toggle
5. selected server 列表(仅 selected 会注入到 tools 模式)

Acceptance:
1. 至少支持 stdio + sse; http transport 若纳入 scope 必须可用
2. selected 控制生效: 仅 selected servers 的 tools 出现在 agent tools 中
3. safe tool naming: toolName <= 64, 仅允许 [a-zA-Z0-9_-], 超长使用 hash 后缀

Tests:
1. Rust: transport 解析与连接单测
2. Frontend: selected servers 过滤工具集单测

Docs:
1. `docs/specs/mcp-spec.md`
2. `docs/specs/database-spec.md`
3. `docs/specs/api-spec.md`

---

### [ ] CB-AI-105: Provider 设置补齐 LiveAgent 的模型配置与推理相关字段

Goal:
1. 支持 per-model contextWindow/maxOutputToken, per-provider request format/ reasoning/ prompt caching

Scope:
1. 扩展 `ai_providers/ai_models` 表或增加新表存储 model config
2. Settings UI:
3. provider 级 request format(openai_completions/openai_responses)与默认 reasoning
4. model 级 contextWindow/maxOutputToken 编辑
5. ChatPage buildModel 不再 hardcode(128000/4096), 改为读取配置

Acceptance:
1. 不同模型的上下文窗口与 max output 可配置并在 compaction 与 stream options 中生效
2. OpenAI Responses 的 reasoning effort 生效

Tests:
1. 后端: schema migration + 读写测试
2. 前端: buildModel 单测

Docs:
1. `docs/specs/database-spec.md`
2. `docs/specs/frontend-spec.md`

---

### [ ] CB-AI-106: 移植 LiveAgent FS 工具集(Read/Write/Edit/Delete/List/Grep/Glob)到“workspace sandbox”语义

Goal:
1. 对齐 LiveAgent `fsTools.ts` 的稳定契约与“读后写”版本校验, 但把 workdir/host FS 替换为 workspace sandbox 内的相对路径

Scope:
1. 定义 workspace 根目录(例如 `/workspace`)并在 sandbox create 时保证存在
2. 后端新增 workspace FS commands(命名可参考 LiveAgent 的 system_read/system_write, 但建议 cratebay 前缀):
3. `workspace_fs_read`(text/image/pdf/notebook, 支持分页/截断, 返回 mtimeMs + contentHash + isPartialView)
4. `workspace_fs_write`(rewrite only, 需 expected 版本信息以拒绝 stale rewrite)
5. `workspace_fs_edit`(exact replacement, 支持 replace_all/expectedReplacements)
6. `workspace_fs_delete`
7. `workspace_fs_list`(depth/offset/maxResults/hasMore)
8. `workspace_fs_glob`(pattern/offset/maxResults)
9. `workspace_fs_grep`(pattern/filePattern/context/multiline/outputMode/offset/headLimit)
10. 前端移植 `FileToolState`(参考 LiveAgent `fileToolState.ts`), 以 conversationId 为维度保存最新 snapshot
11. 前端 AgentTools:
12. 保留 LiveAgent 命名(`Read/Write/Edit/...`)或映射到 snake_case(`file_read_v2` 等)二选一并在 registry 内统一
13. tool result details 对齐 LiveAgent `builtinTypes.ts`(至少 text/read/write/edit/list/grep/glob 的关键字段要一致)
14. 安全: path 必须为相对路径, 禁止绝对路径与 `..`/UNC, 统一分隔符为 `/`

Acceptance:
1. 在 tools/agent-dev 模式下, 模型可仅依赖 FS 工具完成“读取 -> 修改 -> 回写”闭环
2. 未 Read 的既有文件不可 Write/Edit(必须报错提示先 Read)
3. file 变更检测生效: 文件在 Read 后被外部修改, Write/Edit 必须拒绝并要求重新 Read

Tests:
1. Rust: path traversal/分页/grep/glob 的正确性与边界
2. Frontend: FileToolState 行为单测(部分读取 vs 全量读取, stale 检测)

Docs:
1. `docs/specs/api-spec.md` 增加 workspace_fs_* 命令
2. `docs/specs/agent-spec.md` 工具标准补齐 FS v2
3. `docs/specs/testing-spec.md` 增加 FS 工具测试要求

---

### [ ] CB-AI-107: 移植 LiveAgent Shell/Bash 工具到 workspace sandbox(含并行批处理与工具状态)

Goal:
1. 对齐 LiveAgent `shellTools.ts` 与 agentRunner 的 Bash 并行执行体验, 但执行环境为 workspace sandbox

Scope:
1. 后端新增 `workspace_shell_run`:
2. 输入: command(string) 或 argv(string[]), workingDir(relative?), timeoutSeconds
3. 输出: exitCode/stdout/stderr/durationMs
4. 前端新增 Bash AgentTool(命名 `Bash` 或 `shell_exec_v2`)并在 registry 中按 mode 选择加载
5. agent 侧实现“连续 Bash toolcalls”自动并行批处理(参考 LiveAgent `agentRunner.ts` 的 bash batch 逻辑)
6. UI: tool status 提示“正在并行执行 N 个 Bash 命令...”

Acceptance:
1. 连续多个 Bash toolcall 会并行执行并正确回填 toolresults
2. 超时可控且不会卡死 agent

Tests:
1. 前端: bash batch 分组逻辑单测
2. 后端: timeout + stderr 拼接行为测试

Docs:
1. `docs/specs/agent-spec.md` Bash 并行策略说明
2. `docs/specs/api-spec.md` workspace_shell_run

---

### [ ] CB-AI-108: 引入 Agent Prompt Templates(Agents)与 system prompt 组装策略对齐 LiveAgent

Goal:
1. 对齐 LiveAgent Settings -> Agents(多模板/启用一个/追加到 system prompt), 让用户可配置 agent 行为

Scope:
1. DB/Settings: 保存 agent prompt templates(JSON 或新表)
2. UI: Settings -> Agents:
3. 列表/新增/编辑/启用/禁用
4. Chat: system prompt 组装顺序明确:
5. base system prompt + tools suffix + active agent prompt + selected skills prompt + workspace 信息

Acceptance:
1. 启用的 agent 模板会影响后续对话与 cron prompt runner 的 system prompt
2. 切换模板即时生效且可持久化

Tests:
1. 前端: prompt 组装快照测试

Docs:
1. `docs/specs/frontend-spec.md` Agents settings
2. `docs/specs/database-spec.md` agents storage

---

### [ ] CB-AI-109: 增加 MCP server “更新”命令与前端 edit 流程闭环(避免 remove+add 导致 id 漂移)

Goal:
1. 对齐 LiveAgent 直接编辑 server 配置的体验, 并避免 CrateBay 现在的 remove+add 造成 id 变化与 tool name 变化

Scope:
1. 后端新增 `mcp_server_update(id, patch)`:
2. 更新 SQLite 配置并让 McpManager 重新加载/重连
3. 前端 `mcpStore.updateServer` 改为调用 update 命令

Acceptance:
1. 编辑后 server id 不变
2. 更新配置后 tools 能重新同步且旧 tools 不残留

Tests:
1. Rust: update 后重连与 tools 刷新测试
2. Frontend: updateServer 调用参数与 UI 状态机测试

Docs:
1. `docs/specs/api-spec.md` 新增 mcp_server_update

---

### [ ] CB-AI-110: Workspace Sandbox 的 UI 集成(选择模板/重置/查看状态/TTL)

Goal:
1. 让用户在 Chat/Settings 中可见且可控 workspace sandbox, 不再是隐式行为

Scope:
1. ChatPage 顶部增加 workspace 状态区:
2. 当前 sandbox_id/template/running/ttl/last error
3. 操作: 选择 template, 重建(reset), 打开 terminal(可选)
4. Settings -> Runtime 或新增 Settings -> Sandbox:
5. 默认 template/TTL/资源限制(cpu/mem)与清理策略
6. 与 conversation.metadata 绑定逻辑一致

Acceptance:
1. 用户可为不同会话选择不同 template
2. reset 后 workspace 文件与状态清空且不会影响其它会话

Tests:
1. 前端: 切换会话 workspace 状态渲染与操作回归

Docs:
1. `docs/specs/frontend-spec.md` workspace UI

---

### [ ] CB-AI-111: 引入“可选 System Tools”与 runtimeScope gating(对齐 LiveAgent customSystemTools)

Goal:
1. 支持用户在 Settings 选择启用哪些 system tools, 并按 runtimeScope(chat/cron 等)裁剪工具集

Scope:
1. 定义 `SystemToolId` 列表与描述, 并在 Settings 提供 multi-select
2. tool registry v2 支持:
3. selectedSystemToolIds
4. runtimeScope = chat | cron_auto_prompt | ...
5. 部分 system tools 只在特定 scope 可用

Acceptance:
1. 未选择的 system tool 不会出现在 agent tools 中
2. cron runner 只加载允许的 system tools

Tests:
1. registry 输出根据 scope/selection 的快照测试

Docs:
1. `docs/specs/agent-spec.md` runtimeScope 与 system tools
2. `docs/specs/frontend-spec.md` Settings -> System tools

---

### [ ] CB-AI-112: System Prompt “工具模式后缀”与操作规约对齐 LiveAgent(提升 agent 成功率)

Goal:
1. 对齐 LiveAgent `buildToolsSuffix` 的操作规约: workspace 根/路径限制/工具使用顺序/禁止泄露 JSON 等

Scope:
1. 在 CrateBay system prompt 组装中引入 “tools suffix”:
2. workspace root 说明(相对路径规则)
3. FS 工具分页/版本校验/优先 grep/glob/list 再 read 再 write/edit
4. MCP tools 命名约定(mcp_)
5. 根据 ExecutionMode:
6. text 模式: 不出现 tools suffix
7. tools/agent-dev: 注入 suffix

Acceptance:
1. tools 模式下, 模型不会频繁使用绝对路径/.. 之类危险路径
2. 模型能按规约优先使用 grep/glob/list 而不是滥用 Bash

Tests:
1. prompt 组装快照测试(不同模式/不同 selection)

Docs:
1. `docs/specs/agent-spec.md` system prompt 组装顺序与 suffix 内容

---

## 5. Phase 2: 移植 LiveAgent 高级能力(技能/压缩/hooks/cron/agent-dev)

### [ ] CB-AI-199: 增加 “非 UI 流式” 的 LLM 调用能力(供 compaction/cron/internal use)

Goal:
1. 为 context compaction、cron prompt runner(可选)、后台自检等场景提供一个“收集最终 assistant message”的调用方式, 不依赖 UI token 流渲染

Scope:
1. 后端新增命令(二选一):
2. `llm_proxy_complete(provider_id, model_id, messages, options?) -> { message: ChatMessage, usage: UsageStats }`
3. 或 `llm_proxy_stream_collect(...)` 在后端内部订阅 SSE 并收集为最终结果
4. 复用现有 provider lookup/dual headers/request transform 逻辑, 保持 API keys 不出后端
5. 支持 tool definitions 但默认禁用工具执行(防止 compaction 触发 tools)

Acceptance:
1. compaction 能稳定拿到最终 summary 文本与 usage
2. 请求失败能返回可诊断错误(但不泄露 secrets)

Tests:
1. Rust: mock SSE 流收集测试(含 Done/ Error)

Docs:
1. `docs/specs/api-spec.md` 增加 llm_proxy_complete
2. `docs/specs/agent-spec.md` 说明 compaction/internal LLM 调用路径

---

### [ ] CB-AI-200: Skills 目录扫描 + 元数据读取 + ReadSkill tool(Progressive Disclosure)

Goal:
1. 对齐 LiveAgent Skills 机制(固定目录扫描, 仅注入元数据, 通过 ReadSkill 工具按需读取)

Scope:
1. 后端 system commands:
2. `system_list_skill_files`
3. `system_read_skill_metadata`
4. `system_read_skill_text`(支持 offset/length)
5. 前端:
6. Skills settings: enabled + selected skills
7. system prompt: 注入 skill metadata 列表与规则
8. tool: `ReadSkill`(或 `skill_read`) 读取 skill file

Acceptance:
1. 能扫描到 skills 并在 Settings 展示/选择
2. 选择后 system prompt 注入 metadata
3. ReadSkill 工具可读取技能文件(支持截断/分页)

Tests:
1. 后端: path traversal 防护测试
2. 前端: skills discovery 与 prompt 注入单测

Docs:
1. `docs/specs/agent-spec.md`
2. `docs/specs/api-spec.md`

---

### [ ] CB-AI-201: 移植 contextCompaction 引擎(含 checkpoint/节流/mid-turn/post-tool)

Goal:
1. 对齐 LiveAgent `contextCompaction.ts` 的策略与安全 prompt(防 prompt injection)

Scope:
1. 移植 conversationState/segments/checkpoint 结构
2. 在 agent loop 中引入 compaction hooks:
3. pre-send: 发送前判断是否需要 compact
4. mid-stream: token 超限/usage 超阈值时触发
5. post-tool: tool output 过大触发
6. 实现 compaction LLM 调用:
7. 使用 CrateBay backend provider(不能用前端 apiKey)
8. 需要一个“非 UI 流式”的内部调用方式(收集最终 assistant message)
9. tool output prune: 对超大输出做裁剪标记, 保护上下文预算

Acceptance:
1. 在长对话/大 tool 输出情况下不会崩溃或无限增长
2. compaction 产物可持续续聊, 且包含 artifacts/decisions/open loops/next steps 等结构
3. 有节流与最大 compaction 次数限制, 避免抖动

Tests:
1. compaction decision 逻辑单测(阈值/节流/触发原因)
2. compaction payload 序列化稳定性测试

Docs:
1. `docs/specs/agent-spec.md` 增补 compaction 章节

---

### [ ] CB-AI-202: 移植 Conversation Hooks(生命周期事件 command/http)

Goal:
1. 对齐 LiveAgent hook 体系: agent_start/turn_start/message_start/.../agent_end 等事件可配置 hooks

Scope:
1. DB schema: hook settings 持久化(建议新表或 settings JSON)
2. 后端 commands:
3. `hook_run_commands`(在 workspace sandbox 内执行, 支持多步 commands)
4. `hook_run_http_requests`(支持 headers/body)
5. 前端:
6. hook dispatcher(队列串行, warning 回传)
7. Settings UI: hooks CRUD, event 分组, enable toggle

Acceptance:
1. hooks 在事件触发时可靠执行
2. hook 失败不会中断主对话, 但会在 UI 提示 warning

Tests:
1. 后端: command/http 执行错误处理与超时测试
2. 前端: dispatcher 顺序与 warning 处理单测

Docs:
1. `docs/specs/api-spec.md` 新增 hook commands
2. `docs/specs/database-spec.md` hook schema

---

### [ ] CB-AI-203: 移植 Cron(任务调度 + Prompt Runner + 管理工具)

Goal:
1. 对齐 LiveAgent cron: bash/http/prompt 三类任务 + logs + prompt runner 前后端闭环

Scope:
1. DB schema: cron tasks + execution logs
2. Rust: 引入 scheduler(参考 `tokio_cron_scheduler`)与 CronManager
3. Rust events:
4. `cron:auto-prompt-pending`
5. `cron:auto-prompt-expired`
6. 前端 CronPromptRunner:
7. 监听 pending event, 在 agent mode 下运行 prompt, 回传 completion
8. Settings UI: cron tasks CRUD, logs 查看
9. 工具: `ManageCronTask`(或等价 snake_case)允许 agent 通过对话修改 cron 配置

Acceptance:
1. cron bash/http 任务可按计划执行并记录日志
2. prompt 任务: 后端发 pending, 前端执行并回传完成状态, 后端记录日志与超时处理
3. 任务启用/禁用与 cron 表达式更新即时生效

Tests:
1. Rust: cron schedule 解析与执行、超时、already finished 等逻辑测试
2. Frontend: CronPromptRunner 对 pending/expired/completion 的状态机测试

Docs:
1. `docs/specs/backend-spec.md` cron service
2. `docs/specs/api-spec.md` cron commands/events
3. `docs/specs/database-spec.md` cron schema
4. `docs/specs/testing-spec.md` 增补 cron 测试矩阵

---

### [ ] CB-AI-204: Agent Dev Mode 调试日志(jsonl)与可视化

Goal:
1. 对齐 LiveAgent agent dev: 记录 request/result/error 的 debug jsonl, 可追踪每轮对话

Scope:
1. 后端 commands:
2. `system_append_debug_jsonl(conversation_id, entry)`(写入到 `~/.cratebay/logs/` 下)
3. `system_read_debug_jsonl(conversation_id, tail?)`(可选, 便于 UI 展示)
4. 前端 debug logger:
5. 在 agent-dev mode 开启, 每轮请求与关键结果写入 jsonl
6. UI: 在 ChatPage 或单独面板展示最近 debug 条目(可过滤)
7. 安全: sanitize entry, 禁止写入 api key/密钥/敏感 env

Acceptance:
1. agent-dev 模式下 debug jsonl 正确生成且内容可读
2. 不泄露 secrets(审查点: provider apiKey, mcp env, headers)

Tests:
1. sanitize 单测: 输入包含 apiKey 字段必须被替换/移除
2. 后端写入失败不影响主流程

Docs:
1. `docs/specs/backend-spec.md` debug 日志规范

---

## 6. Phase 3: 前端 Chat UI 与工具体验对齐 LiveAgent

### [ ] CB-AI-300: Chat UI 支持 toolcall/toolresult 的结构化展示与状态流转

Goal:
1. 对齐 LiveAgent: toolcall 列表、参数摘要、执行状态、结果折叠、错误恢复

Scope:
1. useAgent 事件桥接: 处理 tool_execution_start/update/end, 并把 tool 事件关联到对应 assistant message
2. MessageList/ToolCallItem: 展示并支持折叠/展开/复制
3. Tool status bar: “正在执行 X 个工具/当前工具/并行 bash”等

Acceptance:
1. agent 模式下, toolcall 与 toolresult 在 UI 可正确追踪
2. tool 执行失败时, UI 展示错误并允许模型恢复继续

Tests:
1. 前端组件快照/交互测试

Docs:
1. `docs/specs/frontend-spec.md` Chat UI 组件约定

---

### [ ] CB-AI-301: 支持 file uploads(可选但建议对齐 LiveAgent)

Goal:
1. 对齐 LiveAgent uploadedFiles: 用户可选择文件, 自动放入 workspace, 消息中注入“请先 Read”指令

Scope:
1. UI: ChatInput 支持选择文件(文本/图片/pdf/notebook)
2. 后端: 将 host 文件复制到 workspace sandbox(用 put_path 或 mount)
3. 前端: 在 user message metadata 记录附件信息用于展示

Acceptance:
1. 上传文件后, agent 能通过 Read 工具读到内容
2. UI 能展示附件列表与大小

Tests:
1. e2e: 上传 -> Read -> 返回摘要

Docs:
1. `docs/specs/frontend-spec.md` uploads UX
2. `docs/specs/api-spec.md` 文件导入接口

---

### [ ] CB-AI-302: MCP 工具命名与分类对齐(避免 tool name 爆炸/不可读)

Goal:
1. 对齐 LiveAgent safeName + toolNameMap

Scope:
1. 对 MCP tools 生成稳定安全 name
2. toolResult details 中携带 server/tool 映射信息便于 UI 展示
3. UI 分类: mcp 工具归类到 MCP 分类

Acceptance:
1. 任意 MCP tool name 不会超过 64 且不会出现非法字符
2. tool 名称冲突可检测并可恢复

Tests:
1. safeName 单测: 超长/非法字符/冲突

---

## 7. Phase 4: 硬化(测试/文档/一致性/发布门禁)

### [ ] CB-AI-400: 引入 tauri-specta types 自动生成并替换手写 types(减少契约漂移)

Goal:
1. 从根上减少“前端想象 API shape”导致的回归

Scope:
1. 在 src-tauri 增加 specta 导出命令或 build step
2. 在 GUI 前端引入生成的 types 并逐步替换 `src/types/*.ts` 手写定义

Acceptance:
1. 关键命令与结构体的 TS types 来自生成产物
2. 手写 types 仅保留 UI 专用派生类型

Tests:
1. CI 中增加类型生成一致性检查(生成产物 diff 为空)

Docs:
1. `docs/specs/architecture.md` 类型契约章节

---

### [ ] CB-AI-401: 增加“契约回归测试套件”(invoke 参数名 + JSON casing + streaming events)

Scope:
1. 前端单测: 检查 invoke payload keys 与 spec 一致
2. Rust 单测: 检查 streaming events 与前端 parser 一致

Acceptance:
1. 任何 casing/参数名回归会在 CI 直接失败

---

### [ ] CB-AI-402: 文档与规格更新闭环

Scope:
1. 任何新增/修改 commands 必须更新 `docs/specs/api-spec.md`
2. 任何新增 store/tool/hook/cron/skills 必须更新对应 spec
3. 完成后更新 `docs/progress.md` 的 Quick Resume

Acceptance:
1. specs 与实现一致(以 specta types/测试为准)

---

## 8. 最终验收(全量移植完成的 DoD)

1. ChatPage 可用: 新建/切换/删除会话、持久化恢复、流式输出、取消
2. 三种执行模式完整: text/tools/agent-dev 均可用且 UI 可切换
3. Provider 设置完整且安全: keys 不出后端, reasoning/requestFormat/模型配置生效
4. Workspace sandbox 闭环: create/start/exec/put/get/cleanup, 与对话绑定并持久化
5. FS 工具对齐 LiveAgent: Read/Write/Edit/Delete/List/Grep/Glob + 版本校验 + 结果 details
6. Skills 闭环: 扫描/选择/元数据注入/ReadSkill progressive disclosure
7. MCP 闭环: server CRUD + selected + transport + safe tool naming + 调用成功
8. Hooks 闭环: lifecycle hooks 可配置可执行, warning 不影响主流程
9. Cron 闭环: bash/http/prompt 任务 + logs + pending/expired/completion 状态机
10. Context Compaction 闭环: 长对话稳定, checkpoint 可恢复, 无明显信息丢失
11. Agent Dev 闭环: debug jsonl 可用且不泄露 secrets
12. 测试通过: `pnpm test` + 关键 e2e + `cargo test --workspace`
13. specs 更新完成并可通过一致性自检
