import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { LocalizedErrorBoundary } from "@/components/ErrorBoundary";
import { useSettingsStore } from "@/stores/settingsStore";

function BrokenChild(): never {
  throw new Error("boom");
}

describe("LocalizedErrorBoundary", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    useSettingsStore.setState({
      settings: {
        language: "zh-CN",
        theme: "dark",
        registryMirrors: [],
        runtimeHttpProxy: "",
        runtimeHttpProxyBridge: false,
        runtimeHttpProxyBindHost: "0.0.0.0",
        runtimeHttpProxyBindPort: 3128,
        runtimeHttpProxyGuestHost: "192.168.64.1",
        includePrereleases: false,
      },
    });
  });

  it("renders localized fallback copy", () => {
    render(
      <LocalizedErrorBoundary>
        <BrokenChild />
      </LocalizedErrorBoundary>,
    );

    expect(screen.getByRole("heading", { name: "出现异常" })).toBeInTheDocument();
    expect(screen.getByText("CrateBay 遇到意外错误，请尝试重启应用。")).toBeInTheDocument();
    expect(screen.getByText("错误详情")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重新加载应用" })).toBeInTheDocument();
    expect(screen.queryByText("Something went wrong")).not.toBeInTheDocument();
  });
});
