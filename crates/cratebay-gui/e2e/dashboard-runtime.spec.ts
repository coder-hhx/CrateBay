import { test, expect } from "@playwright/test";
import { AppLayoutPage } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Dashboard Runtime Control", () => {
  test("can start the built-in Engine from the offline dashboard", async ({ page }) => {
    const appLayout = new AppLayoutPage(page);

    await installTauriMock(page, {
      engineStatus: {
        connected: false,
        version: null,
        api_version: null,
        os: null,
        arch: null,
        engine_source: "builtin",
        source: "builtin",
        socket_path: null,
      },
      runtimeStatus: {
        state: "stopped",
        platform: "macos-vz",
        cpu_cores: 2,
        memory_mb: 2048,
        disk_gb: 20,
        engine_responsive: false,
        compatibility_responsive: false,
        docker_responsive: false,
        engine_source: "builtin",
        docker_source: "builtin",
        uptime_seconds: null,
        resource_usage: null,
      },
      containerList: [],
      localImages: [],
      pods: [],
      volumes: [],
      networks: [],
    });

    await appLayout.goto("/");
    await appLayout.verifyAppLoaded();

    await expect(page.locator('[data-testid="dashboard-page"]')).toBeVisible();
    await expect(page.getByText("Engine is offline")).toBeVisible();
    await expect(page.getByText("Offline", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Start Engine" }).click();

    await expect(page.getByText("Online", { exact: true })).toBeVisible();
    await expect(page.getByText("Engine is offline")).toBeHidden();
  });
});
