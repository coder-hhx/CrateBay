import { expect, test } from "@playwright/test"
import { gotoApp } from "./support/tauri"

test("runs container, image, and volume flows end-to-end", async ({ page }) => {
  await gotoApp(page)

  await page.getByTestId("nav-containers").click()
  await page.getByTestId("containers-run").click()
  const runContainerDialog = page.getByTestId("containers-dialog-run")
  await runContainerDialog.getByPlaceholder("nginx:latest").fill("redis:7")
  await runContainerDialog.getByPlaceholder("my-container").fill("e2e-redis")
  await runContainerDialog.getByRole("button", { name: "Create" }).click()
  await expect(page.getByText("e2e-redis")).toBeVisible()

  await page.getByTestId("nav-images").click()
  await page.getByTestId("images-tab-search").click()
  await page.getByPlaceholder("Search images").fill("redis")
  await page.getByRole("button", { name: "Search" }).click()
  await expect(page.getByText("redis:7")).toBeVisible()
  await page.getByRole("button", { name: "Run" }).first().click()

  const imageRunDialog = page.getByTestId("images-dialog-run")
  await imageRunDialog.getByRole("textbox").nth(1).fill("search-redis")
  await imageRunDialog.getByRole("button", { name: "Create" }).click()
  await expect(imageRunDialog).toContainText("docker exec -it")
  await imageRunDialog.getByRole("button", { name: "Close" }).first().click()

  await page.getByTestId("nav-containers").click()
  await expect(page.getByText("search-redis")).toBeVisible()

  await page.getByTestId("nav-volumes").click()
  await page.getByTestId("volumes-create").click()
  await page.getByRole("dialog").getByPlaceholder("my-volume").fill("e2e-data")
  await page.getByRole("dialog").getByRole("button", { name: "Create" }).click()
  await expect(page.getByText("e2e-data")).toBeVisible()

  const volumeCard = page.locator('[data-slot="card"]').filter({ has: page.getByText("e2e-data") }).first()
  await volumeCard.getByRole("button", { name: "Inspect" }).click()
  await expect(page.getByTestId("volumes-inspect-dialog")).toContainText("e2e-data")
})

test("runs vm and kubernetes flows end-to-end", async ({ page }) => {
  await gotoApp(page)

  await page.getByTestId("nav-vms").click()
  await page.getByTestId("vms-create").click()
  const vmDialog = page.getByTestId("vms-dialog-create")
  await vmDialog.getByPlaceholder("myvm").fill("e2e-vm")
  await vmDialog.getByRole("button", { name: "Create" }).click()
  await expect(page.getByText("e2e-vm")).toBeVisible()

  const vmCard = page.locator('[data-slot="card"]').filter({ has: page.getByText("e2e-vm") }).first()
  await vmCard.getByTitle("Details").click()
  await vmCard.getByTestId(/vm-tab-ports-/).click()
  await vmCard.getByTestId(/vm-port-host-/).fill("2228")
  await vmCard.getByTestId(/vm-port-guest-/).fill("22")
  await vmCard.getByTestId(/vm-port-forward-add-/).click()
  await expect(vmCard.getByText("2228")).toBeVisible()

  await vmCard.getByTestId(/vm-tab-ssh-/).click()
  await expect(vmCard.getByTestId(/vm-ssh-detected-port-/)).toContainText("2228")
  await vmCard.getByTestId(/vm-login-command-/).click()
  await expect(page.getByTestId("app-modal-text")).toContainText("ssh root@127.0.0.1 -p 2228")
  await page.getByTestId("app-modal-text").getByRole("button", { name: "Close" }).first().click()

  await page.getByTestId("nav-kubernetes").click()
  await page.getByTestId("k8s-tab-pods").click()
  await expect(page.getByText("api-6d8d6f9c7c-z9q2w")).toBeVisible()
  await page.locator("tr").filter({ has: page.getByText("api-6d8d6f9c7c-z9q2w") }).getByRole("button", { name: "Logs" }).click()
  await expect(page.getByTestId("k8s-dialog-pod-logs")).toContainText("server started")
})
