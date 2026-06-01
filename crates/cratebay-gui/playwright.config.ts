import { defineConfig, devices } from "@playwright/test";
import path from "path";

/**
 * E2E 测试配置
 *
 * 针对 Tauri webview 和本地开发环境优化
 * 支持开发模式 (`pnpm tauri dev`) 和 CI 模式 (`pnpm preview`)
 */
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,

  /* 测试超时 — Tauri 应用启动较慢 */
  timeout: 60 * 1000,
  expect: {
    timeout: 10 * 1000,
  },

  /* 失败重试 — 网络不稳定时有用 */
  retries: process.env.CI ? 2 : 1,

  /* 工作进程 — 防止并发导致的 Tauri 端口冲突 */
  workers: 1,

  /* 报告器 */
  reporter: [
    ["html"],
    ["list"],
    ...(process.env.CI ? [["github"]] : []),
  ],

  use: {
    baseURL: "http://localhost:1420",

    /* 仅失败时捕获截图 */
    screenshot: "only-on-failure",

    /* 失败时保存视频 */
    video: "retain-on-failure",

    /* 跟踪以用于调试 */
    trace: "on-first-retry",

    /* 增加导航超时以应对 Tauri 启动延迟 */
    navigationTimeout: 30 * 1000,

    /* Tauri webview 特性 */
    actionTimeout: 15 * 1000,
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  /* Web 服务器配置 */
  webServer: [
    {
      /* 开发模式: 完整 Tauri 应用 */
      command: "pnpm tauri dev",
      port: 1420,
      reuseExistingServer: !process.env.CI,
      timeout: 3 * 60 * 1000, // Tauri 首次启动可能需要 2-3 分钟
      env: {
        TAURI_ENV_DEBUG: "true",
        CRATEBAY_DISABLE_RUNTIME_AUTO_START: "1",
      },
    },
  ],

  /* 全局超时 */
  globalTimeout: 30 * 60 * 1000, // 30 分钟总超时
});
