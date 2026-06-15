import { test, expect } from "@playwright/test";
import { ImagesPageObject } from "./pages";
import { installTauriMock } from "./tauri-mock";

test.describe("Images Management", () => {
  let imagesPage: ImagesPageObject;

  test.beforeEach(async ({ page }) => {
    imagesPage = new ImagesPageObject(page);

    await installTauriMock(page, {
      localImages: [
        {
          id: "sha256:node",
          repoTags: ["node:20-alpine"],
          sizeBytes: 120_000_000,
          sizeHuman: "120 MB",
          created: 1_700_000_000,
        },
      ],
    });
    await imagesPage.goto("/");
    await imagesPage.verifyAppLoaded();
    await imagesPage.navigateToImages();
    await imagesPage.verifyImagesLoaded();
  });

  test("can load bundled images into the local image list", async ({ page }) => {
    await page.getByRole("button", { name: "Load Bundled" }).click();

    await expect(page.getByText(/Loaded \d+, skipped \d+, failed 0 bundled image/)).toBeVisible();
    await expect(page.locator('[data-testid="image-operation-feedback"]')).toContainText("Loaded:");
    await expect(page.locator('[data-testid="image-operation-feedback"]')).toContainText("Completed:");
    await expect(page.getByText("cratebay-python-dev:v1")).toBeVisible();
  });

  test("can search registry images and pull one into the local list", async ({ page }) => {
    await page.locator(imagesPage.imagesSearchTab).click();
    await page.locator(imagesPage.imageSearchInput).fill("alpine");
    await page.locator(imagesPage.imageSearchSubmit).click();

    const resultCard = page.locator(imagesPage.imageSearchResult).first();
    await expect(resultCard.getByText("alpine:latest")).toBeVisible();

    await resultCard.locator(imagesPage.imageSearchPull).click();
    await expect(page.getByText("Done", { exact: true })).toBeVisible();

    await page.locator(imagesPage.imagesLocalTab).click();
    await expect(page.getByText("alpine:latest", { exact: true })).toBeVisible();
  });

  test("can tag a local image", async ({ page }) => {
    await page.locator('[title="Tag Image"]').first().click();
    await page.getByPlaceholder("repo/name:tag").fill("cratebay/node:test");
    await page.getByRole("button", { name: "Tag Image" }).last().click();

    await expect(page.getByText("Tagged node:20-alpine as cratebay/node:test.")).toBeVisible();
    await expect(page.getByText("node:20-alpine", { exact: true })).toBeVisible();
  });

  test("can export selected images", async ({ page }) => {
    await page.locator('[data-slot="checkbox"]').first().click();
    await page.getByRole("button", { name: /^Export$/ }).click();
    await page.getByRole("button", { name: /^Export$/ }).last().click();

    await expect(page.getByText("Exported 1 image(s), 4096 bytes written.")).toBeVisible();
    await expect(page.locator('[data-testid="image-operation-feedback"]')).toContainText("Bytes: 4.0 KB");
    await expect(page.locator('[data-testid="image-operation-feedback"]')).toContainText("Path: cratebay-images.tar");
  });

  test("can import an image archive", async ({ page }) => {
    await page.getByRole("button", { name: "Import" }).click();
    await page.getByPlaceholder("Input archive path").fill("/tmp/cratebay-image.tar");
    await page.getByRole("button", { name: "Import Archive" }).click();

    await expect(page.getByText("Imported 1 image(s).")).toBeVisible();
    await expect(page.locator('[data-testid="image-operation-feedback"]')).toContainText("Images: 1");
    await expect(page.locator('[data-testid="image-operation-feedback"]')).toContainText("Path: /tmp/cratebay-image.tar");
    await expect(page.getByText("cratebay/imported:test")).toBeVisible();
  });

  test("can pack a container into a local image", async ({ page }) => {
    await page.getByRole("button", { name: "Pack" }).click();
    await expect(page.getByRole("dialog").getByText("node-01 (running)")).toBeVisible();
    await expect(page.getByTestId("image-pack-name-input")).toHaveValue("cratebay/node-01:snapshot");
    await page.getByTestId("image-pack-submit").click();

    await expect(page.getByText("Packed node-01 as cratebay/node-01:snapshot.")).toBeVisible();
    await expect(page.locator('[data-testid="image-operation-feedback"]')).toContainText("Container: node-01");
    await expect(page.locator(imagesPage.imageRow).filter({ hasText: "cratebay/node-01:snapshot" })).toBeVisible();
  });

  test("can remove a local image", async ({ page }) => {
    const nodeRow = page.locator(imagesPage.imageRow).filter({ hasText: "node:20-alpine" });
    await nodeRow.locator(imagesPage.imageRemoveAction).click();

    await expect(page.getByRole("dialog").getByText("Are you sure you want to remove this image?")).toBeVisible();
    await page.getByRole("checkbox", { name: "Force removal" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Delete" }).click();

    await expect(page.locator(imagesPage.imageRow).filter({ hasText: "node:20-alpine" })).toHaveCount(0);
  });
});
