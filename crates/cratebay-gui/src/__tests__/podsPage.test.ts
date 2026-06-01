import { describe, expect, it, vi } from "vitest";
import type { ContainerInfo } from "@/types/container";
import type { PodContainerInfo } from "@/types/pod";

vi.mock("@/lib/tauri", () => ({
  invoke: vi.fn(),
}));

import { isContainerAttachedToPod } from "@/pages/PodsPage";

const makeContainer = (overrides: Partial<ContainerInfo> = {}): ContainerInfo => ({
  id: "abcdef1234567890",
  shortId: "abcdef123456",
  name: "node-01",
  image: "node:20-alpine",
  status: "running",
  state: "running",
  createdAt: "2026-03-23T00:00:00.000Z",
  ports: [],
  labels: {},
  ...overrides,
});

const makePodContainer = (
  overrides: Partial<PodContainerInfo> = {},
): PodContainerInfo => ({
  id: "abcdef1234567890",
  name: "node-01",
  ipv4Address: null,
  ipv6Address: null,
  ...overrides,
});

describe("PodsPage helpers", () => {
  it("matches pod membership by full or short container id", () => {
    const container = makeContainer();

    expect(isContainerAttachedToPod(container, [makePodContainer()])).toBe(true);
    expect(
      isContainerAttachedToPod(container, [
        makePodContainer({ id: "abcdef123456", name: "" }),
      ]),
    ).toBe(true);
  });

  it("matches pod membership by normalized container name", () => {
    const container = makeContainer({ id: "1111111111112222", shortId: "111111111111" });

    expect(
      isContainerAttachedToPod(container, [
        makePodContainer({ id: "3333333333334444", name: "/node-01" }),
      ]),
    ).toBe(true);
  });

  it("does not match unrelated short identities", () => {
    expect(
      isContainerAttachedToPod(makeContainer(), [
        makePodContainer({ id: "abcdef", name: "other" }),
      ]),
    ).toBe(false);
  });
});
