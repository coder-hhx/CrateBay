import { test, expect } from "@playwright/test";
import { AppLayoutPage } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Networks Management", () => {
  let appLayout: AppLayoutPage;

  test.beforeEach(async ({ page }) => {
    appLayout = new AppLayoutPage(page);

    await installTauriMock(page, {
      networks: [
        {
          id: "net-workspace",
          name: "workspace-net",
          driver: "bridge",
          scope: "local",
          internal: false,
          attachable: true,
          labels: { "com.cratebay.network": "true" },
          containers: {
            abc123: {
              name: "node-01",
              endpointId: "endpoint-abc123",
              ipv4Address: "172.18.0.2/16",
            },
          },
          managedBy: "cratebay-engine",
        },
      ],
    });
    await appLayout.goto("/");
    await appLayout.verifyAppLoaded();
    await appLayout.navigateToNetworks();
    await expect(page.locator('[data-testid="networks-page"]')).toBeVisible();
  });

  test("shows existing networks", async ({ page }) => {
    await expect(page.getByText("workspace-net", { exact: true })).toBeVisible();
    await expect(page.getByText("bridge", { exact: true })).toBeVisible();
    await expect(page.getByText("Attachable", { exact: true })).toBeVisible();
  });

  test("can create, inspect, and delete a network", async ({ page }) => {
    await page.getByPlaceholder("network-name").fill("sandbox-net");
    await page.getByPlaceholder("driver").fill("macvlan");
    await page.getByText("Internal").click();
    await page.getByText("IPv6").click();
    await page.getByRole("button", { name: "Create" }).click();

    await expect(page.getByText("sandbox-net", { exact: true })).toBeVisible();
    await expect(page.getByText("macvlan", { exact: true })).toBeVisible();

    const row = page
      .locator('[data-testid="networks-page"]')
      .locator("div")
      .filter({ hasText: /^sandbox-net/ })
      .first();
    await expect(row.getByText("Internal", { exact: true })).toBeVisible();
    await row.locator('[title="Inspect Network"]').click();
    await expect(page.getByRole("heading", { name: "Inspect Network" })).toBeVisible();
    await expect(page.getByRole("dialog").getByText("cratebay-engine", { exact: true })).toBeVisible();
    await page.getByRole("dialog").getByRole("button", { name: "Close" }).first().click();

    await row.locator('[title="Delete Network"]').click();
    await page.getByRole("dialog").getByText("Detach containers and delete").click();
    await page.getByRole("dialog").getByRole("button", { name: "Delete" }).click();

    await expect(page.getByText("sandbox-net")).toBeHidden();
    await expect
      .poll(async () =>
        page.evaluate(() => {
          const commands = (window as any).__MOCK_TAURI__.invokedCommands;
          return commands
            .filter((item: any) => item.command === "network_delete")
            .at(-1)?.args ?? null;
        }),
      )
      .toEqual({ id: "net-sandbox-net", force: true });
  });
});
