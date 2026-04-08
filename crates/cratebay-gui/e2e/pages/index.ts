import { Page, expect } from "@playwright/test";

/**
 * BasePage — 所有页面的基类
 * 提供通用的页面交互方法
 */
export class BasePage {
  constructor(readonly page: Page) {}

  async goto(path: string = "/") {
    await this.page.goto(path);
    await this.page.waitForLoadState("networkidle");
  }

  async waitForElement(selector: string, timeout = 10000) {
    await this.page.locator(selector).waitFor({ timeout });
  }

  async click(selector: string) {
    await this.page.locator(selector).click();
  }

  async fill(selector: string, text: string) {
    await this.page.locator(selector).fill(text);
  }

  async getText(selector: string): Promise<string> {
    return this.page.locator(selector).textContent() ?? "";
  }

  async isVisible(selector: string): Promise<boolean> {
    try {
      return await this.page.locator(selector).isVisible({ timeout: 3000 });
    } catch {
      return false;
    }
  }

  async waitForNavigation() {
    await this.page.waitForLoadState("networkidle");
  }
}

/**
 * AppLayoutPage — 应用主布局 POM
 * 包含侧边栏和导航
 */
export class AppLayoutPage extends BasePage {
  // 侧边栏导航选择器
  readonly chatNavButton = '[data-testid="nav-chat"], button:has-text("Chat")';
  readonly containersNavButton =
    '[data-testid="nav-containers"], button:has-text("Containers")';
  readonly imagesNavButton =
    '[data-testid="nav-images"], button:has-text("Images")';
  readonly settingsNavButton =
    '[data-testid="nav-settings"], button:has-text("Settings")';
  readonly mcpNavButton =
    '[data-testid="settings-tab-mcp"], button:has-text("MCP")';

  // 应用标题
  readonly appTitle = '[data-testid="app-title"]';

  async navigateToChat() {
    await this.click(this.chatNavButton);
    await this.waitForNavigation();
  }

  async navigateToContainers() {
    await this.click(this.containersNavButton);
    await this.waitForNavigation();
  }

  async navigateToImages() {
    await this.click(this.imagesNavButton);
    await this.waitForNavigation();
  }

  async navigateToMcp() {
    await this.navigateToSettings();
    await this.click(this.mcpNavButton);
    await this.waitForNavigation();
  }

  async navigateToSettings() {
    await this.click(this.settingsNavButton);
    await this.waitForNavigation();
  }

  async verifyAppLoaded() {
    await this.waitForElement(this.appTitle);
  }

  async getCurrentPage(): Promise<string> {
    // 检查哪个导航项处于活跃状态
    const chatActive = await this.isVisible(
      this.chatNavButton + "[aria-current]"
    );
    if (chatActive) return "chat";

    const containersActive = await this.isVisible(
      this.containersNavButton + "[aria-current]"
    );
    if (containersActive) return "containers";

    const imagesActive = await this.isVisible(
      this.imagesNavButton + "[aria-current]"
    );
    if (imagesActive) return "images";

    const settingsActive = await this.isVisible(
      this.settingsNavButton + "[aria-current]"
    );
    if (settingsActive) {
      const mcpActive = await this.page
        .locator(this.mcpNavButton)
        .getAttribute("data-state")
        .then((value) => value === "active")
        .catch(() => false);
      return mcpActive ? "mcp" : "settings";
    }

    return "unknown";
  }
}

/**
 * ChatPage — 聊天页面 POM
 */
export class ChatPageObject extends AppLayoutPage {
  // 聊天输入
  readonly chatInput = '[data-testid="chat-input"], textarea';
  readonly sendButton = '[data-testid="send-button"], button:has-text("Send")';

  // 消息列表
  readonly messageList = '[data-testid="message-list"]';
  readonly messageBubble = '[data-testid="message-bubble"]';
  readonly userMessage = '[data-testid="message"][data-role="user"]';
  readonly assistantMessage = '[data-testid="message"][data-role="assistant"]';

  // Agent 相关
  readonly agentThinking = '[data-testid="agent-thinking"]';
  readonly toolCallCard = '[data-testid="tool-call-card"]';

  // 会话
  readonly newSessionButton =
    'header [data-testid="new-session"], header button:has-text("New")';
  readonly sessionList = '[data-testid="session-list"]';

  async sendMessage(text: string) {
    await this.fill(this.chatInput, text);
    await this.click(this.sendButton);
  }

  async verifyMessageAppears(text: string, timeout = 30000) {
    const locator = this.page.locator(`text="${text}"`);
    await expect(locator).toBeVisible({ timeout });
  }

  async verifyMessagesLoaded() {
    await this.waitForElement(this.messageList);
  }

  async getMessageCount(): Promise<number> {
    return this.page.locator(this.messageBubble).count();
  }

  async waitForNewMessage(timeout = 30000) {
    const currentCount = await this.getMessageCount();
    await this.page.waitForFunction(
      async (count) => {
        const bubbles = await this.page.locator(this.messageBubble).count();
        return bubbles > count;
      },
      currentCount,
      { timeout }
    );
  }

  async verifyAgentThinking() {
    await this.waitForElement(this.agentThinking, 30000);
  }

  async startNewSession() {
    await this.click(this.newSessionButton);
    await this.waitForNavigation();
  }

  async verifyInputReady() {
    await this.waitForElement(this.chatInput);
    const input = this.page.locator(this.chatInput);
    await expect(input).toBeEnabled();
  }
}

/**
 * SettingsPage — 设置页面 POM
 */
export class SettingsPageObject extends AppLayoutPage {
  // 标签页
  readonly generalTab =
    '[data-testid="settings-tab-general"], button:has-text("General")';
  readonly providersTab =
    '[data-testid="settings-tab-providers"], button:has-text("Providers")';
  readonly advancedTab =
    '[data-testid="settings-tab-advanced"], button:has-text("Advanced")';

  // Provider 管理
  readonly addProviderButton =
    '[data-testid="add-provider"], button:has-text("Add")';
  readonly providerList = '[data-testid="provider-list"]';
  readonly providerCard = '[data-testid="provider-card"]';

  // Provider 表单
  readonly providerNameInput =
    '[data-testid="provider-name"], input[placeholder*="Name"]';
  readonly providerBaseUrlInput =
    '[data-testid="provider-base-url"], input[placeholder*="URL"]';
  readonly providerApiKeyInput =
    '[data-testid="provider-api-key"], input[type="password"]';
  readonly providerFormatSelect =
    '[data-testid="provider-format"], select, [role="combobox"]';
  readonly saveProviderButton =
    '[data-testid="save-provider"], button:has-text("Save")';
  readonly testConnectionButton =
    '[data-testid="test-connection"], button:has-text("Test")';

  // 通用设置
  readonly languageSelect = '[data-testid="language-select"], select';
  readonly themeSelect = '[data-testid="theme-select"], select';
  readonly sendOnEnterToggle =
    '[data-testid="send-on-enter"], input[type="checkbox"]';

  async navigateToProviders() {
    await this.navigateToSettings();
    await this.click(this.providersTab);
    await this.waitForNavigation();
  }

  async addNewProvider(
    name: string,
    baseUrl: string,
    apiKey: string,
    format: string
  ) {
    await this.click(this.addProviderButton);
    await this.waitForElement(this.providerNameInput);

    await this.fill(this.providerNameInput, name);
    await this.fill(this.providerBaseUrlInput, baseUrl);
    await this.fill(this.providerApiKeyInput, apiKey);

    // 选择 API format
    const formatSelect = this.page.locator(this.providerFormatSelect);
    await formatSelect.click();
    await this.page.locator(`text="${format}"`).click();

    // 保存
    await this.click(this.saveProviderButton);
    await this.page.waitForTimeout(1000); // 等待保存完成
  }

  async verifyProviderExists(name: string) {
    const provider = this.page.locator(
      `${this.providerCard}:has-text("${name}")`
    );
    await expect(provider).toBeVisible();
  }

  async testConnection() {
    await this.click(this.testConnectionButton);
    // 等待测试结果
    await this.page.waitForTimeout(2000);
  }

  async verifySettingsLoaded() {
    await this.waitForElement(this.generalTab);
  }

  async changeLanguage(language: "en" | "zh-CN") {
    const select = this.page.locator(this.languageSelect);
    await select.selectOption(language);
    await this.page.waitForTimeout(500);
  }

  async verifyLanguageChanged(language: string) {
    const select = this.page.locator(this.languageSelect);
    const value = await select.inputValue();
    expect(value).toBe(language);
  }
}

/**
 * ContainersPage — 容器管理页面 POM
 */
export class ContainersPageObject extends AppLayoutPage {
  // 容器列表
  readonly containerList = '[data-testid="container-list"]';
  readonly containerCard = '[data-testid="container-card"]';
  readonly createContainerButton =
    '[data-testid="create-container"], button:has-text("Create")';

  // 容器操作
  readonly startButton = '[data-testid="container-start"], button:has-text("Start")';
  readonly stopButton = '[data-testid="container-stop"], button:has-text("Stop")';
  readonly deleteButton =
    '[data-testid="container-delete"], button:has-text("Delete")';

  // 过滤器
  readonly statusFilter = '[data-testid="status-filter"], select';
  readonly searchInput = '[data-testid="search-input"], input';

  async verifyContainerListLoaded() {
    await this.waitForElement(this.containerList);
  }

  async getContainerCount(): Promise<number> {
    return this.page.locator(this.containerCard).count();
  }

  async verifyContainerAppears(name: string) {
    const container = this.page.locator(
      `${this.containerCard}:has-text("${name}")`
    );
    await expect(container).toBeVisible();
  }

  async startContainer(name: string) {
    const container = this.page.locator(
      `${this.containerCard}:has-text("${name}")`
    );
    await container.locator(this.startButton).click();
    await this.page.waitForTimeout(1000);
  }

  async stopContainer(name: string) {
    const container = this.page.locator(
      `${this.containerCard}:has-text("${name}")`
    );
    await container.locator(this.stopButton).click();
    await this.page.waitForTimeout(1000);
  }

  async filterByStatus(status: "all" | "running" | "stopped") {
    const select = this.page.locator(this.statusFilter);
    await select.selectOption(status);
    await this.page.waitForTimeout(500);
  }

  async searchContainers(query: string) {
    await this.fill(this.searchInput, query);
    await this.page.waitForTimeout(500);
  }
}

/**
 * McpPage — MCP 服务器页面 POM
 */
export class McpPageObject extends AppLayoutPage {
  // 服务器列表
  readonly serverList = '[data-testid="mcp-server-list"]';
  readonly serverCard = '[data-testid="mcp-server-card"]';
  readonly addServerButton =
    '[data-testid="add-mcp-server"], button:has-text("Add")';

  // 工具列表
  readonly toolList = '[data-testid="mcp-tool-list"]';
  readonly toolItem = '[data-testid="mcp-tool-item"]';

  // 服务器操作
  readonly connectButton =
    '[data-testid="mcp-connect"], button:has-text("Connect")';
  readonly disconnectButton =
    '[data-testid="mcp-disconnect"], button:has-text("Disconnect")';
  readonly deleteServerButton =
    '[data-testid="mcp-delete"], button:has-text("Delete")';

  async verifyServerListLoaded() {
    await this.waitForElement(this.serverList);
  }

  async getServerCount(): Promise<number> {
    return this.page.locator(this.serverCard).count();
  }

  async verifyServerAppears(name: string) {
    const server = this.page.locator(
      `${this.serverCard}:has-text("${name}")`
    );
    await expect(server).toBeVisible();
  }

  async connectServer(name: string) {
    const server = this.page.locator(
      `${this.serverCard}:has-text("${name}")`
    );
    await server.locator(this.connectButton).click();
    await this.page.waitForTimeout(1000);
  }

  async verifyToolsLoaded() {
    await this.waitForElement(this.toolList);
  }

  async getToolCount(): Promise<number> {
    return this.page.locator(this.toolItem).count();
  }

  async verifyToolAppears(toolName: string) {
    const tool = this.page.locator(
      `${this.toolItem}:has-text("${toolName}")`
    );
    await expect(tool).toBeVisible();
  }
}
