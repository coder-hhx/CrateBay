#!/usr/bin/env bash
set -euo pipefail

arch="${1:-}"
if [[ -z "$arch" ]]; then
  echo "Usage: $0 <aarch64|x86_64> [dest_dir]" >&2
  exit 2
fi

case "$arch" in
  aarch64|x86_64) ;;
  *)
    echo "ERROR: invalid arch '$arch' (expected aarch64 or x86_64)" >&2
    exit 2
    ;;
esac

dest_dir="${2:-crates/cratebay-gui/src-tauri/runtime-wsl}"
alpine_version="${CRATEBAY_ALPINE_VERSION:-v3.19}"
minirootfs_version="${CRATEBAY_ALPINE_MINIROOTFS_VERSION:-3.19.0}"
alpine_mirror="${CRATEBAY_ALPINE_MIRROR:-https://dl-cdn.alpinelinux.org/alpine}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

target_triple=""
case "$arch" in
  x86_64) target_triple="x86_64-unknown-linux-musl" ;;
  aarch64) target_triple="aarch64-unknown-linux-musl" ;;
esac

# Ensure Cargo-installed tools are on PATH.
if [[ -d "$HOME/.cargo/bin" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck source=/dev/null
  source "$HOME/.cargo/env"
fi
if command -v rustup >/dev/null 2>&1; then
  rustup_cargo="$(rustup which cargo 2>/dev/null || true)"
  if [[ -n "$rustup_cargo" ]]; then
    export PATH="$(dirname "$rustup_cargo"):$PATH"
  fi
fi

python_cmd=""
if command -v python3 >/dev/null 2>&1; then
  python_cmd="python3"
elif command -v python >/dev/null 2>&1; then
  python_cmd="python"
else
  echo "ERROR: python3 or python is required." >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "ERROR: curl is required." >&2
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  echo "ERROR: tar is required." >&2
  exit 1
fi

cargo_cmd=(cargo)
if command -v rustup >/dev/null 2>&1; then
  cargo_cmd=(rustup run stable cargo)
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

image_id="cratebay-runtime-wsl-${arch}"
image_dir="${dest_dir}/${image_id}"
rootfs_dir="$tmp_dir/rootfs"
apk_dir="$tmp_dir/apks"
apk_cache_dir="${CRATEBAY_ALPINE_APK_CACHE:-$repo_root/target/cratebay-alpine-apk-cache/${alpine_version}/${arch}}"
mkdir -p "$image_dir" "$rootfs_dir" "$apk_dir" "$apk_cache_dir"
rm -f "$image_dir/rootfs.tar"

echo "== Build CrateBay WSL guest binaries (${target_triple}) =="
if command -v cargo-zigbuild >/dev/null 2>&1; then
  "${cargo_cmd[@]}" zigbuild --release -p cratebay-guest-agent --target "$target_triple"
  "${cargo_cmd[@]}" zigbuild --release -p cratebay-engine-adapter --target "$target_triple"
else
  echo "ERROR: cargo-zigbuild is required to cross-compile WSL guest binaries." >&2
  echo "  Install:" >&2
  echo "    brew install zig" >&2
  echo "    rustup target add ${target_triple}" >&2
  echo "    cargo install cargo-zigbuild" >&2
  exit 1
fi

guest_agent_bin="$repo_root/target/${target_triple}/release/cratebay-guest-agent"
engine_adapter_bin="$repo_root/target/${target_triple}/release/cratebay-engine-adapter"
if [[ ! -f "$guest_agent_bin" ]]; then
  echo "ERROR: guest agent binary not found: $guest_agent_bin" >&2
  exit 1
fi
if [[ ! -f "$engine_adapter_bin" ]]; then
  echo "ERROR: engine adapter binary not found: $engine_adapter_bin" >&2
  exit 1
fi

download() {
  local url="$1"
  local out="$2"
  echo "Downloading ${url} -> ${out}"
  curl -fL --retry 8 --retry-all-errors --retry-delay 2 --connect-timeout 20 -o "${out}" "${url}"
}

download_cached() {
  local url="$1"
  local out="$2"

  if [[ -s "$out" ]]; then
    echo "Using cached ${out}"
    return
  fi

  mkdir -p "$(dirname "$out")"
  local part="${out}.part"
  echo "Downloading ${url} -> ${out}"
  curl -fL --retry 8 --retry-all-errors --retry-delay 2 --connect-timeout 20 -C - -o "$part" "$url"
  mv "$part" "$out"
}

echo "== Download Alpine minirootfs (${alpine_version}, ${arch}) =="
release_base="${alpine_mirror}/${alpine_version}/releases/${arch}"
download \
  "${release_base}/alpine-minirootfs-${minirootfs_version}-${arch}.tar.gz" \
  "$tmp_dir/minirootfs.tar.gz"
tar -xzf "$tmp_dir/minirootfs.tar.gz" -C "$rootfs_dir"

echo ""
echo "== Resolve Alpine package dependencies (containerd + CrateBay Engine + CNI) =="
"$python_cmd" - "$alpine_version" "$arch" "$alpine_mirror" >"$tmp_dir/pkglist.txt" <<'PY'
import io
import re
import sys
import tarfile
import time
import urllib.request

alpine_version = sys.argv[1]
arch = sys.argv[2]
alpine_mirror = sys.argv[3].rstrip("/")

repos = [
    ("main", f"{alpine_mirror}/{alpine_version}/main/{arch}/APKINDEX.tar.gz"),
    ("community", f"{alpine_mirror}/{alpine_version}/community/{arch}/APKINDEX.tar.gz"),
]

pkg = {}
provides = {}

def fetch_index(url: str) -> str:
    last_exc = None
    for attempt in range(1, 6):
        try:
            with urllib.request.urlopen(url, timeout=30) as resp:
                data = resp.read()
            break
        except Exception as exc:
            last_exc = exc
            if attempt == 5:
                raise
            time.sleep(2)
    else:
        raise last_exc
    tf = tarfile.open(fileobj=io.BytesIO(data), mode="r:gz")
    raw = tf.extractfile("APKINDEX").read()
    return raw.decode("utf-8", "replace")

for repo, url in repos:
    idx = fetch_index(url)
    for block in idx.strip().split("\n\n"):
        name_match = re.search(r"^P:(.+)$", block, re.M)
        if not name_match:
            continue
        version_match = re.search(r"^V:(.+)$", block, re.M)
        if not version_match:
            continue
        deps_match = re.search(r"^D:(.+)$", block, re.M)
        provides_match = re.search(r"^p:(.+)$", block, re.M)

        name = name_match.group(1).strip()
        version = version_match.group(1).strip()
        deps = deps_match.group(1).split() if deps_match else []
        provided = [token.split("=", 1)[0] for token in provides_match.group(1).split()] if provides_match else []

        pkg[name] = {"repo": repo, "ver": version, "deps": deps, "provides": provided}
        for token in provided:
            provides.setdefault(token, set()).add(name)

def base_token(token: str) -> str:
    for sep in (">=", "<=", ">", "<", "=", "~"):
        if sep in token:
            return token.split(sep, 1)[0]
    return token

def resolve(token: str):
    token = base_token(token)
    if not token or token.startswith("/"):
        return None
    if token in pkg:
        return token
    if token in provides:
        return sorted(provides[token])[0]
    return None

roots = [
    "containerd",
    "containerd-ctr",
    "containerd-openrc",
    "runc",
    "cni-plugins",
    "iptables",
    "e2fsprogs",
    "curl",
    "openrc",
    "iproute2",
    "procps-ng",
    "util-linux",
    "ca-certificates",
]

want = set()
stack = list(roots)

while stack:
    name = stack.pop()
    if name in want:
        continue
    if name not in pkg:
        print(f"ERROR: package not found in index: {name}", file=sys.stderr)
        sys.exit(2)
    want.add(name)
    for dep in pkg[name]["deps"]:
        resolved = resolve(dep)
        if resolved and resolved not in want:
            stack.append(resolved)

for name in sorted(want):
    info = pkg[name]
    print(f"{info['repo']}|{name}|{info['ver']}")
PY

echo "Resolved $(wc -l <"$tmp_dir/pkglist.txt" | tr -d ' ') packages."

download_apk() {
  local repo="$1"
  local name="$2"
  local version="$3"
  local out="$4"
  local url="${alpine_mirror}/${alpine_version}/${repo}/${arch}/${name}-${version}.apk"
  local cached="${apk_cache_dir}/${repo}/${name}-${version}.apk"
  download_cached "$url" "$cached"
  cp "$cached" "$out"
}

echo ""
echo "== Download + extract Alpine packages =="
while IFS='|' read -r repo name version; do
  apk="$apk_dir/${name}-${version}.apk"
  echo "  - ${repo}/${name}-${version}.apk"
  download_apk "$repo" "$name" "$version" "$apk"
  tar -xf "$apk" -C "$rootfs_dir"
done <"$tmp_dir/pkglist.txt"

find "$rootfs_dir" -maxdepth 1 \
  \( -name '.PKGINFO' \
  -o -name '.pre-install' \
  -o -name '.post-install' \
  -o -name '.post-upgrade' \
  -o -name '.post-deinstall' \
  -o -name '.trigger' \
  -o -name '.SIGN.*' \) \
  -delete

echo ""
echo "== Write WSL runtime configuration =="
mkdir -p \
  "$rootfs_dir/etc/cni/net.d" \
  "$rootfs_dir/etc/conf.d" \
  "$rootfs_dir/etc/network" \
  "$rootfs_dir/etc/profile.d" \
  "$rootfs_dir/etc/runlevels/default" \
  "$rootfs_dir/run" \
  "$rootfs_dir/run/openrc" \
  "$rootfs_dir/usr/local/bin" \
  "$rootfs_dir/var" \
  "$rootfs_dir/var/lib/containerd" \
  "$rootfs_dir/var/lib/cratebay-engine" \
  "$rootfs_dir/var/log"

cp "$guest_agent_bin" "$rootfs_dir/usr/local/bin/cratebay-guest-agent"
chmod 0755 "$rootfs_dir/usr/local/bin/cratebay-guest-agent"
cp "$engine_adapter_bin" "$rootfs_dir/usr/local/bin/cratebay-engine-adapter"
chmod 0755 "$rootfs_dir/usr/local/bin/cratebay-engine-adapter"

cat >"$rootfs_dir/etc/wsl.conf" <<'CONF'
[boot]
systemd=false

[interop]
appendWindowsPath=false

[automount]
enabled=true
mountFsTab=false
options=metadata,uid=0,gid=0,umask=022,fmask=0111
CONF

cat >"$rootfs_dir/etc/rc.conf" <<'CONF'
rc_cgroup_mode="unified"
unicode="YES"
CONF

cat >"$rootfs_dir/etc/network/interfaces" <<'CONF'
auto lo
iface lo inet loopback
CONF

cat >"$rootfs_dir/etc/profile.d/cratebay.sh" <<'SH'
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
SH
chmod 0755 "$rootfs_dir/etc/profile.d/cratebay.sh"

cat >"$rootfs_dir/usr/local/bin/cratebay-wsl-engine" <<'SH'
#!/bin/sh
set -eu

containerd_state="${CRATEBAY_CONTAINERD_STATE:-/run/containerd}"
containerd_root="${CRATEBAY_CONTAINERD_ROOT:-/var/lib/cratebay-engine/containerd}"
containerd_sock="${CRATEBAY_CONTAINERD_SOCKET:-${containerd_state}/containerd.sock}"
adapter_sock="${CRATEBAY_ENGINE_ADAPTER_SOCKET:-/run/cratebay/engine.sock}"
legacy_adapter_sock="/run/cratebay/docker.sock"
namespace="${CRATEBAY_CONTAINERD_NAMESPACE:-cratebay}"
proxy_port="${CRATEBAY_ENGINE_PROXY_PORT:-${CRATEBAY_DOCKER_PROXY_PORT:-2375}}"

log() {
  printf '%s %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*" >>/var/log/cratebay-engine.log
}

write_cni_config() {
  mkdir -p /etc/cni/net.d /var/lib/cratebay-engine/cni
  if [ ! -f /etc/cni/net.d/10-cratebay-bridge.conflist ]; then
    cat >/etc/cni/net.d/10-cratebay-bridge.conflist <<'JSON'
{
  "cniVersion": "1.0.0",
  "name": "bridge",
  "plugins": [
    {
      "type": "bridge",
      "bridge": "cratebay0",
      "isGateway": true,
      "ipMasq": true,
      "hairpinMode": true,
      "ipam": {
        "type": "host-local",
        "ranges": [[{ "subnet": "10.88.0.0/16", "gateway": "10.88.0.1" }]],
        "routes": [{ "dst": "0.0.0.0/0" }]
      }
    },
    { "type": "portmap", "capabilities": { "portMappings": true } },
    { "type": "firewall" },
    { "type": "tuning" }
  ]
}
JSON
  fi
}

write_containerd_config() {
  mkdir -p /etc/containerd "$containerd_root" "$containerd_state"
  cat >/etc/containerd/config.toml <<EOF
version = 2
root = "$containerd_root"
state = "$containerd_state"

[grpc]
  address = "$containerd_sock"

[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
  NoPivotRoot = true
EOF
}

pid_running() {
  pid="$1"
  [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

start_containerd() {
  mkdir -p "$containerd_state" "$containerd_root" /var/log
  write_containerd_config
  if [ -S "$containerd_sock" ] && command -v ctr >/dev/null 2>&1 && ctr --address "$containerd_sock" version >/dev/null 2>&1; then
    return 0
  fi
  if command -v rc-service >/dev/null 2>&1 && [ -x /etc/init.d/containerd ]; then
    rc-service containerd start >>/var/log/containerd.log 2>&1 || true
    sleep 1
    if [ -S "$containerd_sock" ]; then
      return 0
    fi
  fi
  if [ -f /run/cratebay-containerd.pid ] && pid_running "$(cat /run/cratebay-containerd.pid 2>/dev/null)"; then
    return 0
  fi
  nohup containerd --config /etc/containerd/config.toml --address "$containerd_sock" --root "$containerd_root" --state "$containerd_state" >>/var/log/containerd.log 2>&1 &
  echo "$!" >/run/cratebay-containerd.pid
}

wait_for_containerd() {
  deadline=$(( $(date +%s) + 60 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if [ -S "$containerd_sock" ]; then
      return 0
    fi
    sleep 1
  done
  echo "containerd socket was not ready: $containerd_sock" >&2
  return 1
}

start_adapter() {
  mkdir -p /run/cratebay /var/log
  if [ -S "$adapter_sock" ] && curl -fsS --unix-socket "$adapter_sock" http://localhost/_ping 2>/dev/null | grep -q OK; then
    return 0
  fi
  rm -f "$adapter_sock"
  if [ -f /run/cratebay-engine-adapter.pid ] && pid_running "$(cat /run/cratebay-engine-adapter.pid 2>/dev/null)"; then
    kill "$(cat /run/cratebay-engine-adapter.pid)" 2>/dev/null || true
  fi
  CRATEBAY_CONTAINERD_SOCKET="$containerd_sock" \
  CRATEBAY_CONTAINERD_NAMESPACE="$namespace" \
    nohup /usr/local/bin/cratebay-engine-adapter \
      --socket "$adapter_sock" \
      --containerd-sock "$containerd_sock" \
      --namespace "$namespace" \
      >>/var/log/cratebay-engine-adapter.log 2>&1 &
  echo "$!" >/run/cratebay-engine-adapter.pid
}

wait_for_adapter() {
  deadline=$(( $(date +%s) + 60 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if [ -S "$adapter_sock" ] && curl -fsS --unix-socket "$adapter_sock" http://localhost/_ping 2>/dev/null | grep -q OK; then
      ln -sf "$adapter_sock" "$legacy_adapter_sock" 2>/dev/null || true
      return 0
    fi
    sleep 1
  done
  echo "CrateBay Engine adapter was not ready: $adapter_sock" >&2
  return 1
}

start_proxy() {
  if [ -f /run/cratebay-guest-agent.pid ] && pid_running "$(cat /run/cratebay-guest-agent.pid 2>/dev/null)"; then
    return 0
  fi
  nohup /usr/local/bin/cratebay-guest-agent --tcp --port "$proxy_port" --engine-sock "$adapter_sock" \
    >>/var/log/cratebay-guest-agent.log 2>&1 &
  echo "$!" >/run/cratebay-guest-agent.pid
}

stop_pidfile() {
  path="$1"
  if [ -f "$path" ]; then
    pid="$(cat "$path" 2>/dev/null || true)"
    if pid_running "$pid"; then
      kill "$pid" 2>/dev/null || true
    fi
    rm -f "$path"
  fi
}

case "${1:-start}" in
  start)
    log "starting CrateBay WSL Engine"
    write_cni_config
    write_containerd_config
    start_containerd
    wait_for_containerd
    start_adapter
    wait_for_adapter
    start_proxy
    log "CrateBay WSL Engine ready on tcp :${proxy_port}"
    ;;
  stop)
    stop_pidfile /run/cratebay-guest-agent.pid
    stop_pidfile /run/cratebay-engine-adapter.pid
    if command -v rc-service >/dev/null 2>&1 && [ -x /etc/init.d/containerd ]; then
      rc-service containerd stop >/dev/null 2>&1 || true
    fi
    stop_pidfile /run/cratebay-containerd.pid
    pkill -x cratebay-guest-agent 2>/dev/null || true
    pkill -x cratebay-engine-adapter 2>/dev/null || true
    pkill -x containerd 2>/dev/null || true
    rm -f "$adapter_sock" "$legacy_adapter_sock"
    ;;
  ping)
    curl -fsS --unix-socket "$adapter_sock" http://localhost/_ping 2>/dev/null | tr -d '\r' || true
    ;;
  status)
    printf 'containerd_socket=%s\n' "$containerd_sock"
    printf 'adapter_socket=%s\n' "$adapter_sock"
    printf 'proxy_port=%s\n' "$proxy_port"
    printf 'namespace=%s\n' "$namespace"
    if [ -S "$adapter_sock" ]; then
      printf 'ping=%s\n' "$(/usr/local/bin/cratebay-wsl-engine ping)"
    else
      printf 'ping=\n'
    fi
    ;;
  *)
    echo "Usage: cratebay-wsl-engine [start|stop|ping|status]" >&2
    exit 2
    ;;
esac
SH
chmod 0755 "$rootfs_dir/usr/local/bin/cratebay-wsl-engine"

if [[ -f "$rootfs_dir/etc/init.d/containerd" ]]; then
  perl -0pi -e 's/need sysfs cgroups/after sysfs cgroups/' "$rootfs_dir/etc/init.d/containerd"
fi

for svc in cgroups containerd; do
  if [[ -e "$rootfs_dir/etc/init.d/$svc" ]]; then
    ln -sf "../../init.d/$svc" "$rootfs_dir/etc/runlevels/default/$svc"
  fi
done

echo ""
echo "== Pack deterministic WSL rootfs.tar =="
"$python_cmd" - "$rootfs_dir" "$image_dir/rootfs.tar" <<'PY'
import os
import sys
import tarfile

root = os.path.abspath(sys.argv[1])
out = sys.argv[2]

with tarfile.open(out, "w") as tar:
    for current, dirs, files in os.walk(root, topdown=True, followlinks=False):
        dirs.sort()
        files.sort()

        rel_current = os.path.relpath(current, root)
        if rel_current != ".":
            info = tar.gettarinfo(current, arcname=rel_current)
            info.uid = 0
            info.gid = 0
            info.uname = "root"
            info.gname = "root"
            info.mtime = 0
            tar.addfile(info)

        for name in files:
            path = os.path.join(current, name)
            arcname = os.path.relpath(path, root)
            info = tar.gettarinfo(path, arcname=arcname)
            info.uid = 0
            info.gid = 0
            info.uname = "root"
            info.gname = "root"
            info.mtime = 0
            if info.isreg():
                with open(path, "rb") as fp:
                    tar.addfile(info, fp)
            else:
                tar.addfile(info)
PY

echo "WSL runtime assets ready: ${image_dir}"
