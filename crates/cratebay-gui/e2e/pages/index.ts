import { Page, expect } from "@playwright/test";

export class BasePage {
  constructor(readonly page: Page) {}

  async goto(path: string = "/") {
    await this.page.goto(path);
    await this.page.waitForLoadState("networkidle");
  }

  async click(selector: string) {
    await this.page.locator(selector).click();
  }

  async fill(selector: string, text: string) {
    await this.page.locator(selector).fill(text);
  }

  async waitForElement(selector: string, timeout = 10_000) {
    await this.page.locator(selector).waitFor({ timeout });
  }

  async waitForNavigation() {
    await this.page.waitForLoadState("networkidle");
  }
}

export class AppLayoutPage extends BasePage {
  readonly containersNavButton = '[data-testid="nav-containers"]';
  readonly imagesNavButton = '[data-testid="nav-images"]';
  readonly settingsNavButton = '[data-testid="nav-settings"]';
  readonly appTitle = '[data-testid="app-title"]';
  readonly containersPodsTab = '[data-testid="containers-tab-pods"]';

  async navigateToContainers() {
    await this.click(this.containersNavButton);
    await this.waitForNavigation();
  }

  async navigateToImages() {
    await this.click(this.imagesNavButton);
    await this.waitForNavigation();
  }

  async navigateToPods() {
    await this.click(this.containersNavButton);
    await this.waitForNavigation();
    await this.click(this.containersPodsTab);
    await this.waitForNavigation();
  }

  async navigateToSettings() {
    await this.click(this.settingsNavButton);
    await this.waitForNavigation();
  }

  async verifyAppLoaded() {
    await this.waitForElement(this.appTitle);
  }
}

export class ImagesPageObject extends AppLayoutPage {
  readonly imagePage = '[data-testid="images-page"]';
  readonly imagesLocalTab = '[data-testid="images-tab-local"]';
  readonly imagesSearchTab = '[data-testid="images-tab-search"]';
  readonly imageSearchInput = '[data-testid="image-search-input"]';
  readonly imageSearchSubmit = '[data-testid="image-search-submit"]';
  readonly imageSearchResult = '[data-testid="image-search-result"]';
  readonly imageSearchPull = '[data-testid="image-search-pull"]';
  readonly imageRow = '[data-testid="image-row"]';
  readonly imageRemoveAction = '[data-testid="image-remove-action"]';

  async verifyImagesLoaded() {
    await this.waitForElement(this.imagePage);
  }
}

export class PodsPageObject extends AppLayoutPage {
  readonly podsPage = '[data-testid="pods-page"]';
  readonly containersTabs = '[data-testid="containers-section-tabs"]';

  async verifyPodsLoaded() {
    await this.waitForElement(this.containersTabs);
    await this.waitForElement(this.podsPage);
  }
}

export class SettingsPageObject extends AppLayoutPage {
  readonly generalTab = '[data-testid="settings-tab-general"]';
  readonly runtimeTab = '[data-testid="settings-tab-runtime"]';
  readonly aboutTab = '[data-testid="settings-tab-about"]';

  async verifySettingsLoaded() {
    await this.waitForElement(this.generalTab);
  }
}

export class ContainersPageObject extends AppLayoutPage {
  readonly containerList = '[data-testid="container-list"]';
  readonly containerCard = '[data-testid="container-card"]';
  readonly createContainerButton =
    '[data-testid="create-container"], button:has-text("Create")';
  readonly startButton = '[data-testid="container-start"], button:has-text("Start")';
  readonly stopButton = '[data-testid="container-stop"], button:has-text("Stop")';
  readonly deleteButton = '[data-testid="container-delete"], button:has-text("Delete")';
  readonly statusFilter = '[data-testid="status-filter"], select';
  readonly searchInput = '[data-testid="search-input"], input';

  async verifyContainerListLoaded() {
    await this.waitForElement(this.containerList);
  }

  async verifyContainerAppears(name: string) {
    const container = this.page.locator(`${this.containerCard}:has-text("${name}")`);
    await expect(container).toBeVisible();
  }
}
