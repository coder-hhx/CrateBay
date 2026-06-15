import { test, expect } from "@playwright/test";
import { AppLayoutPage } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Resource Pages Runtime Control", () => {
  test("can start the Engine from the offline containers page", async ({ page }) => {
    const appLayout = new AppLayoutPage(page);
    await installTauriMock(page, {
      runtimeAutoStartDisabledCommands: ["container_list"],
      containerList: [
        {
          id: "abc123",
          shortId: "abc123",
          name: "node-01",
          status: "running",
          state: "running",
          image: "node:20-alpine",
          templateId: "node-dev",
          cpuCores: 2,
          memoryMb: 2048,
          ports: [],
          createdAt: "2026-03-23T00:00:00.000Z",
          labels: {},
        },
      ],
    });

    await appLayout.goto("/");
    await appLayout.verifyAppLoaded();
    await appLayout.navigateToContainers();

    await expect(page.getByText("Engine is offline")).toBeVisible();
    await expect(page.getByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).toBeHidden();
    await page.getByRole("button", { name: "Start Engine" }).click();

    await expect(page.getByText("node-01", { exact: true })).toBeVisible();
    await expect(page.getByText("Engine is offline")).toBeHidden();
  });

  test("can start the Engine from the offline volumes page", async ({ page }) => {
    const appLayout = new AppLayoutPage(page);
    await installTauriMock(page, {
      runtimeAutoStartDisabledCommands: ["volume_list"],
      volumes: [
        {
          name: "workspace-cache",
          driver: "local",
          mountpoint: "/var/lib/cratebay-engine/volumes/workspace-cache/_data",
          createdAt: "2026-03-23T00:00:00.000Z",
          scope: "local",
          labels: { "com.cratebay.volume": "true" },
          options: {},
          managedBy: "cratebay-engine",
        },
      ],
    });

    await appLayout.goto("/");
    await appLayout.verifyAppLoaded();
    await appLayout.navigateToVolumes();

    await expect(page.getByText("Engine is offline")).toBeVisible();
    await expect(page.getByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).toBeHidden();
    await page.getByRole("button", { name: "Start Engine" }).click();

    await expect(page.getByText("workspace-cache", { exact: true })).toBeVisible();
    await expect(page.getByText("Engine is offline")).toBeHidden();
  });

  test("can start the Engine from the offline networks page", async ({ page }) => {
    const appLayout = new AppLayoutPage(page);
    await installTauriMock(page, {
      runtimeAutoStartDisabledCommands: ["network_list"],
      networks: [
        {
          id: "net-workspace",
          name: "workspace-net",
          driver: "bridge",
          scope: "local",
          internal: false,
          attachable: true,
          labels: { "com.cratebay.network": "true" },
          containers: {},
          managedBy: "cratebay-engine",
        },
      ],
    });

    await appLayout.goto("/");
    await appLayout.verifyAppLoaded();
    await appLayout.navigateToNetworks();

    await expect(page.getByText("Engine is offline")).toBeVisible();
    await expect(page.getByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).toBeHidden();
    await page.getByRole("button", { name: "Start Engine" }).click();

    await expect(page.getByText("workspace-net", { exact: true })).toBeVisible();
    await expect(page.getByText("Engine is offline")).toBeHidden();
  });

  test("can start the Engine from the offline pods page", async ({ page }) => {
    const appLayout = new AppLayoutPage(page);
    await installTauriMock(page, {
      runtimeAutoStartDisabledCommands: ["pod_list"],
      pods: [
        {
          id: "pod123",
          name: "web-stack",
          driver: "bridge",
          createdAt: "2026-03-23T00:00:00.000Z",
          containers: [],
        },
      ],
    });

    await appLayout.goto("/");
    await appLayout.verifyAppLoaded();
    await appLayout.navigateToPods();

    await expect(page.getByText("Engine is offline")).toBeVisible();
    await expect(page.getByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).toBeHidden();
    await page.getByRole("button", { name: "Start Engine" }).click();

    await expect(page.getByText("web-stack", { exact: true })).toBeVisible();
    await expect(page.getByText("Engine is offline")).toBeHidden();
  });
});
