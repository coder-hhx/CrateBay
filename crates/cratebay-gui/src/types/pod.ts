/**
 * Pod-related types for CrateBay.
 */

export interface PodContainerInfo {
  id: string;
  name: string;
  ipv4Address?: string | null;
  ipv6Address?: string | null;
}

export interface PodInfo {
  id: string;
  name: string;
  driver: string;
  createdAt?: string | null;
  labels: Record<string, string>;
  containers: PodContainerInfo[];
}
