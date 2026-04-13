/**
 * Agent Tool Registry.
 *
 * Central registry that exports all AgentTools for pi-agent-core.
 * Tools are organized by category (container, filesystem, shell, mcp, system)
 * and exported as a single flat array.
 */

import type { AgentTool } from "@mariozechner/pi-agent-core";
import { containerTools } from "./containerTools";
import { imageTools } from "./imageTools";
import { filesystemTools } from "./filesystemTools";
import { shellTools } from "./shellTools";
import { mcpTools } from "./mcpTools";
import { systemTools } from "./systemTools";
import { sandboxTools } from "./sandboxTools";
import type { RiskLevel } from "@/types/agent";

/**
 * All built-in agent tools.
 *
 * This is the static set of tools that are always available regardless
 * of MCP server connections. The useMcpToolSync hook merges these
 * with dynamically discovered MCP bridge tools.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const builtinTools: AgentTool<any>[] = [
  ...sandboxTools,
  ...containerTools,
  ...imageTools,
  ...filesystemTools,
  ...shellTools,
  ...mcpTools,
  ...systemTools,
];

/**
 * Alias for backward compatibility.
 * @deprecated Use builtinTools instead.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const allTools: AgentTool<any>[] = builtinTools;

/**
 * Static risk level assignments per tool.
 * Used by the confirmation flow in workflowStore.
 */
export const toolRiskLevels: Record<string, RiskLevel> = {
  // Container tools
  container_list: "low",
  container_inspect: "low",
  container_create: "medium",
  container_start: "low",
  container_stop: "medium",
  container_delete: "high",
  container_exec: "medium",
  container_logs: "low",
  image_list: "low",
  image_search: "low",
  image_pull: "medium",
  image_remove: "high",
  image_inspect: "low",
  image_tag: "medium",

  // Filesystem tools
  file_read: "low",
  file_write: "medium",
  file_list: "low",

  // Shell tools
  shell_exec: "medium",

  // MCP tools
  mcp_list_tools: "low",
  mcp_call_tool: "medium",

  // Sandbox tools
  sandbox_run_code: "low",
  sandbox_install: "medium",

  // System tools
  docker_status: "low",
  system_info: "low",
  runtime_status: "low",
};

/**
 * Destructive keyword patterns for dynamic risk detection.
 * Used primarily for MCP tool call risk assessment.
 */
const destructiveKeywords = [
  "delete",
  "remove",
  "destroy",
  "drop",
  "wipe",
  "prune",
  "terminate",
  "kill",
  "purge",
  "reset",
];

/**
 * Get the risk level for a tool, with dynamic detection for MCP tools.
 */
export function getToolRiskLevel(toolName: string): RiskLevel {
  // Check static registry first
  const staticLevel = toolRiskLevels[toolName];
  if (staticLevel !== undefined) return staticLevel;

  // Dynamic detection for unknown/MCP tools
  const lower = toolName.toLowerCase();
  if (destructiveKeywords.some((kw) => lower.includes(kw))) {
    return "high";
  }

  return "medium";
}

/**
 * Get the display label for a tool.
 */
export function getToolLabel(toolName: string): string {
  const tool = allTools.find((t) => t.name === toolName);
  return tool?.label ?? toolName;
}
