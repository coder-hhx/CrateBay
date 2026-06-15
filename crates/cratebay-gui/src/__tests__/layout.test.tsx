import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen, fireEvent } from "@testing-library/react";
import { AppLayout } from "@/components/layout/AppLayout";
import { Sidebar } from "@/components/layout/Sidebar";
import { TopBar } from "@/components/layout/TopBar";
import { StatusBar } from "@/components/layout/StatusBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";

// Mock @tauri-apps/api to avoid native module errors
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

/**
 * Helper to wrap a component with TooltipProvider (required by Sidebar).
 */
function WithTooltip({ children }: { children: React.ReactNode }) {
  return <TooltipProvider>{children}</TooltipProvider>;
}

function resetLocale() {
  useSettingsStore.setState((state) => ({
    settings: {
      ...state.settings,
      language: "en",
    },
  }));
}

// ---------------------------------------------------------------------------
// AppLayout (already wraps children with TooltipProvider)
// ---------------------------------------------------------------------------
describe("AppLayout", () => {
  beforeEach(() => {
    resetLocale();
    useAppStore.setState({
      currentPage: "containers",
      sidebarOpen: true,
      sidebarWidth: 260,
      engineConnected: false,
      dockerConnected: false,
      runtimeStatus: "stopped",
      theme: "dark",
    });
  });

  it("renders sidebar, content area, and status bar", () => {
    render(
      <AppLayout>
        <div data-testid="child-content">Page Content</div>
      </AppLayout>,
    );

    // Sidebar renders the app name (also in TopBar breadcrumb, so use getAllByText)
    const crateBayElements = screen.getAllByText("CrateBay");
    expect(crateBayElements.length).toBeGreaterThanOrEqual(1);
    // Children are rendered
    expect(screen.getByTestId("child-content")).toBeInTheDocument();
    // Version is rendered (appears in both Sidebar bottom and StatusBar)
    const versionElements = screen.getAllByText(/v0\.9\.0/);
    expect(versionElements.length).toBeGreaterThanOrEqual(1);
  });

  it("renders navigation items in sidebar", () => {
    render(
      <AppLayout>
        <div>Content</div>
      </AppLayout>,
    );

    const imageElements = screen.getAllByText("Images");
    expect(imageElements.length).toBeGreaterThanOrEqual(1);
    const containerElements = screen.getAllByText("Containers");
    expect(containerElements.length).toBeGreaterThanOrEqual(1);
    const settingsElements = screen.getAllByText("Settings");
    expect(settingsElements.length).toBeGreaterThanOrEqual(1);
    const volumesElements = screen.getAllByText("Volumes");
    expect(volumesElements.length).toBeGreaterThanOrEqual(1);
    const networksElements = screen.getAllByText("Networks");
    expect(networksElements.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// Sidebar (requires TooltipProvider wrapper)
// ---------------------------------------------------------------------------
describe("Sidebar", () => {
  beforeEach(() => {
    resetLocale();
    useAppStore.setState({
      currentPage: "containers",
      sidebarOpen: true,
      sidebarWidth: 260,
      engineConnected: false,
      dockerConnected: false,
      runtimeStatus: "stopped",
    });
  });

  it("renders all nav items", () => {
    render(
      <WithTooltip>
        <Sidebar />
      </WithTooltip>,
    );

    expect(screen.getByText("Containers")).toBeInTheDocument();
    expect(screen.getByText("Images")).toBeInTheDocument();
    expect(screen.getByText("Volumes")).toBeInTheDocument();
    expect(screen.getByText("Networks")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("clicking a nav item changes currentPage in appStore", () => {
    render(
      <WithTooltip>
        <Sidebar />
      </WithTooltip>,
    );

    fireEvent.click(screen.getByText("Settings"));
    expect(useAppStore.getState().currentPage).toBe("settings");

    fireEvent.click(screen.getByText("Containers"));
    expect(useAppStore.getState().currentPage).toBe("containers");

    fireEvent.click(screen.getByText("Images"));
    expect(useAppStore.getState().currentPage).toBe("images");

    fireEvent.click(screen.getByText("Volumes"));
    expect(useAppStore.getState().currentPage).toBe("volumes");

    fireEvent.click(screen.getByText("Networks"));
    expect(useAppStore.getState().currentPage).toBe("networks");
  });
});

// ---------------------------------------------------------------------------
// TopBar
// ---------------------------------------------------------------------------
describe("TopBar", () => {
  beforeEach(() => {
    resetLocale();
    useAppStore.setState({
      currentPage: "containers",
      sidebarOpen: true,
    });
  });

  it("shows the current page breadcrumb", () => {
    render(<TopBar />);
    const containerElements = screen.getAllByText("Containers");
    expect(containerElements.length).toBeGreaterThanOrEqual(1);
  });

  it("updates header content when page changes", () => {
    const { rerender } = render(<TopBar />);
    const containerElements = screen.getAllByText("Containers");
    expect(containerElements.length).toBeGreaterThanOrEqual(1);

    act(() => {
      useAppStore.setState({ currentPage: "settings" });
    });
    rerender(<TopBar />);
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("toggles sidebar when toggle button is clicked", () => {
    render(<TopBar />);
    expect(useAppStore.getState().sidebarOpen).toBe(true);

    const toggleBtn = screen.getByLabelText("Collapse sidebar");
    fireEvent.click(toggleBtn);
    expect(useAppStore.getState().sidebarOpen).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// StatusBar
// ---------------------------------------------------------------------------
describe("StatusBar", () => {
  beforeEach(() => {
    resetLocale();
    useAppStore.setState({
      engineConnected: false,
      dockerConnected: false,
      runtimeStatus: "stopped",
    });
  });

  it("shows disconnected status by default", () => {
    render(<StatusBar />);
    expect(screen.getByText("Not Connected")).toBeInTheDocument();
  });

  it("shows engine ready when native engine is connected", () => {
    useAppStore.setState({ engineConnected: true, dockerConnected: true, runtimeStatus: "running" });
    render(<StatusBar />);
    expect(screen.getByText("Engine Ready")).toBeInTheDocument();
  });

  it("shows engine ready when native engine is connected regardless of runtimeStatus", () => {
    useAppStore.setState({ engineConnected: true, dockerConnected: true, runtimeStatus: "stopped" });
    render(<StatusBar />);
    expect(screen.getByText("Engine Ready")).toBeInTheDocument();
  });

  it("does not show native engine ready for a compatibility-only endpoint", () => {
    useAppStore.setState({ engineConnected: false, dockerConnected: true, runtimeStatus: "starting" });
    render(<StatusBar />);
    expect(screen.queryByText("Engine Ready")).not.toBeInTheDocument();
    expect(screen.getByText("Engine Starting...")).toBeInTheDocument();
  });

  it("shows starting when runtime is starting", () => {
    useAppStore.setState({ runtimeStatus: "starting" });
    render(<StatusBar />);
    expect(screen.getByText("Engine Starting...")).toBeInTheDocument();
  });

  it("shows version number", () => {
    render(<StatusBar />);
    expect(screen.getByText("v0.9.0")).toBeInTheDocument();
  });
});
