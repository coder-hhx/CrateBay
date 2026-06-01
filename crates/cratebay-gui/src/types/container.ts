/**
 * Container type definitions for CrateBay.
 *
 * Matches frontend-spec.md §4.3 — containerStore types.
 */
import type { LocalImageInfo } from "@/types/image";

/**
 * Container information returned from the backend.
 */
export interface ContainerInfo {
  id: string;
  shortId: string;
  name: string;
  image: string;
  status: "running" | "stopped" | "creating" | "exited" | "paused" | "restarting" | "removing" | "dead" | "created";
  state: string;
  createdAt: string;
  cpuCores?: number;
  memoryMb?: number;
  ports: PortMapping[];
  labels: Record<string, string>;
}

/**
 * Request payload for creating a new container.
 */
export interface ContainerCreateRequest {
  name: string;
  image: string;
  templateId?: string;
  entrypoint?: string;
  command?: string;
  env?: string[];
  ports?: PortMapping[];
  volumes?: VolumeMount[];
  cpuCores?: number;
  memoryMb?: number;
  workingDir?: string;
  pod?: string;
  network?: "bridge" | "none" | "host";
  user?: string;
  readOnlyRootfs?: boolean;
  autoStart?: boolean;
}

/**
 * Container template definition.
 */
export interface ContainerTemplate {
  id: string;
  name: string;
  description: string;
  image: string;
  defaultCommand: string;
  defaultCpuCores: number;
  defaultMemoryMb: number;
  tags: string[];
}

/**
 * Filter criteria for container list.
 */
export interface ContainerFilter {
  status: "all" | "running" | "stopped" | "creating";
  search: string;
  templateId: string | null;
}

/**
 * Port mapping between host and container.
 */
export interface PortMapping {
  hostPort: number;
  containerPort: number;
  protocol: "tcp" | "udp";
}

/**
 * Bind mount between host and container.
 */
export interface VolumeMount {
  hostPath: string;
  containerPath: string;
  readOnly?: boolean;
}

/**
 * Container status change event from the backend.
 */
export interface ContainerStatusEvent {
  containerId: string;
  status: "running" | "stopped" | "error";
  message?: string;
}

export type DockerImageInfo = LocalImageInfo;

/**
 * Container log line event from the backend.
 */
export interface ContainerLogEvent {
  containerId: string;
  line: string;
  stream: "stdout" | "stderr";
  timestamp: string;
}
