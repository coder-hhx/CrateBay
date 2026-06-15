import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ContainerDeleteDialog } from "@/components/container/ContainerDeleteDialog";
import { useSettingsStore } from "@/stores/settingsStore";
import type { ContainerInfo } from "@/types/container";

const deleteContainerMock = vi.fn();

vi.mock("@/stores/containerStore", () => ({
  useContainerStore: (selector: (state: { deleteContainer: typeof deleteContainerMock }) => unknown) =>
    selector({ deleteContainer: deleteContainerMock }),
}));

const container: ContainerInfo = {
  id: "container-1",
  shortId: "container-1",
  name: "node-01",
  image: "node:20-alpine",
  status: "running",
  state: "running",
  createdAt: "2026-06-15T00:00:00Z",
  ports: [],
  labels: {},
};

describe("ContainerDeleteDialog", () => {
  beforeEach(() => {
    deleteContainerMock.mockReset();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("deletes without force by default", async () => {
    const onOpenChange = vi.fn();
    deleteContainerMock.mockResolvedValue(undefined);

    render(
      <ContainerDeleteDialog
        container={container}
        open
        onOpenChange={onOpenChange}
      />,
    );

    fireEvent.click(screen.getByTestId("container-delete-confirm"));

    await waitFor(() => {
      expect(deleteContainerMock).toHaveBeenCalledWith("container-1", false);
    });
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("passes force when the checkbox is enabled", async () => {
    deleteContainerMock.mockResolvedValue(undefined);

    render(
      <ContainerDeleteDialog
        container={container}
        open
        onOpenChange={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("checkbox", { name: "Force removal" }));
    fireEvent.click(screen.getByTestId("container-delete-confirm"));

    await waitFor(() => {
      expect(deleteContainerMock).toHaveBeenCalledWith("container-1", true);
    });
  });

  it("keeps the dialog open and shows the native rejection reason", async () => {
    const onOpenChange = vi.fn();
    deleteContainerMock.mockRejectedValue(new Error("container remove failed"));

    render(
      <ContainerDeleteDialog
        container={container}
        open
        onOpenChange={onOpenChange}
      />,
    );

    fireEvent.click(screen.getByTestId("container-delete-confirm"));

    expect(await screen.findByText("container remove failed")).toBeInTheDocument();
    expect(within(screen.getByRole("dialog")).getByText("node-01")).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
  });

  it("localizes the delete fallback error", async () => {
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "zh-CN",
      },
    }));
    deleteContainerMock.mockRejectedValue(null);

    render(
      <ContainerDeleteDialog
        container={container}
        open
        onOpenChange={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByTestId("container-delete-confirm"));

    expect(await screen.findByText("删除失败")).toBeInTheDocument();
    expect(screen.queryByText("Delete failed")).not.toBeInTheDocument();
  });
});
