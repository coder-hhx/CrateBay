import { test, expect } from "@playwright/test";
import { AppLayoutPage, ImagesPageObject } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Navigation", () => {
  let appLayout: AppLayoutPage;
  let imagesPage: ImagesPageObject;

  test.beforeEach(async ({ page }) => {
    appLayout = new AppLayoutPage(page);
    imagesPage = new ImagesPageObject(page);

    await installTauriMock(page);
    await appLayout.goto("/");
    await appLayout.verifyAppLoaded();
  });

  test("app loads on the Dashboard page", async ({ page }) => {
    await expect(page.locator('[data-testid="dashboard-page"]')).toBeVisible();
    await expect(page.locator('[data-testid="app-title"]')).toBeVisible();
  });

  test("sidebar shows management navigation", async ({ page }) => {
    await expect(page.locator('[data-testid="nav-dashboard"]')).toBeVisible();
    await expect(page.locator('[data-testid="nav-images"]')).toBeVisible();
    await expect(page.locator('[data-testid="nav-containers"]')).toBeVisible();
    await expect(page.locator('[data-testid="nav-pods"]')).toBeVisible();
    await expect(page.locator('[data-testid="nav-volumes"]')).toBeVisible();
    await expect(page.locator('[data-testid="nav-networks"]')).toBeVisible();
    await expect(page.locator('[data-testid="nav-settings"]')).toBeVisible();
  });

  test("can navigate between Images, Containers, Pods, Volumes, Networks, and Settings", async ({ page }) => {
    await appLayout.navigateToContainers();
    await expect(page.locator('[data-testid="container-list"]')).toBeVisible();

    await appLayout.navigateToPods();
    await expect(page.locator('[data-testid="pods-page"]')).toBeVisible();

    await appLayout.navigateToVolumes();
    await expect(page.locator('[data-testid="volumes-page"]')).toBeVisible();

    await appLayout.navigateToNetworks();
    await expect(page.locator('[data-testid="networks-page"]')).toBeVisible();

    await appLayout.navigateToSettings();
    await expect(page.locator('[data-testid="settings-section-general"]')).toBeVisible();

    await appLayout.navigateToImages();
    await imagesPage.verifyImagesLoaded();
  });

  test("quick navigation remains stable", async ({ page }) => {
    await appLayout.navigateToContainers();
    await appLayout.navigateToImages();
    await appLayout.navigateToPods();
    await appLayout.navigateToVolumes();
    await appLayout.navigateToNetworks();
    await appLayout.navigateToSettings();
    await appLayout.navigateToImages();

    await imagesPage.verifyImagesLoaded();
    await expect(page.locator('[data-testid="nav-images"]')).toHaveAttribute("aria-current", "page");
  });
});
