import { useCallback } from "react";
import { useContainerStore } from "@/stores/containerStore";
import type { ContainerCreateRequest } from "@/types/container";

/**
 * Hook providing container CRUD operations with loading state management.
 * Wraps containerStore operations with additional UI convenience.
 */
export function useContainerActions() {
  const fetchContainers = useContainerStore((s) => s.fetchContainers);
  const fetchTemplates = useContainerStore((s) => s.fetchTemplates);
  const createContainer = useContainerStore((s) => s.createContainer);
  const startContainer = useContainerStore((s) => s.startContainer);
  const stopContainer = useContainerStore((s) => s.stopContainer);
  const deleteContainer = useContainerStore((s) => s.deleteContainer);
  const loading = useContainerStore((s) => s.loading);
  const error = useContainerStore((s) => s.error);

  const refresh = useCallback(async () => {
    await Promise.all([fetchContainers(), fetchTemplates()]);
  }, [fetchContainers, fetchTemplates]);

  const create = useCallback(
    async (req: ContainerCreateRequest) => {
      const container = await createContainer(req);
      return container;
    },
    [createContainer],
  );

  const start = useCallback(
    async (id: string) => {
      await startContainer(id);
    },
    [startContainer],
  );

  const stop = useCallback(
    async (id: string) => {
      await stopContainer(id);
    },
    [stopContainer],
  );

  const remove = useCallback(
    async (id: string, force = false) => {
      await deleteContainer(id, force);
    },
    [deleteContainer],
  );

  return {
    refresh,
    create,
    start,
    stop,
    remove,
    loading,
    error,
  };
}
