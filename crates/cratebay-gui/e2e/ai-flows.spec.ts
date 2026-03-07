import { expect, test } from "@playwright/test"
import { gotoApp } from "./support/tauri"

test("runs AI Hub models, sandboxes, and MCP flows", async ({ page }) => {
  await gotoApp(page)

  await page.getByTestId("nav-ai").click()
  await page.getByRole("tab", { name: "Models" }).click()
  await expect(page.getByText("/Users/test/.ollama/models")).toBeVisible()
  await page.getByPlaceholder("qwen2.5:7b").fill("llama3.2:3b")
  await page.getByRole("button", { name: "Pull" }).click()
  await expect(page.getByText("llama3.2:3b")).toBeVisible()
  const modelRow = page.locator("tr").filter({ has: page.getByText("llama3.2:3b") })
  await modelRow.getByRole("button", { name: "Delete" }).click()
  await expect(modelRow).toHaveCount(0)

  await page.getByRole("tab", { name: "Sandboxes" }).click()
  await page.locator('button[role="combobox"]').first().click()
  await page.getByRole("option", { name: "Node Dev" }).click()
  await page.getByPlaceholder("cbx-node-dev-...").fill("e2e-node-box")
  await page.getByPlaceholder("Default is current user").fill("tester")
  await page.getByRole("button", { name: "Create Sandbox" }).click()
  await expect(page.getByRole("cell", { name: "e2e-node-box", exact: true })).toBeVisible()
  await page.getByPlaceholder("echo hello from sandbox").fill("node -v")
  await page.getByRole("button", { name: "Run in sandbox" }).click()
  await expect(page.getByText(/node -v/)).toBeVisible()

  await page.getByRole("tab", { name: "MCP" }).click()
  const mcpRow = page.locator("tr").filter({ has: page.getByText("Filesystem MCP") })
  await expect(mcpRow).toBeVisible()
  await mcpRow.getByRole("button", { name: "Start" }).click()
  await expect(mcpRow).toContainText("Running")
  await page.getByRole("button", { name: "Export config" }).click()
  await expect(page.locator("textarea[readonly]")).toContainText("local-mcp-1")
})

test("runs settings and assistant AI flows end-to-end", async ({ page }) => {
  await gotoApp(page)

  await page.getByTestId("nav-settings").click()
  await page.getByRole("tab", { name: "AI" }).click()
  await page.getByRole("button", { name: "Test Connection" }).click()
  await expect(page.getByText("Connection succeeded")).toBeVisible()

  const codexSkill = page.getByTestId("skill-card-agent-cli-codex-prompt")
  await codexSkill.getByTestId("skill-input-agent-cli-codex-prompt").fill("summarize repo")
  await codexSkill.getByRole("button", { name: "Run Skill" }).click()
  await expect(codexSkill.getByTestId("skill-result-agent-cli-codex-prompt")).toContainText("codex exec summarize repo")

  await page.getByTestId("nav-ai").click()
  await page.getByRole("tab", { name: "Assistant" }).click()

  const assistantSkill = page.getByTestId("assistant-skill-card-agent-cli-codex-prompt")
  await assistantSkill.getByTestId("assistant-skill-input-agent-cli-codex-prompt").fill("generate summary")
  await assistantSkill.getByRole("button", { name: "Run Skill" }).click()
  await expect(page.getByTestId("assistant-skill-result-agent-cli-codex-prompt")).toContainText("codex exec generate summary")

  await page.getByPlaceholder("Describe what you want to do...").fill("stop web")
  await page.getByRole("button", { name: "Generate Plan" }).click()
  await expect(page.getByText("Stop container")).toBeVisible()
  await page.getByRole("button", { name: "Run Step" }).click()
  await expect(page.getByTestId("assistant-step-result-step-1")).toContainText("request_id=e2e-")
})
