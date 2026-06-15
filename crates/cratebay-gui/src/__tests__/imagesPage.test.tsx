import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ImagesPage } from "@/pages/ImagesPage";
import { useSettingsStore } from "@/stores/settingsStore";

const invokeMock = vi.fn();
const openMock = vi.fn();
const saveMock = vi.fn();

vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openMock(...args),
  save: (...args: unknown[]) => saveMock(...args),
}));

function localImage() {
  return {
    id: "sha256:node",
    repoTags: ["node:20-alpine"],
    sizeBytes: 120_000_000,
    sizeHuman: "120 MB",
    created: 1_700_000_000,
  };
}

function recentLocalImage(hoursAgo: number) {
  return {
    ...localImage(),
    created: Math.floor((Date.now() - hoursAgo * 60 * 60 * 1000) / 1000),
  };
}

describe("ImagesPage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    saveMock.mockReset();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("uses the native save picker for image export archives", async () => {
    saveMock.mockResolvedValue("/tmp/exported-from-picker.tar");
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([localImage()]);
      }
      if (command === "image_export") {
        return Promise.resolve(4096);
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    expect(await screen.findByText("node:20-alpine")).toBeInTheDocument();
    fireEvent.click(screen.getAllByRole("checkbox")[0]);
    fireEvent.click(screen.getByRole("button", { name: /^Export$/ }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Browse" }));

    await waitFor(() => {
      expect(saveMock).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Choose Export Path",
          defaultPath: "cratebay-images.tar",
          filters: [
            { name: "Image archives", extensions: ["tar", "tar.gz", "tgz"] },
            { name: "All files", extensions: ["*"] },
          ],
        }),
      );
    });
    expect(await screen.findByDisplayValue("/tmp/exported-from-picker.tar")).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: /^Export$/ }).at(-1)!);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("image_export", {
        images: ["node:20-alpine"],
        output: "/tmp/exported-from-picker.tar",
      });
    });
    expect(await screen.findByTestId("image-operation-feedback")).toHaveTextContent(
      "Exported 1 image(s), 4096 bytes written.",
    );
    expect(screen.getByTestId("image-operation-feedback")).toHaveTextContent("Bytes: 4.0 KB");
    expect(screen.getByTestId("image-operation-feedback")).toHaveTextContent(
      "Path: /tmp/exported-from-picker.tar",
    );
  });

  it("uses the native open picker for image import archives", async () => {
    openMock.mockResolvedValue("/tmp/imported-from-picker.tar");
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([]);
      }
      if (command === "image_import") {
        return Promise.resolve(["cratebay/imported:test"]);
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    fireEvent.click(await screen.findByRole("button", { name: "Import" }));
    fireEvent.click(screen.getByRole("button", { name: "Browse" }));

    await waitFor(() => {
      expect(openMock).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "Choose Import Archive",
          multiple: false,
          filters: [
            { name: "Image archives", extensions: ["tar", "tar.gz", "tgz"] },
            { name: "All files", extensions: ["*"] },
          ],
        }),
      );
    });
    expect(await screen.findByDisplayValue("/tmp/imported-from-picker.tar")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Import Archive" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("image_import", {
        input: "/tmp/imported-from-picker.tar",
      });
    });
    expect(await screen.findByTestId("image-operation-feedback")).toHaveTextContent(
      "Imported 1 image(s).",
    );
    expect(screen.getByTestId("image-operation-feedback")).toHaveTextContent(
      "Path: /tmp/imported-from-picker.tar",
    );
  });

  it("localizes native image archive picker labels", async () => {
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "zh-CN",
      },
    }));
    openMock.mockResolvedValue(null);
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    fireEvent.click(await screen.findByRole("button", { name: "导入" }));
    fireEvent.click(screen.getByRole("button", { name: "浏览" }));

    await waitFor(() => {
      expect(openMock).toHaveBeenCalledWith(
        expect.objectContaining({
          title: "选择导入归档",
          filters: [
            { name: "镜像归档", extensions: ["tar", "tar.gz", "tgz"] },
            { name: "所有文件", extensions: ["*"] },
          ],
        }),
      );
    });
  });

  it("localizes image search timeout feedback", async () => {
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "zh-CN",
      },
    }));
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      if (command === "image_search") {
        return new Promise(() => {});
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    fireEvent.click(await screen.findByRole("button", { name: "搜索" }));
    const searchInput = screen.getByTestId("image-search-input");
    fireEvent.change(searchInput, { target: { value: "alpine" } });

    try {
      vi.useFakeTimers();
      fireEvent.click(screen.getByTestId("image-search-submit"));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(15_000);
      });
    } finally {
      vi.useRealTimers();
    }

    expect(await screen.findByText("镜像搜索在 15 秒后超时。")).toBeInTheDocument();
    expect(screen.queryByText(/Image search timeout/i)).not.toBeInTheDocument();
  });

  it("localizes image inspect labels", async () => {
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "zh-CN",
      },
    }));
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([localImage()]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      if (command === "image_inspect") {
        return Promise.resolve({
          id: "sha256:node",
          repoTags: ["node:20-alpine"],
          sizeBytes: 120_000_000,
          created: "2026-06-15T00:00:00Z",
          architecture: "arm64",
          os: "linux",
          dockerVersion: "cratebay",
          layers: 7,
        });
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    expect(await screen.findByText("node:20-alpine")).toBeInTheDocument();
    fireEvent.click(screen.getByTitle("查看镜像详情"));

    expect(await screen.findByText("操作系统")).toBeInTheDocument();
    expect(screen.queryByText("OS")).not.toBeInTheDocument();
  });

  it("packs a container into a local image from the images toolbar", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([localImage()]);
      }
      if (command === "container_list") {
        return Promise.resolve([
          {
            id: "abc123",
            shortId: "abc123",
            name: "node-01",
            image: "node:20-alpine",
            status: "running",
            state: "running",
            createdAt: "2026-03-23T00:00:00.000Z",
            ports: [],
            labels: {},
          },
        ]);
      }
      if (command === "image_pack_container") {
        return Promise.resolve("cratebay/node-01:snapshot");
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    fireEvent.click(await screen.findByRole("button", { name: "Pack" }));
    expect(await screen.findByText("node-01 (running)")).toBeInTheDocument();
    expect(screen.getByTestId("image-pack-name-input")).toHaveValue("cratebay/node-01:snapshot");

    fireEvent.click(screen.getByTestId("image-pack-submit"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("image_pack_container", {
        container: "abc123",
        image: "cratebay/node-01:snapshot",
      });
    });
    expect(await screen.findByTestId("image-operation-feedback")).toHaveTextContent(
      "Packed node-01 as cratebay/node-01:snapshot.",
    );
    expect(screen.getByTestId("image-operation-feedback")).toHaveTextContent("Container: node-01");
  });

  it("renders local image relative times in English", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([recentLocalImage(2)]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    expect(await screen.findByText("node:20-alpine")).toBeInTheDocument();
    expect(screen.getByText("2 hr ago")).toBeInTheDocument();
    expect(screen.queryByText(/小时|天前|1 小时内/)).not.toBeInTheDocument();
  });

  it("shows an Engine offline recovery callout when local images cannot auto-start the runtime", async () => {
    let imageListCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        imageListCalls += 1;
        if (imageListCalls === 1) {
          return Promise.reject(
            new Error(
              "Implicit runtime start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START",
            ),
          );
        }
        return Promise.resolve([localImage()]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      if (command === "runtime_start") {
        return Promise.resolve("ok");
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    expect(await screen.findByText("Engine is offline")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Start Engine" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("runtime_start");
    });
    expect(await screen.findByText("node:20-alpine")).toBeInTheDocument();
    expect(screen.queryByText("Engine is offline")).not.toBeInTheDocument();
  });

  it("removes an image without force by default", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([localImage()]);
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    expect(await screen.findByText("node:20-alpine")).toBeInTheDocument();
    fireEvent.click(screen.getByTitle("Remove Image"));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("image_remove", {
        id: "sha256:node",
        force: false,
      });
    });
  });

  it("passes force when the image removal checkbox is enabled", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([localImage()]);
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    expect(await screen.findByText("node:20-alpine")).toBeInTheDocument();
    fireEvent.click(screen.getByTitle("Remove Image"));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("checkbox", { name: "Force removal" }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("image_remove", {
        id: "sha256:node",
        force: true,
      });
    });
  });

  it("shows the native image removal error when the Engine rejects deletion", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "image_list") {
        return Promise.resolve([localImage()]);
      }
      if (command === "image_remove") {
        return Promise.reject(new Error("image is in use by CrateBay containers"));
      }
      return Promise.resolve(null);
    });

    render(<ImagesPage />);

    expect(await screen.findByText("node:20-alpine")).toBeInTheDocument();
    fireEvent.click(screen.getByTitle("Remove Image"));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));

    expect(await screen.findByTestId("image-operation-feedback")).toHaveTextContent(
      "image is in use by CrateBay containers",
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
