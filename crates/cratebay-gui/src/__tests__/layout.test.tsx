import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AppLayout } from "@/components/layout/AppLayout";
import { Sidebar } from "@/components/layout/Sidebar";
import { TopBar } from "@/components/layout/TopBar";
import { StatusBar } from "@/components/layout/StatusBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useChatStore } from "@/stores/chatStore";

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

function resetLocaleAndChat() {
  useSettingsStore.setState((state) => ({
    settings: {
      ...state.settings,
      language: "en",
    },
  }));
  useChatStore.setState({
    sessions: [],
    activeSessionId: null,
  });
}

// ---------------------------------------------------------------------------
// AppLayout (already wraps children with TooltipProvider)
// ---------------------------------------------------------------------------
describe("AppLayout", () => {
  beforeEach(() => {
    resetLocaleAndChat();
    useAppStore.setState({
      currentPage: "chat",
      sidebarOpen: true,
      sidebarWidth: 260,
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

    // "Chat" appears in both Sidebar nav and TopBar breadcrumb, use getAllByText
    const chatElements = screen.getAllByText("Chat");
    expect(chatElements.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Containers")).toBeInTheDocument();
    expect(screen.getByText("Images")).toBeInTheDocument();
    const settingsElements = screen.getAllByText("Settings");
    expect(settingsElements.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// Sidebar (requires TooltipProvider wrapper)
// ---------------------------------------------------------------------------
describe("Sidebar", () => {
  beforeEach(() => {
    resetLocaleAndChat();
    useAppStore.setState({
      currentPage: "chat",
      sidebarOpen: true,
      sidebarWidth: 260,
    });
  });

  it("renders all nav items", () => {
    render(
      <WithTooltip>
        <Sidebar />
      </WithTooltip>,
    );

    expect(screen.getByText("Chat")).toBeInTheDocument();
    expect(screen.getByText("Containers")).toBeInTheDocument();
    expect(screen.getByText("Images")).toBeInTheDocument();
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

    fireEvent.click(screen.getByText("Chat"));
    expect(useAppStore.getState().currentPage).toBe("chat");
  });
});

// ---------------------------------------------------------------------------
// TopBar
// ---------------------------------------------------------------------------
describe("TopBar", () => {
  beforeEach(() => {
    resetLocaleAndChat();
    useAppStore.setState({
      currentPage: "chat",
      sidebarOpen: true,
    });
  });

  it("shows chat session title on chat page", () => {
    render(<TopBar />);
    const newChatElements = screen.getAllByText("New Chat");
    expect(newChatElements.length).toBeGreaterThanOrEqual(1);
  });

  it("updates header content when page changes", () => {
    const { rerender } = render(<TopBar />);
    const newChatElements = screen.getAllByText("New Chat");
    expect(newChatElements.length).toBeGreaterThanOrEqual(1);

    useAppStore.setState({ currentPage: "settings" });
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
    resetLocaleAndChat();
    useAppStore.setState({
      dockerConnected: false,
      runtimeStatus: "stopped",
    });
  });

  it("shows disconnected status by default", () => {
    render(<StatusBar />);
    expect(screen.getByText("Not Connected")).toBeInTheDocument();
  });

  it("shows engine ready when docker connected", () => {
    useAppStore.setState({ dockerConnected: true, runtimeStatus: "running" });
    render(<StatusBar />);
    expect(screen.getByText("Engine Ready")).toBeInTheDocument();
  });

  it("shows engine ready when docker connected regardless of runtimeStatus", () => {
    useAppStore.setState({ dockerConnected: true, runtimeStatus: "stopped" });
    render(<StatusBar />);
    expect(screen.getByText("Engine Ready")).toBeInTheDocument();
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
