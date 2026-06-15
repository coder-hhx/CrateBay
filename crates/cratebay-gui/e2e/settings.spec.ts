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

  test("shows the supported settings sections", async ({ page }) => {
    await expect(page.locator(settingsPage.generalSection)).toBeVisible();
    await expect(page.locator(settingsPage.updatesSection)).toBeVisible();
    await expect(page.locator(settingsPage.aboutSection)).toBeVisible();
    await expect(page.locator(settingsPage.runtimeSection)).toBeVisible();
  });

  test("shows Runtime settings", async ({ page }) => {
    const runtimeSection = page.locator(settingsPage.runtimeSection);
    await expect(runtimeSection).toContainText("Engine VM Control");
    await expect(runtimeSection).toContainText("Registry Mirrors");
  });

  test("shows runtime diagnostics", async ({ page }) => {
    const diagnostics = page.locator(settingsPage.runtimeDiagnostics);
    await expect(diagnostics).toContainText("Runtime Diagnostics");
    await expect(diagnostics).toContainText("Engine Endpoint");
    await expect(diagnostics).toContainText("cratebay-containerd");
    await expect(diagnostics).toContainText("cratebay.engine.v1");
    await expect(diagnostics).toContainText("/tmp/cratebay/engine.sock");
    await expect(diagnostics).toContainText("macos-vz");
    await expect(diagnostics).toContainText("CPU usage");
    await expect(diagnostics).toContainText("18.5%");
    await expect(diagnostics).toContainText("Memory usage");
    await expect(diagnostics).toContainText("768 / 2048 MB");
    await expect(diagnostics).toContainText("Disk usage");
    await expect(diagnostics).toContainText("6.5 / 20 GB");
    await expect(diagnostics).toContainText("Runtime containers");
  });

  test("shows runtime control operation feedback", async ({ page }) => {
    const runtimeSection = page.locator(settingsPage.runtimeSection);

    await runtimeSection.getByRole("button", { name: "Stop" }).click();
    await expect(page.locator('[data-testid="runtime-operation-result"]')).toContainText("Engine VM stopped");
    await expect(page.locator('[data-testid="runtime-operation-result"]')).toContainText("State: stopped");

    await runtimeSection.getByRole("button", { name: "Start", exact: true }).click();
    await expect(page.locator('[data-testid="runtime-operation-result"]')).toContainText("Engine VM started");
    await expect(page.locator('[data-testid="runtime-operation-result"]')).toContainText("State: ready");
    await expect(page.locator('[data-testid="runtime-operation-result"]')).toContainText("/tmp/cratebay/engine.sock");
  });

  test("shows native Engine maintenance actions", async ({ page }) => {
    const maintenance = page.locator('[data-testid="engine-maintenance"]');

    await expect(maintenance).toContainText("Engine Maintenance");
    await expect(maintenance).toContainText("Engine Contract");
    await expect(maintenance).toContainText("cratebay-containerd");
    await expect(maintenance).toContainText("cratebay.engine.v1");
    await expect(maintenance).toContainText("cratebay");
    await expect(maintenance).toContainText("Native Substrate");
    await expect(maintenance).toContainText("containerd task service");
    await expect(maintenance).toContainText("4.0 KB");
    await expect(maintenance).toContainText("node-01");

    await maintenance.getByRole("button", { name: "Apply GC" }).click();
    await expect(maintenance.locator('[data-testid="engine-maintenance-result"]')).toContainText("Storage GC complete");
    await expect(maintenance).toContainText("0 B");

    await maintenance.getByRole("button", { name: "Reap" }).click();
    await expect(maintenance.locator('[data-testid="engine-maintenance-result"]')).toContainText("Reaped shim task");
    await expect(maintenance.locator('[data-testid="engine-maintenance-result"]')).toContainText("Remaining tasks: 0");
    await expect(maintenance).toContainText("No shim tasks found.");
  });

  test("can edit the Runtime HTTP proxy input", async ({ page }) => {
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
