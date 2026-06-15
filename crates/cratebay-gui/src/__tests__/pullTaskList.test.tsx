import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { PullTaskList } from "@/components/images/PullTaskList";
import { usePullStore } from "@/stores/pullStore";
import { useSettingsStore } from "@/stores/settingsStore";

describe("PullTaskList", () => {
  beforeEach(() => {
    usePullStore.setState({
      tasks: [],
    });
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("renders pull task labels in the active UI language", async () => {
    usePullStore.setState({
      tasks: [
        {
          id: "pull-active",
          image: "alpine:latest",
          progress: 0,
          status: "尝试镜像站 1/3",
          complete: false,
          error: null,
          currentBytes: 0,
          totalBytes: 0,
          speed: 0,
        },
        {
          id: "pull-done",
          image: "node:20-alpine",
          progress: 100,
          status: "",
          complete: true,
          error: null,
          currentBytes: 1024,
          totalBytes: 1024,
          speed: 0,
        },
        {
          id: "pull-failed",
          image: "busybox:latest",
          progress: 0,
          status: "",
          complete: true,
          error: "registry unavailable",
          currentBytes: 0,
          totalBytes: 0,
          speed: 0,
        },
      ],
    });

    render(<PullTaskList />);

    expect(screen.getByRole("button", { name: "Pull tasks" })).toBeInTheDocument();
    expect(await screen.findByText("Pull tasks (1 active / 2 completed)")).toBeInTheDocument();
    expect(screen.getByText("Preparing...")).toBeInTheDocument();
    expect(screen.getByText("Done")).toBeInTheDocument();
    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear" })).toBeInTheDocument();
    expect(screen.queryByText(/尝试镜像站|拉取|准备中|失败|完成/)).not.toBeInTheDocument();
  });
});
