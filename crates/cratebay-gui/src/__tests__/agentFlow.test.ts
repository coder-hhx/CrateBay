/**
 * Agent Flow Tests — testing-spec §4.1
 *
 * Tests the CrateBay Agent engine using mock streamFn and mock Tauri invoke.
 * Verifies:
 * - Agent ↔ streamFn integration
 * - Tool call routing and execution
 * - Multi-turn conversation flow
 * - beforeToolCall confirmation flow (risk-level blocking)
 * - Context pruning (transformContext)
 * - Error handling
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";
import {
  createMockStreamFn,
  createMockStreamFnWithSpy,
  createMockModel,
} from "@/__mocks__/mockStreamFn";

// Mock Tauri before importing agent/tools
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { Agent, type AgentEvent, type AgentMessage } from "@mariozechner/pi-agent-core";
import type { Message } from "@mariozechner/pi-ai";
import { allTools, getToolRiskLevel } from "@/tools";
import { buildSystemPrompt } from "@/lib/systemPrompt";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const mockModel = createMockModel();

/**
 * defaultConvertToLlm — mirrors the implementation in agent.ts
 */
function defaultConvertToLlm(messages: AgentMessage[]): Message[] {
  return messages.filter(
    (m): m is Message =>
      typeof m === "object" &&
      m !== null &&
      "role" in m &&
      (m.role === "user" || m.role === "assistant" || m.role === "toolResult"),
  );
}

/**
 * Collect all events from an agent prompt run.
 */
function collectEvents(agent: Agent): AgentEvent[] {
  const events: AgentEvent[] = [];
  agent.subscribe((e) => events.push(e));
  return events;
}

// ---------------------------------------------------------------------------
// §4.1 — Agent with mock streamFn: basic text response
// ---------------------------------------------------------------------------
describe("Agent Flow — text-only response", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("agent responds with text when no tools are needed", async () => {
    const streamFn = createMockStreamFn([
      { content: "Hello! I can help you manage containers." },
    ]);

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: defaultConvertToLlm,
      toolExecution: "sequential",
    });

    const events = collectEvents(agent);

    await agent.prompt("Hello");

    // Should have agent_start, turn_start, message_start, message_update(s), message_end, turn_end, agent_end
    const agentStartEvents = events.filter((e) => e.type === "agent_start");
    const agentEndEvents = events.filter((e) => e.type === "agent_end");
    expect(agentStartEvents).toHaveLength(1);
    expect(agentEndEvents).toHaveLength(1);

    // Check final messages contain our text
    const endEvent = agentEndEvents[0];
    if (endEvent.type === "agent_end") {
      const assistantMsgs = endEvent.messages.filter(
        (m) => "role" in m && m.role === "assistant",
      );
      expect(assistantMsgs.length).toBeGreaterThanOrEqual(1);
    }
  });
});

// ---------------------------------------------------------------------------
// §4.1 — Agent tool call → Tauri invoke → result round-trip
// ---------------------------------------------------------------------------
describe("Agent Flow — tool call execution", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("agent executes container_list tool when streamFn returns a tool call", async () => {
    // Mock: first call returns tool call, second call returns text summary
    const streamFn = createMockStreamFn([
      {
        toolCalls: [{ name: "container_list", arguments: {} }],
      },
      {
        content: "You have 2 containers.",
      },
    ]);

    // Mock Tauri invoke for container_list
    mockInvoke.mockImplementation((command: string) => {
      if (command === "container_list") {
        return Promise.resolve([
          { short_id: "abc123", name: "node-01", image: "node:20", state: "running", cpu_cores: 2, memory_mb: 2048 },
          { short_id: "def456", name: "py-dev", image: "python:3.12", state: "stopped", cpu_cores: 1, memory_mb: 1024 },
        ]);
      }
      return Promise.reject(new Error(`Unknown command: ${command}`));
    });

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: defaultConvertToLlm,
      toolExecution: "sequential",
    });

    const events = collectEvents(agent);

    await agent.prompt("Show me my containers");

    // Should have tool execution events
    const toolStartEvents = events.filter((e) => e.type === "tool_execution_start");
    const toolEndEvents = events.filter((e) => e.type === "tool_execution_end");
    expect(toolStartEvents.length).toBeGreaterThanOrEqual(1);
    expect(toolEndEvents.length).toBeGreaterThanOrEqual(1);

    // Verify tool name
    if (toolStartEvents[0].type === "tool_execution_start") {
      expect(toolStartEvents[0].toolName).toBe("container_list");
    }

    // Verify invoke was called
    expect(mockInvoke).toHaveBeenCalledWith("container_list", { filters: undefined });
  });

  it("agent handles tool execution errors gracefully", async () => {
    const streamFn = createMockStreamFn([
      {
        toolCalls: [{ name: "container_list", arguments: {} }],
      },
      {
        content: "Sorry, I could not fetch the containers.",
      },
    ]);

    // Mock invoke to fail
    mockInvoke.mockRejectedValue(new Error("Docker not connected"));

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: defaultConvertToLlm,
      toolExecution: "sequential",
    });

    const events = collectEvents(agent);

    await agent.prompt("Show me my containers");

    // Should have tool_execution_end with isError=true
    const toolEndEvents = events.filter((e) => e.type === "tool_execution_end");
    expect(toolEndEvents.length).toBeGreaterThanOrEqual(1);

    if (toolEndEvents[0].type === "tool_execution_end") {
      expect(toolEndEvents[0].isError).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// §4.1 — beforeToolCall confirmation flow
// ---------------------------------------------------------------------------
describe("Agent Flow — beforeToolCall confirmation", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("blocks high-risk tool calls when confirmation is denied", async () => {
    const streamFn = createMockStreamFn([
      {
        toolCalls: [{ name: "container_delete", arguments: { id: "abc123" } }],
      },
      {
        content: "Operation was cancelled.",
      },
    ]);

    mockInvoke.mockResolvedValue(undefined);

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: defaultConvertToLlm,
      toolExecution: "sequential",
      beforeToolCall: async (context) => {
        const riskLevel = getToolRiskLevel(context.toolCall.name);
        if (riskLevel === "high" || riskLevel === "critical") {
          return {
            block: true,
            reason: `User cancelled ${context.toolCall.name} operation.`,
          };
        }
        return undefined;
      },
    });

    const events = collectEvents(agent);

    await agent.prompt("Delete container abc123");

    // container_delete invoke should NOT have been called
    const deleteInvocations = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "container_delete",
    );
    expect(deleteInvocations).toHaveLength(0);

    // Tool execution should end with isError=true
    const toolEndEvents = events.filter((e) => e.type === "tool_execution_end");
    expect(toolEndEvents.length).toBeGreaterThanOrEqual(1);
    if (toolEndEvents[0].type === "tool_execution_end") {
      expect(toolEndEvents[0].isError).toBe(true);
    }
  });

  it("allows low-risk tool calls without blocking", async () => {
    const streamFn = createMockStreamFn([
      {
        toolCalls: [{ name: "container_list", arguments: {} }],
      },
      {
        content: "Here are your containers.",
      },
    ]);

    mockInvoke.mockResolvedValue([]);

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: defaultConvertToLlm,
      toolExecution: "sequential",
      beforeToolCall: async (context) => {
        const riskLevel = getToolRiskLevel(context.toolCall.name);
        if (riskLevel === "high" || riskLevel === "critical") {
          return { block: true, reason: "Blocked by test" };
        }
        return undefined;
      },
    });

    await agent.prompt("List containers");

    // container_list invoke SHOULD have been called
    expect(mockInvoke).toHaveBeenCalledWith("container_list", { filters: undefined });
  });
});

// ---------------------------------------------------------------------------
// §4.1 — Multi-turn conversation (streamFn spy)
// ---------------------------------------------------------------------------
describe("Agent Flow — multi-turn conversation", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("agent accumulates messages across multiple prompts", async () => {
    const spy = createMockStreamFnWithSpy([
      { content: "Hello! What can I help you with?" },
      { content: "I can manage containers for you." },
    ]);

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn: spy.streamFn,
      convertToLlm: defaultConvertToLlm,
      toolExecution: "sequential",
    });

    await agent.prompt("Hello");
    await agent.prompt("What can you do?");

    // streamFn should have been called twice
    expect(spy.calls).toHaveLength(2);

    // Second call should have more messages in context than the first
    expect(spy.calls[1].context.messages.length).toBeGreaterThan(
      spy.calls[0].context.messages.length,
    );
  });
});

// ---------------------------------------------------------------------------
// Context transform
// ---------------------------------------------------------------------------
describe("Agent Flow — context transform", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("transformContext prunes old messages", async () => {
    const spy = createMockStreamFnWithSpy([
      { content: "Response 1" },
      { content: "Response 2" },
      { content: "Response 3" },
    ]);

    const MAX_MESSAGES = 2;

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: [],
        thinkingLevel: "off",
      },
      streamFn: spy.streamFn,
      convertToLlm: defaultConvertToLlm,
      transformContext: async (messages) => {
        if (messages.length > MAX_MESSAGES) {
          return messages.slice(-MAX_MESSAGES);
        }
        return messages;
      },
      toolExecution: "sequential",
    });

    await agent.prompt("Message 1");
    await agent.prompt("Message 2");
    await agent.prompt("Message 3");

    // The third call should have pruned context
    const lastCall = spy.calls[spy.calls.length - 1];
    // Context messages should be limited to MAX_MESSAGES
    expect(lastCall.context.messages.length).toBeLessThanOrEqual(MAX_MESSAGES);
  });
});

// ---------------------------------------------------------------------------
// System prompt generation
// ---------------------------------------------------------------------------
describe("buildSystemPrompt", () => {
  it("includes all tool names and descriptions", () => {
    const prompt = buildSystemPrompt(allTools);

    for (const tool of allTools) {
      expect(prompt).toContain(tool.name);
    }
  });

  it("contains behavioral rules section", () => {
    const prompt = buildSystemPrompt(allTools);

    expect(prompt).toContain("Behavioral Rules");
    expect(prompt).toContain("Error recovery");
    expect(prompt).toContain("Restrictions");
  });
});

// ---------------------------------------------------------------------------
// createUserMessage helper
// ---------------------------------------------------------------------------
describe("createUserMessage", () => {
  it("creates a valid user message with timestamp", async () => {
    const { createUserMessage } = await import("@/lib/agent");

    const before = Date.now();
    const msg = createUserMessage("Test message");
    const after = Date.now();

    expect(msg.role).toBe("user");
    expect(msg.content).toBe("Test message");
    expect(msg.timestamp).toBeGreaterThanOrEqual(before);
    expect(msg.timestamp).toBeLessThanOrEqual(after);
  });
});

// ---------------------------------------------------------------------------
// streamFn error handling
// ---------------------------------------------------------------------------
describe("Agent Flow — streamFn error response", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  it("agent handles streamFn error gracefully", async () => {
    const streamFn = createMockStreamFn([
      { error: "Rate limit exceeded" },
    ]);

    const agent = new Agent({
      initialState: {
        systemPrompt: "You are a test assistant.",
        model: mockModel,
        tools: allTools,
        thinkingLevel: "off",
      },
      streamFn,
      convertToLlm: defaultConvertToLlm,
      toolExecution: "sequential",
    });

    const events = collectEvents(agent);

    await agent.prompt("Hello");

    // Should have an agent_end event (agent loop completed)
    const endEvents = events.filter((e) => e.type === "agent_end");
    expect(endEvents.length).toBeGreaterThanOrEqual(1);
  });
});
