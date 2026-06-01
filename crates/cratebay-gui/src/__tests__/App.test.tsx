import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useAppStore } from "@/stores/appStore";
import { APP_VERSION } from "@/lib/constants";

// Mock child pages to avoid deep rendering issues (e.g. infinite update loops)
vi.mock("@/pages/ContainersPage", () => ({
  ContainersPage: () => <div data-testid="page-containers">ContainersPage</div>,
}));
vi.mock("@/pages/ImagesPage", () => ({
  ImagesPage: () => <div data-testid="page-images">ImagesPage</div>,
}));
vi.mock("@/pages/SettingsPage", () => ({
  SettingsPage: () => <div data-testid="page-settings">SettingsPage</div>,
}));

// Mock @tauri-apps/api to avoid native module errors in test environment
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Import App after mocks
import App from "../App";

describe("App", () => {
  beforeEach(() => {
    useAppStore.setState({
      currentPage: "containers",
      sidebarOpen: true,
      sidebarWidth: 260,
      dockerConnected: false,
      runtimeStatus: "stopped",
      theme: "dark",
    });
  });

  it("renders the application within AppLayout", () => {
    render(<App />);
    // App name appears in Sidebar logo section
    const elements = screen.getAllByText(/CrateBay/i);
    expect(elements.length).toBeGreaterThanOrEqual(1);
  });

  it("renders the version number", () => {
    render(<App />);
    const versionElements = screen.getAllByText(`v${APP_VERSION}`);
    expect(versionElements.length).toBeGreaterThanOrEqual(1);
  });

  it("renders the default Containers page", () => {
    render(<App />);
    expect(screen.getByTestId("page-containers")).toBeInTheDocument();
  });

  it("renders navigation sidebar with all pages", () => {
    render(<App />);
    const navButtons = Array.from(
      document.querySelectorAll('[data-testid^="nav-"]'),
    ).map((el) => el.getAttribute("data-testid"));

    expect(navButtons).toEqual([
      "nav-containers",
      "nav-images",
      "nav-settings",
    ]);
  });

  it.each([
    ["containers", "page-containers"],
    ["images", "page-images"],
    ["settings", "page-settings"],
  ] as const)("renders only the %s page", (currentPage, pageTestId) => {
    useAppStore.setState({ currentPage });

    render(<App />);

    expect(screen.getByTestId(pageTestId)).toBeInTheDocument();
    expect(document.querySelectorAll('[data-testid^="page-"]')).toHaveLength(1);
  });
});
