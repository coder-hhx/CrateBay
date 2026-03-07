import { expect, test } from "@playwright/test"
import { gotoApp } from "./support/tauri"

test("navigates all top-level sections", async ({ page }) => {
  await gotoApp(page)

  await expect(page.getByTestId("nav-dashboard")).toBeVisible()
  await expect(page.getByRole("heading", { name: "Dashboard" })).toBeVisible()

  await page.getByTestId("nav-containers").click()
  await expect(page.getByTestId("containers-run")).toBeVisible()

  await page.getByTestId("nav-images").click()
  await expect(page.getByTestId("images-tab-local")).toBeVisible()

  await page.getByTestId("nav-volumes").click()
  await expect(page.getByTestId("volumes-create")).toBeVisible()

  await page.getByTestId("nav-vms").click()
  await expect(page.getByTestId("vms-create")).toBeVisible()

  await page.getByTestId("nav-kubernetes").click()
  await expect(page.getByTestId("k8s-tab-overview")).toBeVisible()

  await page.getByTestId("nav-ai").click()
  await expect(page.getByRole("tab", { name: "Models" })).toBeVisible()
  await expect(page.getByRole("tab", { name: "Assistant" })).toBeVisible()

  await page.getByTestId("nav-settings").click()
  await expect(page.getByRole("tab", { name: "General" })).toBeVisible()
  await expect(page.getByRole("tab", { name: "AI" })).toBeVisible()
})
