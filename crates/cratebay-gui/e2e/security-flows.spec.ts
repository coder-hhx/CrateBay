import { expect, test } from "@playwright/test"
import { gotoApp } from "./support/tauri"

test("surfaces skill schema validation errors to the user", async ({ page }) => {
  await gotoApp(page)

  await page.getByTestId("nav-settings").click()
  await page.getByRole("tab", { name: "AI" }).click()

  const sandboxSkill = page.getByTestId("skill-card-managed-sandbox-command")
  await sandboxSkill
    .getByTestId("skill-input-managed-sandbox-command")
    .fill('{"id":"sandbox-1","command":"echo hello","extra":true}')
  await sandboxSkill.getByRole("button", { name: "Run Skill" }).click()
  await expect(sandboxSkill).toContainText("input.extra is not allowed")
})

test("blocks assistant execution when confirmation is denied", async ({ page }) => {
  await gotoApp(page, { confirmResult: false })

  await page.getByTestId("nav-ai").click()
  await page.getByRole("tab", { name: "Assistant" }).click()
  await page.getByPlaceholder("Describe what you want to do...").fill("stop web")
  await page.getByRole("button", { name: "Generate Plan" }).click()
  await expect(page.getByText("Stop container")).toBeVisible()
  await page.getByRole("button", { name: "Run Step" }).click()

  await expect(page.getByTestId("assistant-step-result-step-1")).not.toBeVisible()
  const calls = await page.evaluate(() => (window as typeof window & { __CRATEBAY_E2E__: { getInvokeCalls(): Array<{ cmd: string }> } }).__CRATEBAY_E2E__.getInvokeCalls())
  expect(calls.some((call) => call.cmd === "assistant_execute_step")).toBe(false)
})
