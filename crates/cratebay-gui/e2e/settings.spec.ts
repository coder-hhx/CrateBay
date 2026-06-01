import { test, expect } from "@playwright/test";
import { SettingsPageObject } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Settings", () => {
  let settingsPage: SettingsPageObject;

  test.beforeEach(async ({ page }) => {
    settingsPage = new SettingsPageObject(page);

    await installTauriMock(page);
    await settingsPage.goto("/");
    await settingsPage.verifyAppLoaded();
    await settingsPage.navigateToSettings();
    await settingsPage.verifySettingsLoaded();
  });

  test("shows the supported settings tabs", async ({ page }) => {
    await expect(page.locator('[data-testid="settings-tab-general"]')).toBeVisible();
    await expect(page.locator('[data-testid="settings-tab-runtime"]')).toBeVisible();
    await expect(page.locator('[data-testid="settings-tab-about"]')).toBeVisible();
  });

  test("can switch to Runtime settings", async ({ page }) => {
    await page.locator('[data-testid="settings-tab-runtime"]').click();

    await expect(page.getByText("Runtime Control")).toBeVisible();
    await expect(page.getByText("Registry Mirrors")).toBeVisible();
  });

  test("shows runtime diagnostics", async ({ page }) => {
    await page.locator('[data-testid="settings-tab-runtime"]').click();

    const diagnostics = page.locator('[data-testid="runtime-diagnostics"]');
    await expect(diagnostics).toContainText("Runtime Diagnostics");
    await expect(diagnostics).toContainText("Docker Endpoint");
    await expect(diagnostics).toContainText("25.0.0");
    await expect(diagnostics).toContainText("1.44");
    await expect(diagnostics).toContainText("/tmp/docker.sock");
    await expect(diagnostics).toContainText("macos-vz");
  });

  test("can edit the Runtime HTTP proxy input", async ({ page }) => {
    await page.locator('[data-testid="settings-tab-runtime"]').click();

    const proxyInput = page.locator('input[placeholder="127.0.0.1:7890"]');
    await proxyInput.fill("127.0.0.1:7890");

    await expect(proxyInput).toHaveValue("127.0.0.1:7890");
  });

  test("can return to Images without losing the app shell", async ({ page }) => {
    await settingsPage.navigateToImages();

    await expect(page.locator('[data-testid="images-page"]')).toBeVisible();
    await expect(page.locator('[data-testid="app-title"]')).toBeVisible();
  });
});
