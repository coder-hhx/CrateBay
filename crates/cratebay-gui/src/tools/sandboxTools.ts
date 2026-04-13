/**
 * Sandbox tools for the CrateBay Agent.
 *
 * High-level sandbox operations that abstract away container management.
 * These are the primary tools AI agents should use for code execution.
 */

import { Type } from "@sinclair/typebox";
import type { AgentTool, AgentToolResult } from "@mariozechner/pi-agent-core";
import { invoke } from "@/lib/tauri";

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

const SandboxRunCodeParams = Type.Object({
  language: Type.Union(
    [
      Type.Literal("python"),
      Type.Literal("javascript"),
      Type.Literal("bash"),
      Type.Literal("rust"),
    ],
    { description: "Programming language to use" },
  ),
  code: Type.String({ description: "Source code to execute" }),
  timeout_seconds: Type.Optional(
    Type.Number({
      description: "Execution timeout in seconds (default: 60)",
      minimum: 1,
      maximum: 3600,
    }),
  ),
  sandbox_id: Type.Optional(
    Type.String({
      description: "Reuse an existing sandbox instead of creating a new one",
    }),
  ),
});

const SandboxInstallParams = Type.Object({
  sandbox_id: Type.String({ description: "Target sandbox ID" }),
  package_manager: Type.Union(
    [
      Type.Literal("pip"),
      Type.Literal("npm"),
      Type.Literal("cargo"),
      Type.Literal("apt"),
    ],
    { description: "Package manager to use" },
  ),
  packages: Type.Array(Type.String(), {
    description: "Package names to install",
    minItems: 1,
  }),
});

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

interface RunCodeResult {
  sandbox_id: string;
  exit_code: number;
  stdout: string;
  stderr: string;
  duration_ms: number;
  language: string;
}

interface InstallResult {
  sandbox_id: string;
  package_manager: string;
  exit_code: number;
  stdout: string;
  stderr: string;
  duration_ms: number;
}

// ---------------------------------------------------------------------------
// Tool helpers
// ---------------------------------------------------------------------------

function textResult(text: string): AgentToolResult<undefined> {
  return {
    content: [{ type: "text", text }],
    details: undefined,
  };
}

function formatRunCodeResult(result: RunCodeResult): string {
  let output = "";

  if (result.stdout) {
    output += result.stdout;
  }
  if (result.stderr) {
    if (output) output += "\n";
    output += `[stderr] ${result.stderr}`;
  }

  const header = `Language: ${result.language} | Exit: ${result.exit_code} | ${result.duration_ms}ms | Sandbox: ${result.sandbox_id}`;

  if (!output) {
    return `${header}\n(no output)`;
  }
  return `${header}\n\n${output}`;
}

function formatInstallResult(result: InstallResult): string {
  let output = "";
  if (result.stdout) output += result.stdout;
  if (result.stderr) {
    if (output) output += "\n";
    output += result.stderr;
  }

  const header = `${result.package_manager} | Exit: ${result.exit_code} | ${result.duration_ms}ms`;
  return output ? `${header}\n\n${output}` : `${header}\n(no output)`;
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

const sandboxRunCodeTool: AgentTool<typeof SandboxRunCodeParams> = {
  name: "sandbox_run_code",
  label: "Run Code",
  description:
    "Execute code in an isolated sandbox. Automatically creates a sandbox if needed. " +
    "Supports Python, JavaScript, Bash, and Rust. Returns stdout, stderr, and exit code.",
  parameters: SandboxRunCodeParams,
  execute: async (_toolCallId, params) => {
    try {
      const result = await invoke<RunCodeResult>("sandbox_run_code", {
        language: params.language,
        code: params.code,
        sandbox_id: params.sandbox_id ?? null,
        timeout_seconds: params.timeout_seconds ?? null,
      });
      return textResult(formatRunCodeResult(result));
    } catch (error) {
      return textResult(`Error: ${error}`);
    }
  },
};

const sandboxInstallTool: AgentTool<typeof SandboxInstallParams> = {
  name: "sandbox_install",
  label: "Install Packages",
  description:
    "Install packages in an existing sandbox using pip, npm, cargo, or apt. " +
    "The sandbox must be running. Use sandbox_run_code first to create a sandbox with cleanup=false.",
  parameters: SandboxInstallParams,
  execute: async (_toolCallId, params) => {
    try {
      const result = await invoke<InstallResult>("sandbox_install", {
        sandbox_id: params.sandbox_id,
        package_manager: params.package_manager,
        packages: params.packages,
      });
      return textResult(formatInstallResult(result));
    } catch (error) {
      return textResult(`Error: ${error}`);
    }
  },
};

/**
 * All sandbox tools.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export const sandboxTools: AgentTool<any>[] = [
  sandboxRunCodeTool,
  sandboxInstallTool,
];
