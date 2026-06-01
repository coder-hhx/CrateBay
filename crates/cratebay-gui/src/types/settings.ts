/**
 * Settings type definitions for CrateBay.
 */

export interface AppSettings {
  language: "en" | "zh-CN";
  theme: "dark" | "light" | "system";
  registryMirrors: string[];
  runtimeHttpProxy: string;
  runtimeHttpProxyBridge: boolean;
  runtimeHttpProxyBindHost: string;
  runtimeHttpProxyBindPort: number;
  runtimeHttpProxyGuestHost: string;
}

export const DEFAULT_REGISTRY_MIRRORS: string[] = [
  "docker.1ms.run",
  "docker.xuanyuan.me",
  "dockerhub.icu",
];

export function normalizeBaseUrl(url: string): string {
  return url.trim().replace(/\/+$/, "");
}
