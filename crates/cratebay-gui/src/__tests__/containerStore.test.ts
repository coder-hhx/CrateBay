import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockInvoke, mockListen, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing stores
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: mockListen,
  isTauri: vi.fn(() => false),
}));

import { useContainerStore } from "@/stores/containerStore";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { ContainerInfo, ContainerTemplate } from "@/types/container";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const makeContainer = (overrides: Partial<ContainerInfo> = {}): ContainerInfo => ({
  id: "c-1",
  shortId: "c-1",
  name: "node-01",
  image: "node:20-slim",
  status: "running",
  state: "running",
  createdAt: new Date().toISOString(),
  cpuCores: 2,
  memoryMb: 2048,
  ports: [],
  labels: {
    "com.cratebay.template_id": "node-dev",
  },
  ...overrides,
});

const makeTemplate = (overrides: Partial<ContainerTemplate> = {}): ContainerTemplate => ({
  id: "node-dev",
  name: "Node.js Dev",
  description: "Node.js development environment",
  image: "node:20-slim",
  defaultCommand: "bash",
  defaultCpuCores: 2,
  defaultMemoryMb: 2048,
  tags: ["node", "javascript"],
  ...overrides,
});

function resetStore() {
  const timer = useContainerStore.getState()._transitionalRefreshTimer;
  if (timer !== null) {
    clearTimeout(timer);
  }
  const controller = useContainerStore.getState()._fetchAbortController;
  controller?.abort();

  useContainerStore.setState({
    containers: [],
    images: [],
    loading: false,
    imagesLoading: false,
    error: null,
    _fetchAbortController: null,
    _transitionalRefreshTimer: null,
    selectedContainerId: null,
    templates: [],
    filter: { status: "all", search: "", templateId: null },
  });
  useAppStore.setState({ notifications: [] });
  useSettingsStore.setState((state) => ({
    settings: {
      ...state.settings,
      language: "en",
      registryMirrors: ["docker.1ms.run", "docker.xuanyuan.me", "dockerhub.icu"],
    },
  }));
}

function notificationText() {
  return useAppStore
    .getState()
    .notifications
    .map((n) => `${n.title} ${n.message ?? ""}`)
    .join("\n");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe("containerStore", () => {
  beforeEach(() => {
    resetStore();
    resetTauriMocks();
  });

  // -------------------------------------------------------------------------
  // fetchContainers
  // -------------------------------------------------------------------------
  describe("fetchContainers", () => {
    it("sets loading=true then populates containers from invoke", async () => {
      const containers = [makeContainer(), makeContainer({ id: "c-2", name: "py-dev" })];
      mockInvoke.mockResolvedValueOnce(containers);

      await useContainerStore.getState().fetchContainers();

      expect(mockInvoke).toHaveBeenCalledWith("container_list");
      expect(useContainerStore.getState().containers).toEqual(containers);
      expect(useContainerStore.getState().loading).toBe(false);
    });

    it("falls back gracefully when invoke fails (non-Tauri mode)", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useContainerStore.getState().fetchContainers();

      // Should not throw, loading should be false
      expect(useContainerStore.getState().loading).toBe(false);
    });

    it("uses the English fallback error when refresh fails without a readable message", async () => {
      mockInvoke.mockRejectedValueOnce({});

      await useContainerStore.getState().fetchContainers();

      expect(useContainerStore.getState().error).toBe(
        "Refresh failed: check the runtime connection status.",
      );
      expect(useContainerStore.getState().error).not.toMatch(/[\u4e00-\u9fff]/);
    });
  });

  // -------------------------------------------------------------------------
  // createContainer
  // -------------------------------------------------------------------------
  describe("createContainer", () => {
    it("invokes container_create with request and appends result", async () => {
      const newContainer = makeContainer({ id: "c-new", name: "new-box" });
      mockInvoke
        .mockResolvedValueOnce([
          {
            id: "img-1",
            repoTags: ["node:20-slim"],
            sizeBytes: 123,
            sizeHuman: "123 B",
            created: 111,
          },
        ])
        .mockResolvedValueOnce(newContainer)
        .mockResolvedValueOnce([newContainer]);

      const result = await useContainerStore.getState().createContainer({
        templateId: "node-dev",
        name: "new-box",
        image: "node:20-slim",
      });

      expect(mockInvoke).toHaveBeenNthCalledWith(1, "image_list");
      expect(mockInvoke).toHaveBeenNthCalledWith(2, "container_create", {
        request: {
          templateId: "node-dev",
          name: "new-box",
          image: "node:20-slim",
          registryMirrors: ["docker.1ms.run", "docker.xuanyuan.me", "dockerhub.icu"],
        },
      });
      expect(result.id).toBe("c-new");
      expect(useContainerStore.getState().containers).toHaveLength(1);
      expect(useAppStore.getState().notifications.at(-1)?.title).toBe(
        "Container new-box created",
      );
      expect(notificationText()).not.toMatch(/[\u4e00-\u9fff]/);
    });

    it("localizes pull progress, pull completion, and create notifications", async () => {
      const seenPlaceholders: string[] = [];
      const newContainer = makeContainer({ id: "c-new", name: "pull-box", image: "redis:7" });
      mockInvoke
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce("pull-test-channel")
        .mockResolvedValueOnce(newContainer)
        .mockResolvedValueOnce([newContainer]);
      mockListen.mockImplementationOnce((_event, callback) => {
        const onProgress = callback as (payload: {
          status: string;
          progress_percent: number;
          complete: boolean;
          error?: string | null;
        }) => void;

        onProgress({
          status: "尝试镜像站 1/3",
          progress_percent: 0,
          complete: false,
          error: null,
        });
        seenPlaceholders.push(useContainerStore.getState().containers.at(-1)?.shortId ?? "");

        onProgress({
          status: "Downloading",
          progress_percent: 37,
          complete: false,
          error: null,
        });
        seenPlaceholders.push(useContainerStore.getState().containers.at(-1)?.shortId ?? "");

        onProgress({
          status: "Pull complete",
          progress_percent: 100,
          complete: true,
          error: null,
        });

        return Promise.resolve(() => {});
      });

      await useContainerStore.getState().createContainer({
        name: "pull-box",
        image: "redis:7",
      });

      expect(mockInvoke).toHaveBeenNthCalledWith(2, "image_pull", {
        image: "redis:7",
        mirrors: ["docker.1ms.run", "docker.xuanyuan.me", "dockerhub.icu"],
        channel_id: expect.stringMatching(/^pull-/),
      });
      expect(mockListen).toHaveBeenCalledWith(
        expect.stringMatching(/^image:pull:pull-/),
        expect.any(Function),
      );
      expect(seenPlaceholders).toEqual(["Pulling...", "Pulling 37%"]);
      expect(useAppStore.getState().notifications.map((n) => n.title)).toEqual([
        "Pulling image redis:7",
        "Image redis:7 pull completed",
        "Container pull-box created",
      ]);
      expect(notificationText()).not.toMatch(/[\u4e00-\u9fff]/);
    });

    it("throws and removes placeholder when create fails", async () => {
      mockInvoke
        .mockResolvedValueOnce([
          {
            id: "img-1",
            repoTags: ["node:20-slim"],
            sizeBytes: 123,
            sizeHuman: "123 B",
            created: 111,
          },
        ])
        .mockRejectedValueOnce(new Error("create failed"));

      await expect(
        useContainerStore.getState().createContainer({
          templateId: "node-dev",
          name: "fallback-box",
          image: "node:20-slim",
        }),
      ).rejects.toThrow("create failed");
      expect(useContainerStore.getState().containers).toHaveLength(0);
      expect(useAppStore.getState().notifications.at(-1)?.title).toBe(
        "Container create failed",
      );
      expect(notificationText()).not.toMatch(/[\u4e00-\u9fff]/);
    });
  });

  // -------------------------------------------------------------------------
  // startContainer / stopContainer
  // -------------------------------------------------------------------------
  describe("startContainer", () => {
    it("refreshes list and updates container status to running", async () => {
      const c = makeContainer({ id: "c-1", status: "stopped" });
      useContainerStore.setState({ containers: [c] });
      mockInvoke
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce([makeContainer({ id: "c-1", status: "running", state: "running" })]);

      await useContainerStore.getState().startContainer("c-1");

      expect(mockInvoke).toHaveBeenNthCalledWith(1, "container_start", { id: "c-1" });
      expect(mockInvoke).toHaveBeenNthCalledWith(2, "container_list");
      expect(useContainerStore.getState().containers[0].status).toBe("running");
      expect(useAppStore.getState().notifications.at(0)).toMatchObject({
        type: "info",
        title: "Starting container...",
      });
      expect(notificationText()).not.toMatch(/[\u4e00-\u9fff]/);
    });
  });

  describe("stopContainer", () => {
    it("refreshes list and updates container status to stopped", async () => {
      const c = makeContainer({ id: "c-1", status: "running" });
      useContainerStore.setState({ containers: [c] });
      mockInvoke
        .mockResolvedValueOnce(undefined)
        .mockResolvedValueOnce([makeContainer({ id: "c-1", status: "stopped", state: "exited" })]);

      await useContainerStore.getState().stopContainer("c-1");

      expect(mockInvoke).toHaveBeenNthCalledWith(1, "container_stop", { id: "c-1" });
      expect(mockInvoke).toHaveBeenNthCalledWith(2, "container_list");
      expect(useContainerStore.getState().containers[0].status).toBe("stopped");
    });
  });

  // -------------------------------------------------------------------------
  // deleteContainer
  // -------------------------------------------------------------------------
  describe("deleteContainer", () => {
    it("removes the container from the list", async () => {
      const c1 = makeContainer({ id: "c-1" });
      const c2 = makeContainer({ id: "c-2", name: "py-dev" });
      useContainerStore.setState({ containers: [c1, c2] });
      mockInvoke.mockResolvedValueOnce(undefined).mockResolvedValueOnce([c2]);

      await useContainerStore.getState().deleteContainer("c-1");

      expect(mockInvoke).toHaveBeenNthCalledWith(1, "container_delete", {
        id: "c-1",
        force: false,
      });
      expect(mockInvoke).toHaveBeenNthCalledWith(2, "container_list");
      expect(useContainerStore.getState().containers).toHaveLength(1);
      expect(useContainerStore.getState().containers[0].id).toBe("c-2");
      expect(useAppStore.getState().notifications.at(-1)?.title).toBe("Container deleted");
      expect(notificationText()).not.toMatch(/[\u4e00-\u9fff]/);
    });

    it("passes force when requested", async () => {
      const c = makeContainer({ id: "c-1" });
      useContainerStore.setState({ containers: [c] });
      mockInvoke.mockResolvedValueOnce(undefined).mockResolvedValueOnce([]);

      await useContainerStore.getState().deleteContainer("c-1", true);

      expect(mockInvoke).toHaveBeenNthCalledWith(1, "container_delete", {
        id: "c-1",
        force: true,
      });
    });

    it("clears selectedContainerId if deleted container was selected", async () => {
      const c = makeContainer({ id: "c-1" });
      useContainerStore.setState({ containers: [c], selectedContainerId: "c-1" });
      mockInvoke.mockResolvedValueOnce(undefined).mockResolvedValueOnce([]);

      await useContainerStore.getState().deleteContainer("c-1");

      expect(useContainerStore.getState().selectedContainerId).toBeNull();
    });

    it("preserves selectedContainerId if different container deleted", async () => {
      const c1 = makeContainer({ id: "c-1" });
      const c2 = makeContainer({ id: "c-2", name: "py-dev" });
      useContainerStore.setState({
        containers: [c1, c2],
        selectedContainerId: "c-2",
      });
      mockInvoke.mockResolvedValueOnce(undefined).mockResolvedValueOnce([c2]);

      await useContainerStore.getState().deleteContainer("c-1");

      expect(useContainerStore.getState().selectedContainerId).toBe("c-2");
    });

    it("rolls back and notifies when delete fails", async () => {
      const c = makeContainer({ id: "c-1" });
      useContainerStore.setState({ containers: [c] });
      mockInvoke.mockRejectedValueOnce(new Error("remove failed"));

      await expect(useContainerStore.getState().deleteContainer("c-1")).rejects.toThrow(
        "remove failed",
      );

      expect(useContainerStore.getState().containers).toEqual([c]);
      expect(useContainerStore.getState().error).toBe("remove failed");
      expect(useAppStore.getState().notifications.at(-1)?.title).toBe("Delete failed");
      expect(notificationText()).not.toMatch(/[\u4e00-\u9fff]/);
    });
  });

  // -------------------------------------------------------------------------
  // fetchTemplates
  // -------------------------------------------------------------------------
  describe("fetchTemplates", () => {
    it("populates templates from invoke", async () => {
      const templates = [makeTemplate(), makeTemplate({ id: "python-dev", name: "Python Dev" })];
      mockInvoke.mockResolvedValueOnce(templates);

      await useContainerStore.getState().fetchTemplates();

      expect(useContainerStore.getState().templates).toEqual(templates);
    });

    it("uses mock templates when invoke fails", async () => {
      mockInvoke.mockRejectedValueOnce(new Error("no Tauri"));

      await useContainerStore.getState().fetchTemplates();

      const templates = useContainerStore.getState().templates;
      expect(templates.length).toBeGreaterThanOrEqual(3);
      expect(templates.map((t) => t.id)).toEqual(
        expect.arrayContaining(["node-dev", "python-dev", "rust-dev"]),
      );
    });
  });

  // -------------------------------------------------------------------------
  // selectContainer
  // -------------------------------------------------------------------------
  describe("selectContainer", () => {
    it("sets selectedContainerId", () => {
      useContainerStore.getState().selectContainer("c-42");
      expect(useContainerStore.getState().selectedContainerId).toBe("c-42");
    });

    it("can clear selection with null", () => {
      useContainerStore.setState({ selectedContainerId: "c-42" });
      useContainerStore.getState().selectContainer(null);
      expect(useContainerStore.getState().selectedContainerId).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // filter + filteredContainers
  // -------------------------------------------------------------------------
  describe("filteredContainers", () => {
    const containers = [
      makeContainer({
        id: "c-1",
        name: "node-01",
        image: "node:20-slim",
        status: "running",
        labels: { "com.cratebay.template_id": "node-dev" },
      }),
      makeContainer({
        id: "c-2",
        name: "py-dev",
        image: "python:3.12-slim",
        status: "stopped",
        labels: { "com.cratebay.template_id": "python-dev" },
      }),
      makeContainer({
        id: "c-3",
        name: "rust-box",
        image: "rust:1.75-slim",
        status: "running",
        labels: { "com.cratebay.template_id": "rust-dev" },
      }),
    ];

    beforeEach(() => {
      useContainerStore.setState({ containers });
    });

    it("returns all containers with default filter", () => {
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(3);
    });

    it("filters by status", () => {
      useContainerStore.getState().setFilter({ status: "running" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(2);
      expect(filtered.every((c) => c.status === "running")).toBe(true);
    });

    it("filters by templateId", () => {
      useContainerStore.getState().setFilter({ templateId: "python-dev" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("py-dev");
    });

    it("filters by search term (name)", () => {
      useContainerStore.getState().setFilter({ search: "rust" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("rust-box");
    });

    it("filters by search term (case insensitive)", () => {
      useContainerStore.getState().setFilter({ search: "NODE" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("node-01");
    });

    it("combines multiple filters", () => {
      useContainerStore.getState().setFilter({ status: "running", search: "rust" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(1);
      expect(filtered[0].name).toBe("rust-box");
    });

    it("returns empty when no matches", () => {
      useContainerStore.getState().setFilter({ search: "nonexistent" });
      const filtered = useContainerStore.getState().filteredContainers();
      expect(filtered).toHaveLength(0);
    });
  });
});
