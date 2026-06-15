import { describe, expect, it, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { useSettingsStore } from "@/stores/settingsStore";

function TestDialog() {
  return (
    <Dialog open>
      <DialogContent>
        <DialogTitle>Dialog title</DialogTitle>
        <DialogDescription>Dialog description</DialogDescription>
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  );
}

describe("Dialog i18n", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      settings: {
        language: "en",
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

  it("localizes default close controls", () => {
    useSettingsStore.setState((state) => ({
      settings: { ...state.settings, language: "zh-CN" },
    }));

    render(<TestDialog />);

    expect(screen.getAllByRole("button", { name: "关闭" })).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
  });
});
