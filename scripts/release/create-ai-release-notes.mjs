#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { copyFileSync, readFileSync, writeFileSync } from "node:fs";
import { parseReleaseVersion } from "./release-version.mjs";

const DEFAULT_BASE_URL = "https://api.openai.com/v1";
const DEFAULT_MODEL = "gpt-4.1-mini";
const MAX_CONTEXT_CHARS = 22000;

const [releaseTagArg, outputPath, fallbackNotesPath] = process.argv.slice(2);

function usage() {
  return "Usage: create-ai-release-notes.mjs <release-tag> <output-path> [fallback-notes-file]";
}

function fail(message, code = 1) {
  console.error(message);
  process.exit(code);
}

if (!releaseTagArg || !outputPath) {
  fail(usage());
}

let releaseVersion;
try {
  releaseVersion = parseReleaseVersion(releaseTagArg);
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}

function runGit(args, options = {}) {
  const result = spawnSync("git", args, {
    cwd: options.cwd ?? process.cwd(),
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  if (result.status !== 0) {
    if (options.optional) return "";
    throw new Error(`git ${args.join(" ")} failed: ${result.stderr.trim()}`);
  }
  return result.stdout.trim();
}

function compact(value, maxChars = MAX_CONTEXT_CHARS) {
  if (value.length <= maxChars) return value;
  return `${value.slice(0, maxChars)}\n\n[truncated]`;
}

function fallbackNotes() {
  if (fallbackNotesPath) {
    try {
      const fallback = readFileSync(fallbackNotesPath, "utf8").trim();
      if (fallback) return fallback;
    } catch {
      // Fall through to deterministic notes.
    }
  }
  return `# CrateBay ${releaseVersion.releaseTag}\n\nRelease ${releaseVersion.releaseTag}.`;
}

function writeFallback(reason) {
  console.warn(`AI release notes unavailable: ${reason}`);
  if (fallbackNotesPath) {
    try {
      copyFileSync(fallbackNotesPath, outputPath);
      console.log(`Wrote fallback release notes: ${outputPath}`);
      return;
    } catch {
      // Fall through to generated fallback notes.
    }
  }
  writeFileSync(outputPath, `${fallbackNotes().trim()}\n`);
  console.log(`Wrote fallback release notes: ${outputPath}`);
}

function stripCodeFence(markdown) {
  const trimmed = markdown.trim();
  const match = trimmed.match(/^```(?:markdown|md)?\s*\n([\s\S]*?)\n```$/i);
  return match ? match[1].trim() : trimmed;
}

function normalizeMarkdown(markdown) {
  let output = stripCodeFence(markdown);
  if (!output) return "";
  if (!output.startsWith("#")) {
    output = `# CrateBay ${releaseVersion.releaseTag}\n\n${output}`;
  }
  return `${output.trim()}\n`;
}

function previousTagFor(releaseCommit) {
  return runGit(["describe", "--tags", "--abbrev=0", `${releaseCommit}^`], {
    optional: true,
  });
}

function collectContext() {
  const releaseCommit = runGit(["rev-list", "-n", "1", releaseVersion.releaseTag]);
  const previousTag = previousTagFor(releaseCommit);
  const range = previousTag ? `${previousTag}..${releaseCommit}` : releaseCommit;
  const repository = process.env.GITHUB_REPOSITORY?.trim() || "nicepkg/CrateBay";

  const commitLog = runGit([
    "log",
    "--date=short",
    "--format=%h%x09%ad%x09%an%x09%s",
    range,
  ]);
  const diffStat = previousTag
    ? runGit(["diff", "--stat", previousTag, releaseCommit], { optional: true })
    : runGit(["show", "--stat", "--oneline", "--no-renames", releaseCommit], { optional: true });
  const changedFiles = previousTag
    ? runGit(["diff", "--name-status", previousTag, releaseCommit], { optional: true })
    : runGit(["show", "--name-status", "--format=", releaseCommit], { optional: true });
  const githubNotes = fallbackNotesPath ? readFileSync(fallbackNotesPath, "utf8").trim() : "";

  return {
    appVersion: releaseVersion.appVersion,
    changedFiles: compact(changedFiles, 7000),
    commitLog: compact(commitLog, 10000),
    diffStat: compact(diffStat, 7000),
    githubNotes: compact(githubNotes, 8000),
    previousTag,
    range,
    releaseTag: releaseVersion.releaseTag,
    repository,
  };
}

function buildPrompt(context) {
  return [
    `Repository: ${context.repository}`,
    `Release tag: ${context.releaseTag}`,
    `App version: ${context.appVersion}`,
    `Previous tag: ${context.previousTag || "none"}`,
    `Commit range: ${context.range}`,
    "",
    "Write polished GitHub release notes in Markdown for this CrateBay release.",
    "",
    "Rules:",
    "- Output Markdown only.",
    "- Do not invent features, fixes, compatibility claims, metrics, dates, or contributors.",
    "- Use only the provided GitHub notes, commit log, diff stat, and changed files.",
    "- Write for users first, developers second.",
    `- Start with exactly this H1: # CrateBay ${context.releaseTag}`,
    "- Add a one-sentence blockquote summary after the H1.",
    "- Use concise sections such as Overview, Highlights, Added, Changed, Fixed, Internal.",
    "- Omit a section if there is no evidence for it.",
    "",
    "GitHub generated notes:",
    "```markdown",
    context.githubNotes || "(none)",
    "```",
    "",
    "Commit log:",
    "```text",
    context.commitLog || "(none)",
    "```",
    "",
    "Diff stat:",
    "```text",
    context.diffStat || "(none)",
    "```",
    "",
    "Changed files:",
    "```text",
    context.changedFiles || "(none)",
    "```",
  ].join("\n");
}

function responseText(payload) {
  if (typeof payload.output_text === "string") return payload.output_text;

  const output = payload.output;
  if (Array.isArray(output)) {
    const parts = [];
    for (const item of output) {
      if (!Array.isArray(item.content)) continue;
      for (const content of item.content) {
        if (typeof content.text === "string") parts.push(content.text);
      }
    }
    if (parts.length > 0) return parts.join("\n");
  }

  const choice = payload.choices?.[0]?.message?.content;
  if (typeof choice === "string") return choice;
  if (Array.isArray(choice)) {
    return choice
      .map((part) => (typeof part.text === "string" ? part.text : ""))
      .filter(Boolean)
      .join("\n");
  }

  return "";
}

async function fetchJsonWithTimeout(endpoint, { apiKey, body, timeoutMs }) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  const response = await fetch(endpoint, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    signal: controller.signal,
    body: JSON.stringify(body),
  }).finally(() => clearTimeout(timeout));

  const text = await response.text();
  if (!response.ok) {
    throw new Error(`API returned HTTP ${response.status}: ${text.slice(0, 500)}`);
  }
  return JSON.parse(text);
}

async function createNotes() {
  const apiKey = process.env.CRATEBAY_RELEASE_NOTES_API_KEY || process.env.OPENAI_API_KEY;
  if (!apiKey) {
    writeFallback("CRATEBAY_RELEASE_NOTES_API_KEY or OPENAI_API_KEY is not configured");
    return;
  }

  const baseUrl = process.env.CRATEBAY_RELEASE_NOTES_BASE_URL || DEFAULT_BASE_URL;
  const model = process.env.CRATEBAY_RELEASE_NOTES_MODEL || DEFAULT_MODEL;
  const timeoutMs = Number(process.env.CRATEBAY_RELEASE_NOTES_TIMEOUT_MS || "60000");
  const prompt = buildPrompt(collectContext());
  const endpoint = `${baseUrl.replace(/\/+$/, "")}/responses`;
  const body = {
    input: [
      {
        role: "system",
        content: [
          {
            type: "input_text",
            text: "You are a precise release-notes editor. You only make claims grounded in the provided repository context.",
          },
        ],
      },
      {
        role: "user",
        content: [{ type: "input_text", text: prompt }],
      },
    ],
    max_output_tokens: 3000,
    model,
    store: false,
  };

  const reasoningEffort = (process.env.CRATEBAY_RELEASE_NOTES_REASONING || "").trim();
  if (reasoningEffort) {
    body.reasoning = { effort: reasoningEffort };
  }

  const payload = await fetchJsonWithTimeout(endpoint, { apiKey, body, timeoutMs });
  const markdown = normalizeMarkdown(responseText(payload));
  if (!markdown) {
    throw new Error("AI response did not contain release notes text");
  }
  writeFileSync(outputPath, markdown);
  console.log(`Wrote AI release notes: ${outputPath}`);
}

try {
  await createNotes();
} catch (error) {
  writeFallback(error instanceof Error ? error.message : String(error));
}
