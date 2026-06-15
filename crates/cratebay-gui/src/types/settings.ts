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
  includePrereleases: boolean;
}

export const DEFAULT_REGISTRY_MIRRORS: string[] = [
  "docker.1ms.run",
  "docker.xuanyuan.me",
  "dockerhub.icu",
];

export const DEFAULT_RUNTIME_HTTP_PROXY = "";
export const DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE = false;
export const DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST = "0.0.0.0";
export const DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT = 3128;
export const DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST = "192.168.64.1";

export function normalizeBaseUrl(url: string): string {
  return url.trim().replace(/\/+$/, "");
}
