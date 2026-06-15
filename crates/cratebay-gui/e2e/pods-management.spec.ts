import { test, expect } from "@playwright/test";
import { PodsPageObject } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Pods Management", () => {
  let podsPage: PodsPageObject;

  test.beforeEach(async ({ page }) => {
    podsPage = new PodsPageObject(page);

    await installTauriMock(page, {
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
    await podsPage.goto("/");
    await podsPage.verifyAppLoaded();
    await podsPage.navigateToPods();
    await podsPage.verifyPodsLoaded();
  });

  test("shows existing pods", async ({ page }) => {
    await expect(page.getByText("web-stack")).toBeVisible();
    await expect(page.getByText("bridge")).toBeVisible();
  });

  test("can create a pod", async ({ page }) => {
    await page.getByPlaceholder("pod-name").fill("api-stack");
    await page.getByPlaceholder("driver").fill("macvlan");
    await page.getByText("Internal").click();
    await page.getByText("IPv6").click();
    await page.getByRole("button", { name: "Create" }).click();

    await expect(page.getByText("api-stack")).toBeVisible();
    await expect(page.getByText("macvlan")).toBeVisible();
  });

  test("can add, inspect, and remove a container from a pod", async ({ page }) => {
    await page.getByLabel("Select container").click();
    await page.locator('[data-slot="select-item"]').filter({ hasText: /node-01/ }).first().click();
    await page.getByRole("button", { name: "Add Container" }).click();

    await expect(page.getByText("node-01")).toBeVisible();

    await page.locator('[title="Inspect Pod"]').first().click();
    await expect(page.getByRole("heading", { name: "Inspect Pod" })).toBeVisible();
    await expect(page.getByRole("dialog").getByText("172.18.0.2/16")).toBeVisible();
    await page.getByRole("button", { name: "Close" }).first().click();

    await page.locator('[title="Remove Container"]').first().click();
    await expect(page.locator('[data-testid="pods-page"]').getByText("node-01", { exact: true })).toBeHidden();
  });

  test("can delete a pod", async ({ page }) => {
    await page.locator('[title="Delete Pod"]').first().click();
    await page.getByRole("checkbox", { name: "Disconnect containers and delete" }).click();
    await page.getByRole("button", { name: "Delete" }).click();

    await expect(page.getByText("web-stack")).toBeHidden();
  });
});
