import { test, expect } from "@playwright/test";
import { AppLayoutPage } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Volumes Management", () => {
  let appLayout: AppLayoutPage;

  test.beforeEach(async ({ page }) => {
    appLayout = new AppLayoutPage(page);

    await installTauriMock(page, {
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
    await expect(page.locator('[data-testid="volumes-page"]')).toBeVisible();
  });

  test("shows existing volumes", async ({ page }) => {
    await expect(page.getByText("workspace-cache", { exact: true })).toBeVisible();
    await expect(page.getByText(/cratebay-engine\/volumes\/workspace-cache/)).toBeVisible();
  });

  test("can create, inspect, and delete a volume", async ({ page }) => {
    await page.getByPlaceholder("volume-name").fill("build-cache");
    await page.getByPlaceholder("driver").fill("nfs");
    await page.getByRole("button", { name: "Create" }).click();

    await expect(page.getByText("build-cache", { exact: true })).toBeVisible();
    await expect(page.getByText("nfs", { exact: true })).toBeVisible();

    const row = page.locator('[data-testid="volumes-page"]').locator("div").filter({ hasText: /^build-cache/ }).first();
    await row.locator('[title="Inspect Volume"]').click();
    await expect(page.getByRole("heading", { name: "Inspect Volume" })).toBeVisible();
    await expect(page.getByRole("dialog").getByText("cratebay-engine", { exact: true })).toBeVisible();
    await page.getByRole("dialog").getByRole("button", { name: "Close" }).first().click();

    await row.locator('[title="Delete Volume"]').click();
    await page.getByRole("dialog").getByText("Force removal").click();
    await page.getByRole("dialog").getByRole("button", { name: "Delete" }).click();

    await expect(page.getByText("build-cache")).toBeHidden();
    await expect
      .poll(async () =>
        page.evaluate(() => {
          const commands = (window as any).__MOCK_TAURI__.invokedCommands;
          return commands
            .filter((item: any) => item.command === "volume_delete")
            .at(-1)?.args ?? null;
        }),
      )
      .toEqual({ name: "build-cache", force: true });
  });
});
