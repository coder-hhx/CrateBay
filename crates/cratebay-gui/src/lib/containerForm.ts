import type { PortMapping, VolumeMount } from "@/types/container";

export function isBuiltInNetworkMode(value: string): boolean {
  return value === "bridge" || value === "none" || value === "host";
}

export function parsePortMapping(input: string): PortMapping {
  const spec = input.trim();
  if (!spec) throw new Error("empty");

  const slashIndex = spec.lastIndexOf("/");
  const portPart = slashIndex >= 0 ? spec.slice(0, slashIndex) : spec;
  const protocol = (slashIndex >= 0 ? spec.slice(slashIndex + 1) : "tcp").toLowerCase();
  if (protocol !== "tcp" && protocol !== "udp" && protocol !== "sctp") {
    throw new Error("protocol");
  }

  const parts = portPart.split(":");
  if (parts.length !== 1 && parts.length !== 2) {
    throw new Error("format");
  }
  const containerPort = parsePort(parts[parts.length - 1]);
  const hostPort = parts.length === 2 ? parsePort(parts[0]) : containerPort;

  return { hostPort, containerPort, protocol };
}

function parsePort(value: string): number {
  const port = Number(value.trim());
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error("port");
  }
  return port;
}

export function formatPortMapping(port: PortMapping): string {
  const prefix =
    port.hostPort === port.containerPort
      ? `${port.containerPort}`
      : `${port.hostPort}:${port.containerPort}`;
  return `${prefix}/${port.protocol}`;
}

export function parseVolumeMount(input: string): VolumeMount {
  const spec = input.trim();
  if (!spec) throw new Error("empty");

  const parts = spec.split(":");
  if (parts.length !== 2 && parts.length !== 3) {
    throw new Error("format");
  }
  const [hostPath, containerPath, mode] = parts.map((part) => part.trim());
  if (!hostPath || !containerPath || !containerPath.startsWith("/")) {
    throw new Error("path");
  }
  if (mode && mode !== "ro" && mode !== "rw") {
    throw new Error("mode");
  }

  return {
    hostPath,
    containerPath,
    readOnly: mode === "ro" ? true : mode === "rw" ? false : undefined,
  };
}

export function formatVolumeMount(volume: VolumeMount): string {
  const mode = volume.readOnly ? ":ro" : "";
  return `${volume.hostPath}:${volume.containerPath}${mode}`;
}

export function parseEnvVar(input: string): string {
  const spec = input.trim();
  if (!spec) throw new Error("empty");

  const separatorIndex = spec.indexOf("=");
  if (separatorIndex <= 0) {
    throw new Error("format");
  }

  const key = spec.slice(0, separatorIndex).trim();
  const value = spec.slice(separatorIndex + 1).trim();
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
    throw new Error("key");
  }

  return `${key}=${value}`;
}

export function envKey(envVar: string): string {
  return envVar.split("=", 1)[0];
}
