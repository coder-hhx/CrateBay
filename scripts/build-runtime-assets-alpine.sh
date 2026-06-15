#!/usr/bin/env bash
set -euo pipefail

arch="${1:-}"
if [[ -z "$arch" ]]; then
  echo "Usage: $0 <aarch64|x86_64> [dest_dir]" >&2
  exit 2
fi

dest_dir="${2:-crates/cratebay-gui/src-tauri/runtime-images}"
alpine_version="${CRATEBAY_ALPINE_VERSION:-v3.19}"
# Alpine netboot kernel+initramfs flavor: `virt` is optimized for VMs and
# provides a working virtio console (console=hvc0) under Virtualization.framework.
netboot_flavor="${CRATEBAY_ALPINE_NETBOOT_FLAVOR:-virt}" # virt|lts
alpine_mirror="${CRATEBAY_ALPINE_MIRROR:-https://dl-cdn.alpinelinux.org/alpine}"
ubuntu_release="${CRATEBAY_UBUNTU_RELEASE:-24.04}"
ubuntu_series="${CRATEBAY_UBUNTU_SERIES:-noble}"
ubuntu_suite="${CRATEBAY_UBUNTU_SUITE:-noble-updates}"
ubuntu_ports_base="${CRATEBAY_UBUNTU_PORTS_BASE:-https://ports.ubuntu.com/ubuntu-ports}"
ubuntu_cloud_base="${CRATEBAY_UBUNTU_CLOUD_BASE:-https://cloud-images.ubuntu.com/releases}"

case "$arch" in
  aarch64|x86_64) ;;
  *)
    echo "ERROR: invalid arch '$arch' (expected aarch64 or x86_64)" >&2
    exit 2
    ;;
esac

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

if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 is required." >&2
  exit 1
fi

if ! command -v cpio >/dev/null 2>&1; then
  echo "ERROR: cpio is required." >&2
  exit 1
fi

if ! command -v gzip >/dev/null 2>&1; then
  echo "ERROR: gzip is required." >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "ERROR: curl is required." >&2
  exit 1
fi

if [[ "$arch" != "aarch64" ]] && ! command -v unsquashfs >/dev/null 2>&1; then
  echo "ERROR: unsquashfs (squashfs-tools) is required." >&2
  echo "  Install: brew install squashfs" >&2
  exit 1
fi

if [[ "$arch" == "aarch64" ]]; then
  if ! command -v ar >/dev/null 2>&1; then
    echo "ERROR: ar is required for Ubuntu kernel module extraction." >&2
    exit 1
  fi
  if ! command -v zstd >/dev/null 2>&1; then
    echo "ERROR: zstd is required for Ubuntu kernel module extraction." >&2
    echo "  Install: brew install zstd" >&2
    exit 1
  fi
  if ! command -v strings >/dev/null 2>&1; then
    echo "ERROR: strings is required for Ubuntu kernel metadata extraction." >&2
    exit 1
  fi
fi

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

cargo_cmd=(cargo)
if command -v rustup >/dev/null 2>&1; then
  cargo_cmd=(rustup run stable cargo)
fi

echo "== Build CrateBay guest binaries (${target_triple}) =="
if command -v cargo-zigbuild >/dev/null 2>&1; then
  "${cargo_cmd[@]}" zigbuild --release -p cratebay-guest-agent --target "$target_triple"
  "${cargo_cmd[@]}" zigbuild --release -p cratebay-engine-adapter --target "$target_triple"
else
  echo "ERROR: cargo-zigbuild is required to cross-compile guest binaries." >&2
  echo "  Install:" >&2
  echo "    brew install zig" >&2
  echo "    rustup target add ${target_triple}" >&2
  echo "    cargo install cargo-zigbuild" >&2
  exit 1
fi

guest_agent_bin="$repo_root/target/${target_triple}/release/cratebay-guest-agent"
if [[ ! -f "$guest_agent_bin" ]]; then
  echo "ERROR: guest agent binary not found: $guest_agent_bin" >&2
  exit 1
fi

cp "$guest_agent_bin" "$tmp_dir/cratebay-guest-agent"
chmod 0755 "$tmp_dir/cratebay-guest-agent"
engine_adapter_bin="$repo_root/target/${target_triple}/release/cratebay-engine-adapter"
if [[ ! -f "$engine_adapter_bin" ]]; then
  echo "ERROR: engine adapter binary not found: $engine_adapter_bin" >&2
  exit 1
fi

cp "$engine_adapter_bin" "$tmp_dir/cratebay-engine-adapter"
chmod 0755 "$tmp_dir/cratebay-engine-adapter"

echo ""
echo "== Download runtime kernel+initramfs (${arch}) =="
release_base="${alpine_mirror}/${alpine_version}/releases/${arch}/netboot"
curl -fL --retry 8 --retry-all-errors --retry-delay 2 --connect-timeout 20 -o "$tmp_dir/initramfs.gz" "${release_base}/initramfs-${netboot_flavor}"
if [[ "$arch" == "aarch64" ]]; then
  ubuntu_kernel_url="${ubuntu_cloud_base}/${ubuntu_release}/release/unpacked/ubuntu-${ubuntu_release}-server-cloudimg-arm64-vmlinuz-generic"
  curl -fL --retry 8 --retry-all-errors --retry-delay 2 --connect-timeout 20 -o "$tmp_dir/ubuntu-vmlinuz.gz" "$ubuntu_kernel_url"
  gzip -dc "$tmp_dir/ubuntu-vmlinuz.gz" >"$tmp_dir/vmlinuz"
else
  curl -fL --retry 8 --retry-all-errors --retry-delay 2 --connect-timeout 20 -o "$tmp_dir/vmlinuz" "${release_base}/vmlinuz-${netboot_flavor}"
  curl -fL --retry 8 --retry-all-errors --retry-delay 2 --connect-timeout 20 -o "$tmp_dir/modloop" "${release_base}/modloop-${netboot_flavor}"
fi

echo ""
echo "== Unpack initramfs =="
mkdir -p "$tmp_dir/initrd-root"
gzip -dc "$tmp_dir/initramfs.gz" | (cd "$tmp_dir/initrd-root" && cpio -idm --quiet)

echo ""
if [[ "$arch" == "aarch64" ]]; then
  echo "== Install Ubuntu aarch64 kernel modules for Apple Virtualization.framework =="
  python3 - "$tmp_dir" "$tmp_dir/initrd-root" "$ubuntu_ports_base" "$ubuntu_series" "$ubuntu_suite" "$tmp_dir/vmlinuz" <<'PY'
import gzip
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request

tmp_dir = pathlib.Path(sys.argv[1])
initrd_root = pathlib.Path(sys.argv[2])
ports_base = sys.argv[3].rstrip("/")
series = sys.argv[4]
suite = sys.argv[5]
kernel_path = pathlib.Path(sys.argv[6])

kernel_data = kernel_path.read_bytes()
match = re.search(rb"Linux version ([^ ]+)", kernel_data)
if not match:
    raise SystemExit("ERROR: failed to extract Ubuntu kernel release from downloaded Image")
kernel_release = match.group(1).decode("utf-8")

packages_needed = [
    f"linux-modules-{kernel_release}",
    f"linux-modules-extra-{kernel_release}",
]
candidate_suites = [suite, f"{series}-security", series]

pkg_meta = {}
for candidate in candidate_suites:
    url = f"{ports_base}/dists/{candidate}/main/binary-arm64/Packages.gz"
    try:
        with urllib.request.urlopen(url) as resp:
            raw = resp.read()
    except Exception:
        continue

    text = gzip.decompress(raw).decode("utf-8", "replace")
    for block in text.strip().split("\n\n"):
        lines = {}
        for line in block.splitlines():
            if ":" not in line:
                continue
            key, value = line.split(":", 1)
            lines[key.strip()] = value.strip()
        name = lines.get("Package")
        filename = lines.get("Filename")
        if name in packages_needed and filename:
            pkg_meta[name] = filename
    if all(name in pkg_meta for name in packages_needed):
        break

missing = [name for name in packages_needed if name not in pkg_meta]
if missing:
    raise SystemExit(
        "ERROR: failed to locate Ubuntu kernel module packages for "
        f"{kernel_release}: {', '.join(missing)}"
    )

download_dir = tmp_dir / "ubuntu-kmods"
download_dir.mkdir(parents=True, exist_ok=True)

extract_roots = []
for package_name in packages_needed:
    deb_path = download_dir / f"{package_name}.deb"
    urllib.request.urlretrieve(f"{ports_base}/{pkg_meta[package_name]}", deb_path)
    extract_dir = download_dir / package_name
    extract_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(["ar", "x", str(deb_path)], cwd=extract_dir, check=True)

    data_tar = None
    for candidate in extract_dir.iterdir():
        if candidate.name.startswith("data.tar"):
            data_tar = candidate
            break
    if data_tar is None:
        raise SystemExit(f"ERROR: {deb_path.name} did not contain a data.tar payload")

    subprocess.run(["tar", "-xf", str(data_tar)], cwd=extract_dir, check=True)
    modules_root = extract_dir / "lib" / "modules" / kernel_release
    if modules_root.is_dir():
        extract_roots.append(modules_root)

if not extract_roots:
    raise SystemExit(f"ERROR: Ubuntu module packages did not contain lib/modules/{kernel_release}")

module_paths = {}
for root in extract_roots:
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix not in {".zst", ".ko"}:
            continue
        name = path.name
        if name.endswith(".ko.zst"):
            module_name = name[:-7]
        elif name.endswith(".ko"):
            module_name = name[:-3]
        else:
            continue
        module_paths.setdefault(module_name, path)

required_modules = [
    "libcrc32c",
    "nfnetlink",
    "x_tables",
    "llc",
    "stp",
    "bridge",
    "br_netfilter",
    "nf_defrag_ipv4",
    "nf_defrag_ipv6",
    "nf_conntrack",
    "nf_nat",
    "nf_tables",
    "nft_compat",
    "nft_chain_nat",
    "nft_ct",
    "ip_tables",
    "iptable_nat",
    "iptable_filter",
    "iptable_mangle",
    "iptable_raw",
    "xt_conntrack",
    "xt_addrtype",
    "xt_MASQUERADE",
    "xt_nat",
    "xt_tcpudp",
    "xt_comment",
    "overlay",
    "fuse",
    "virtiofs",
    "vsock",
    "vmw_vsock_virtio_transport_common",
    "vmw_vsock_virtio_transport",
    "veth",
]

missing_required = [name for name in required_modules if name not in module_paths]
if missing_required:
    raise SystemExit(
        "ERROR: missing required Ubuntu modules for CrateBay Runtime: "
        + ", ".join(missing_required)
    )

module_bytes_cache = {}
depends_cache = {}

def read_module_bytes(path: pathlib.Path) -> bytes:
    key = str(path)
    cached = module_bytes_cache.get(key)
    if cached is not None:
        return cached

    if path.name.endswith(".zst"):
        payload = subprocess.check_output(["zstd", "-dc", str(path)])
    else:
        payload = path.read_bytes()
    module_bytes_cache[key] = payload
    return payload

def module_dependencies(name: str):
    cached = depends_cache.get(name)
    if cached is not None:
        return cached

    payload = read_module_bytes(module_paths[name])
    deps = []
    for match in re.finditer(rb"depends=([^\x00\n]*)", payload):
        raw = match.group(1).decode("utf-8", "replace").strip()
        if not raw:
            continue
        deps.extend(part.strip() for part in raw.split(",") if part.strip())
        break
    depends_cache[name] = deps
    return deps

ordered = []
visited = set()

def visit(name: str):
    if name in visited:
        return
    visited.add(name)
    for dep in module_dependencies(name):
        if dep in module_paths:
            visit(dep)
    ordered.append(name)

for module_name in required_modules:
    visit(module_name)

modules_dest_root = initrd_root / "lib" / "modules" / kernel_release
if modules_dest_root.parent.exists():
    shutil.rmtree(modules_dest_root.parent)
modules_dest_root.mkdir(parents=True, exist_ok=True)

load_list = []
for module_name in ordered:
    src = module_paths[module_name]
    src_root = next(root for root in extract_roots if src.is_relative_to(root))
    relative = src.relative_to(src_root)
    if relative.suffix == ".zst":
        relative = relative.with_suffix("")
    dest = modules_dest_root / relative
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(read_module_bytes(src))
    os.chmod(dest, 0o644)
    load_list.append(str(pathlib.Path("/lib/modules") / kernel_release / relative))

kmods_list = initrd_root / "etc" / "cratebay-kmods.list"
kmods_list.parent.mkdir(parents=True, exist_ok=True)
kmods_list.write_text("\n".join(load_list) + "\n", encoding="utf-8")
PY
else
  echo "== Extract kernel modules from modloop (squashfs) =="
  rm -rf "$tmp_dir/modloop-root"
  unsquashfs -f -d "$tmp_dir/modloop-root" "$tmp_dir/modloop" >/dev/null

  mod_ver="$(ls "$tmp_dir/modloop-root/modules" 2>/dev/null | head -n 1 || true)"
  if [[ -z "$mod_ver" ]]; then
    echo "ERROR: modloop did not contain modules/ directory." >&2
    exit 1
  fi

  rm -rf "$tmp_dir/initrd-root/lib/modules/$mod_ver"
  mkdir -p "$tmp_dir/initrd-root/lib/modules"
  cp -a "$tmp_dir/modloop-root/modules/$mod_ver" "$tmp_dir/initrd-root/lib/modules/"
fi

echo ""
echo "== Resolve Alpine package dependencies (CrateBay containerd engine) =="
python3 - "$alpine_version" "$arch" >"$tmp_dir/pkglist.txt" <<'PY'
import io
import os
import re
import sys
import tarfile
import time
import urllib.request

alpine_version = sys.argv[1]
arch = sys.argv[2]
alpine_mirror = os.environ.get("CRATEBAY_ALPINE_MIRROR", "https://dl-cdn.alpinelinux.org/alpine")

repos = [
    ("main", f"{alpine_mirror}/{alpine_version}/main/{arch}/APKINDEX.tar.gz"),
    ("community", f"{alpine_mirror}/{alpine_version}/community/{arch}/APKINDEX.tar.gz"),
]

pkg = {}  # name -> {"repo":..., "ver":..., "deps":[...], "provides":[...]}
provides = {}  # token -> set(names)

def fetch_index(url: str) -> str:
    last_error = None
    for attempt in range(1, 6):
        try:
            with urllib.request.urlopen(url, timeout=30) as resp:
                data = resp.read()
            break
        except Exception as exc:
            last_error = exc
            if attempt == 5:
                raise
            time.sleep(min(2 ** attempt, 10))
    else:
        raise last_error
    tf = tarfile.open(fileobj=io.BytesIO(data), mode="r:gz")
    raw = tf.extractfile("APKINDEX").read()
    return raw.decode("utf-8", "replace")

for repo, url in repos:
    idx = fetch_index(url)
    for block in idx.strip().split("\n\n"):
        m = re.search(r"^P:(.+)$", block, re.M)
        if not m:
            continue
        name = m.group(1).strip()
        mv = re.search(r"^V:(.+)$", block, re.M)
        if not mv:
            continue
        ver = mv.group(1).strip()
        mdeps = re.search(r"^D:(.+)$", block, re.M)
        deps = mdeps.group(1).split() if mdeps else []
        mprov = re.search(r"^p:(.+)$", block, re.M)
        prov = [t.split("=", 1)[0] for t in mprov.group(1).split()] if mprov else []

        pkg[name] = {"repo": repo, "ver": ver, "deps": deps, "provides": prov}
        for t in prov:
            provides.setdefault(t, set()).add(name)

def base_token(tok: str) -> str:
    # Strip version constraints; keep prefixes like so:/cmd:/pc: intact.
    for sep in (">=", "<=", ">", "<", "=", "~"):
        if sep in tok:
            return tok.split(sep, 1)[0]
    return tok

def resolve(tok: str):
    t = base_token(tok)
    if not t or t.startswith("/"):
        return None
    if t in pkg:
        return t
    if t in provides:
        return sorted(provides[t])[0]
    return None

roots = ["containerd", "containerd-ctr", "runc", "cni-plugins", "iptables", "iptables-legacy", "iproute2", "e2fsprogs", "e2fsprogs-extra"]
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

apk_dir="$tmp_dir/apks"
mkdir -p "$apk_dir"

download_apk() {
  local repo="$1"
  local name="$2"
  local version="$3"
  local out="$4"
  local url="${alpine_mirror}/${alpine_version}/${repo}/${arch}/${name}-${version}.apk"
  curl -fL --retry 8 --retry-all-errors --retry-delay 2 --connect-timeout 20 -o "$out" "$url"
}

echo ""
echo "== Download + extract Alpine packages =="
while IFS='|' read -r repo name version; do
  apk="$apk_dir/${name}-${version}.apk"
  echo "  - ${repo}/${name}-${version}.apk"
  download_apk "$repo" "$name" "$version" "$apk"
  tar -xf "$apk" -C "$tmp_dir/initrd-root"
done <"$tmp_dir/pkglist.txt"

echo ""
echo "== Install CrateBay guest binaries into initramfs =="
mkdir -p "$tmp_dir/initrd-root/usr/local/bin"
cp "$tmp_dir/cratebay-guest-agent" "$tmp_dir/initrd-root/usr/local/bin/cratebay-guest-agent"
chmod 0755 "$tmp_dir/initrd-root/usr/local/bin/cratebay-guest-agent"
cp "$tmp_dir/cratebay-engine-adapter" "$tmp_dir/initrd-root/usr/local/bin/cratebay-engine-adapter"
chmod 0755 "$tmp_dir/initrd-root/usr/local/bin/cratebay-engine-adapter"

echo ""
echo "== Write udhcpc DHCP script =="
mkdir -p "$tmp_dir/initrd-root/usr/share/udhcpc"
cat >"$tmp_dir/initrd-root/usr/share/udhcpc/default.script" <<'SH'
#!/bin/sh
set -eu

RESOLV_CONF=/etc/resolv.conf

mask_to_prefix() {
  local mask="$1"
  local prefix=0
  local octet

  old_ifs="$IFS"
  IFS=.
  set -- $mask
  IFS="$old_ifs"

  for octet in "$@"; do
    case "$octet" in
      255) prefix=$((prefix + 8)) ;;
      254) prefix=$((prefix + 7)) ;;
      252) prefix=$((prefix + 6)) ;;
      248) prefix=$((prefix + 5)) ;;
      240) prefix=$((prefix + 4)) ;;
      224) prefix=$((prefix + 3)) ;;
      192) prefix=$((prefix + 2)) ;;
      128) prefix=$((prefix + 1)) ;;
      0) ;;
      *) echo 24; return ;;
    esac
  done

  echo "$prefix"
}

case "$1" in
  deconfig)
    ip addr flush dev "$interface" || true
    ip link set "$interface" up || true
    ;;
  renew|bound)
    ip addr flush dev "$interface" || true
    if [ -n "${subnet:-}" ]; then
      ip addr add "$ip/$(mask_to_prefix "$subnet")" dev "$interface"
    elif [ -n "${mask:-}" ]; then
      ip addr add "$ip/$(mask_to_prefix "$mask")" dev "$interface"
    else
      ip addr add "$ip/24" dev "$interface"
    fi

    ip link set "$interface" up || true
    ip route del default dev "$interface" 2>/dev/null || true
    metric=0
    for router_ip in ${router:-}; do
      ip route add default via "$router_ip" dev "$interface" metric "$metric" 2>/dev/null || \
        ip route replace default via "$router_ip" dev "$interface" metric "$metric" || true
      metric=$((metric + 1))
    done

    : > "$RESOLV_CONF"
    for dns_ip in ${dns:-}; do
      echo "nameserver $dns_ip" >> "$RESOLV_CONF"
    done
    ;;
esac

exit 0
SH
chmod 0755 "$tmp_dir/initrd-root/usr/share/udhcpc/default.script"

echo ""
echo "== Write /init (runtime entrypoint) =="
cat >"$tmp_dir/initrd-root/init" <<'SH'
#!/bin/sh
set -eu

export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

# BusyBox applet links can shadow iproute2 in PATH. CrateBay pod networking
# needs `ip netns`, so point the adapter at the full iproute2 binary.
if [ -x /sbin/ip ]; then
  export CRATEBAY_IP=/sbin/ip
elif [ -x /usr/sbin/ip ]; then
  export CRATEBAY_IP=/usr/sbin/ip
fi

# Alpine netboot initramfs ships BusyBox without applet symlinks (only /bin/sh).
# Install symlinks so commands like `mkdir`/`mount` are available.
if [ -x /bin/busybox ]; then
  /bin/busybox --install -s /bin >/dev/null 2>&1 || true
  /bin/busybox --install -s /sbin >/dev/null 2>&1 || true
  /bin/busybox --install -s /usr/bin >/dev/null 2>&1 || true
  /bin/busybox --install -s /usr/sbin >/dev/null 2>&1 || true
fi

# Provide a stable /etc/os-release so the compatibility API can report OS info.
mkdir -p /etc /usr/lib
if [ ! -f /etc/os-release ]; then
  cat >/etc/os-release <<'EOF'
NAME="CrateBay Runtime"
ID=cratebay
VERSION_ID="0.1.0"
PRETTY_NAME="CrateBay Runtime"
EOF
fi
if [ ! -f /usr/lib/os-release ]; then
  ln -sf /etc/os-release /usr/lib/os-release || true
fi

log() {
  echo "[cratebay-runtime] $*"
  printf '<6>[cratebay-runtime] %s\n' "$*" >/dev/kmsg 2>/dev/null || true
}

cmdline_value() {
  key="$1"
  for arg in $(cat /proc/cmdline 2>/dev/null); do
    case "$arg" in
      "$key"=*)
        printf '%s' "${arg#*=}"
        return 0
        ;;
    esac
  done
  return 1
}

log "booting..."

mkdir -p /proc /sys /dev /run /tmp /var /var/lib /var/lib/cratebay-engine /sys/fs/cgroup

mount -t proc proc /proc || true
mount -t sysfs sysfs /sys || true
mount -t devtmpfs devtmpfs /dev || true
mkdir -p /dev/pts
mount -t devpts devpts /dev/pts || true

mount -t tmpfs tmpfs /run || true
mkdir -p /run/lock
ln -sf /run /var/run

host_epoch="$(cmdline_value cratebay_host_epoch || true)"
if [ -n "$host_epoch" ]; then
  if date -u -s "@${host_epoch}" >/dev/null 2>&1 \
    || date -u -D '%s' -s "${host_epoch}" >/dev/null 2>&1 \
    || /bin/busybox date -u -s "@${host_epoch}" >/dev/null 2>&1; then
    log "clock_sync=ok epoch=${host_epoch}"
  else
    log "WARN: failed to set clock from cratebay_host_epoch=${host_epoch}"
  fi
fi

# Try loading common virtio/kernel modules (ok if built-in).
#
# NOTE: Container networking requires bridge + veth + netfilter/NAT modules on
# some kernels. We load a broad set best-effort to reduce "engine starts but
# exits early" scenarios on minimal netboot kernels.
if [ -f /etc/cratebay-kmods.list ]; then
  while IFS= read -r module_path; do
    [ -n "$module_path" ] || continue
    [ -f "$module_path" ] || continue
    insmod "$module_path" >/dev/null 2>&1 || true
  done </etc/cratebay-kmods.list
fi
for m in \
  virtio_pci virtio_blk virtio_net virtio_vsock vmw_vsock_virtio_transport vsock \
  fuse virtiofs \
  overlay bridge veth br_netfilter \
  nf_tables nf_tables_inet nf_tables_ipv4 nf_tables_ipv6 nf_tables_bridge nft_compat nft_chain_nat nft_ct nft_counter \
  nf_conntrack nf_nat ip_tables iptable_nat iptable_filter iptable_mangle iptable_raw x_tables \
  xt_conntrack xt_addrtype xt_MASQUERADE xt_nat xt_tcpudp xt_comment \
  tun; do
  modprobe "$m" >/dev/null 2>&1 || true
done

# Basic vsock sanity signal (best-effort).
if [ -e /proc/net/vsock ]; then
  log "vsock: /proc/net/vsock present"
else
  log "WARN: vsock: /proc/net/vsock missing"
fi

# cgroup v2 setup (required for the CrateBay containerd engine).
if mount -t cgroup2 none /sys/fs/cgroup 2>/dev/null; then
  if [ -f /sys/fs/cgroup/cgroup.controllers ]; then
    mkdir -p /sys/fs/cgroup/init
    echo $$ > /sys/fs/cgroup/init/cgroup.procs 2>/dev/null || true

    ctrls="$(cat /sys/fs/cgroup/cgroup.controllers 2>/dev/null || true)"
    enable=""
    for c in $ctrls; do
      enable="$enable +$c"
    done
    if [ -n "$enable" ]; then
      echo "$enable" > /sys/fs/cgroup/cgroup.subtree_control 2>/dev/null || true
    fi
  fi
else
  log "WARN: failed to mount cgroup2"
fi

# Networking via DHCP (virtio-net + NAT).
ip link set lo up >/dev/null 2>&1 || true
iface="$(ls /sys/class/net 2>/dev/null | grep -v '^lo$' | head -n 1 || true)"
if [ -n "$iface" ]; then
  ip link set "$iface" up >/dev/null 2>&1 || true
  udhcpc -i "$iface" -q -t 5 -T 3 -n -s /usr/share/udhcpc/default.script >/dev/null 2>&1 || true

  # Surface the guest IP for the host-side runner (used for TCP forwarding).
  ip4="$(ip -4 -o addr show dev "$iface" scope global 2>/dev/null | awk '{print $4}' | cut -d/ -f1 | head -n 1 || true)"
  if [ -n "$ip4" ]; then
    log "guest_ip=${ip4}"
  fi

  default_gw="$(ip route show default 2>/dev/null | awk '/^default / {print $3; exit}' || true)"
  if [ -z "$default_gw" ]; then
    default_gw="$(cmdline_value cratebay_default_gw || true)"
  fi
  if [ -z "$default_gw" ]; then
    default_gw="192.168.64.1"
  fi
  ip route replace default via "$default_gw" dev "$iface" >/dev/null 2>&1 || true
  runtime_dns="$(cmdline_value cratebay_dns || true)"
  if [ -n "$runtime_dns" ]; then
    : > /etc/resolv.conf
    old_ifs="$IFS"
    IFS=,
    set -- $runtime_dns
    IFS="$old_ifs"
    for dns_ip in "$@"; do
      [ -n "$dns_ip" ] && echo "nameserver $dns_ip" >> /etc/resolv.conf
    done
  elif [ ! -s /etc/resolv.conf ]; then
    printf 'nameserver %s\n' "$default_gw" >/etc/resolv.conf
  fi
  log "net_debug routes=$(ip route 2>/dev/null | tr '\n' ';' || true)"
  log "net_debug resolv=$(tr '\n' ';' </etc/resolv.conf 2>/dev/null || true)"
  if command -v nc >/dev/null 2>&1; then
    if nc -z -w 2 1.1.1.1 443 >/dev/null 2>&1; then
      log "net_debug egress_1_1_1_1=ok"
    else
      log "net_debug egress_1_1_1_1=fail"
    fi
    if nc -z -w 2 quay.io 443 >/dev/null 2>&1; then
      log "net_debug egress_quay_443=ok"
    else
      log "net_debug egress_quay_443=fail"
    fi
  fi
fi

# Prepare persistent disk (/dev/vda) for CrateBay engine state.
if [ -b /dev/vda ]; then
  mkdir -p /var/lib/cratebay-engine
  /sbin/e2fsck -pf /dev/vda >/dev/null 2>&1 || true
  /usr/sbin/resize2fs /dev/vda >/dev/null 2>&1 || true
  if ! mount -t ext4 -o rw,noatime /dev/vda /var/lib/cratebay-engine 2>/dev/null; then
    log "formatting /dev/vda as ext4 (first boot)"
    mkfs.ext4 -F /dev/vda >/dev/null 2>&1 || mkfs.ext4 /dev/vda >/dev/null 2>&1 || true
    mount -t ext4 -o rw,noatime /dev/vda /var/lib/cratebay-engine 2>/dev/null || true
  fi
fi

# Mount host shares passed by the macOS VZ runner. CrateBay shares /Users by
# default so Docker/Compose bind mounts such as /Users/me/project:/workspace
# resolve inside the guest VM.
modprobe fuse >/tmp/cratebay-modprobe-fuse.log 2>&1 || true
modprobe virtiofs >/tmp/cratebay-modprobe-virtiofs.log 2>&1 || true
if grep -qw virtiofs /proc/filesystems 2>/dev/null; then
  log "virtiofs=available"
  mkdir -p /Users
  if mountpoint -q /Users 2>/dev/null; then
    log "mounted_share=Users:/Users already"
  elif mount_output="$(mount -t virtiofs Users /Users 2>&1)"; then
    log "mounted_share=Users:/Users"
  else
    log "WARN: failed to mount virtiofs share Users:/Users: ${mount_output}"
  fi
else
  fuse_err="$(tr '\n' ';' </tmp/cratebay-modprobe-fuse.log 2>/dev/null || true)"
  virtiofs_err="$(tr '\n' ';' </tmp/cratebay-modprobe-virtiofs.log 2>/dev/null || true)"
  log "WARN: virtiofs filesystem unavailable; fuse_modprobe=${fuse_err}; virtiofs_modprobe=${virtiofs_err}"
fi

# Ensure CA bundle exists (generated by the package's triggers on a real install).
if command -v update-ca-certificates >/dev/null 2>&1; then
  update-ca-certificates >/dev/null 2>&1 || true
fi

log "starting CrateBay containerd engine..."
# Best-effort sysctls commonly required for container networking.
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.bridge.bridge-nf-call-iptables=1 >/dev/null 2>&1 || true
sysctl -w net.bridge.bridge-nf-call-ip6tables=1 >/dev/null 2>&1 || true
if command -v iptables-legacy >/dev/null 2>&1; then
  ln -sf "$(command -v iptables-legacy)" /sbin/iptables
  ln -sf "$(command -v iptables-legacy-save)" /sbin/iptables-save 2>/dev/null || true
  ln -sf "$(command -v iptables-legacy-restore)" /sbin/iptables-restore 2>/dev/null || true
  log "iptables_backend=legacy"
fi
if command -v ip6tables-legacy >/dev/null 2>&1; then
  ln -sf "$(command -v ip6tables-legacy)" /sbin/ip6tables
  ln -sf "$(command -v ip6tables-legacy-save)" /sbin/ip6tables-save 2>/dev/null || true
  ln -sf "$(command -v ip6tables-legacy-restore)" /sbin/ip6tables-restore 2>/dev/null || true
fi

runtime_http_proxy="$(cmdline_value cratebay_http_proxy || true)"
runtime_engine="$(cmdline_value cratebay_runtime_engine || true)"
if [ -z "$runtime_engine" ]; then
  runtime_engine=containerd
fi
if [ "$runtime_engine" != "containerd" ]; then
  log "WARN: unsupported cratebay_runtime_engine=${runtime_engine}; falling back to containerd"
fi
engine_proxy_port="$(cmdline_value cratebay_engine_proxy_port || true)"
if [ -z "$engine_proxy_port" ]; then
  engine_proxy_port="$(cmdline_value cratebay_docker_proxy_port || true)"
fi
if [ -z "$engine_proxy_port" ]; then
  engine_proxy_port="${CRATEBAY_ENGINE_PROXY_PORT:-${CRATEBAY_ENGINE_VSOCK_PORT:-${CRATEBAY_DOCKER_PROXY_PORT:-${CRATEBAY_DOCKER_VSOCK_PORT:-6237}}}}"
fi
if [ -n "$runtime_http_proxy" ]; then
  export HTTP_PROXY="http://${runtime_http_proxy}"
  export HTTPS_PROXY="http://${runtime_http_proxy}"
  export http_proxy="$HTTP_PROXY"
  export https_proxy="$HTTPS_PROXY"
  export NO_PROXY="127.0.0.1,localhost,::1"
  export no_proxy="$NO_PROXY"
  log "engine_proxy=${HTTP_PROXY}"
fi

engine_root=/var/lib/cratebay-engine
containerd_root="${engine_root}/containerd"
containerd_state=/run/containerd
containerd_sock="${containerd_state}/containerd.sock"
adapter_sock=/run/cratebay/engine.sock
legacy_adapter_sock=/run/cratebay/docker.sock
cratebay_namespace="${CRATEBAY_CONTAINERD_NAMESPACE:-cratebay}"
mkdir -p "$containerd_root" "$engine_root/tmp" "$containerd_state" /run/cratebay /etc/containerd /etc/cni/net.d /var/lib/cni
chmod 1777 "$engine_root/tmp" || true
export TMPDIR="$engine_root/tmp"

cat >/etc/containerd/config.toml <<EOF
version = 2
root = "$containerd_root"
state = "$containerd_state"

[grpc]
  address = "$containerd_sock"

[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
  NoPivotRoot = true
EOF

cat >/etc/cni/net.d/10-cratebay.conflist <<'EOF'
{
  "cniVersion": "1.0.0",
  "name": "cratebay",
  "plugins": [
    {
      "type": "bridge",
      "bridge": "cratebay0",
      "isGateway": true,
      "ipMasq": true,
      "ipam": {
        "type": "host-local",
        "ranges": [[{ "subnet": "10.88.0.0/16" }]],
        "routes": [{ "dst": "0.0.0.0/0" }]
      }
    },
    {
      "type": "firewall"
    }
  ]
}
EOF

if command -v containerd >/dev/null 2>&1; then
  containerd \
    --config /etc/containerd/config.toml \
    --address "$containerd_sock" \
    --root "$containerd_root" \
    --state "$containerd_state" &
  containerd_pid="$!"
else
  containerd_pid=""
  log "ERROR: containerd binary is missing"
fi

for _ in $(seq 1 400); do
  [ -S "$containerd_sock" ] && break
  sleep 0.05
done
if [ -n "$containerd_pid" ] && ! kill -0 "$containerd_pid" >/dev/null 2>&1; then
  log "ERROR: containerd exited early"
elif [ ! -S "$containerd_sock" ]; then
  log "ERROR: containerd did not create $containerd_sock"
fi

if command -v ctr >/dev/null 2>&1 && [ -S "$containerd_sock" ]; then
  log "ctr version (address=$containerd_sock)"
  ctr --address "$containerd_sock" version 2>&1 | head -n 8 || true
fi

if [ -x /usr/local/bin/cratebay-engine-adapter ]; then
  log "starting cratebay-engine-adapter (${adapter_sock} -> ${containerd_sock})"
  CRATEBAY_CONTAINERD_SOCKET="$containerd_sock" \
  CRATEBAY_CONTAINERD_NAMESPACE="$cratebay_namespace" \
    /usr/local/bin/cratebay-engine-adapter \
      --socket "$adapter_sock" \
      --containerd-sock "$containerd_sock" \
      --namespace "$cratebay_namespace" &
  adapter_pid="$!"
else
  adapter_pid=""
  log "ERROR: cratebay-engine-adapter is missing"
fi

for _ in $(seq 1 400); do
  [ -S "$adapter_sock" ] && break
  sleep 0.05
done
if [ -n "$adapter_pid" ] && ! kill -0 "$adapter_pid" >/dev/null 2>&1; then
  log "ERROR: cratebay-engine-adapter exited early"
elif [ ! -S "$adapter_sock" ]; then
  log "ERROR: cratebay-engine-adapter did not create $adapter_sock"
fi
if [ -S "$adapter_sock" ]; then
  ln -sf "$adapter_sock" "$legacy_adapter_sock" 2>/dev/null || true
fi

if [ -x /usr/local/bin/cratebay-guest-agent ]; then
  if [ -e /proc/net/vsock ]; then
    log "starting cratebay-guest-agent (vsock -> ${adapter_sock})"
    /usr/local/bin/cratebay-guest-agent --port "${engine_proxy_port}" --engine-sock "$adapter_sock" &
    agent_pid="$!"
  else
    agent_pid=""
    log "skipping cratebay-guest-agent (vsock): no vsock device"
  fi

  # TCP connect mode: guest connects OUT to host's TCP forward listener.
  # This is the primary transport for VZ.framework on Intel Mac where vsock
  # is not available. The host's cratebay-vz --tcp-forward binds 0.0.0.0:PORT
  # and waits for the guest to connect back. We use the default gateway as
  # the host IP since VZ NAT routes through it.
  agent_connect_target="$(cmdline_value cratebay_agent_connect || true)"
  if [ -z "$agent_connect_target" ] && [ ! -e /proc/net/vsock ] && [ -n "$default_gw" ]; then
    agent_connect_target="${default_gw}:${engine_proxy_port}"
  fi
  if [ -n "$agent_connect_target" ]; then
    agent_connect_pool_size="$(cmdline_value cratebay_agent_connect_pool_size || true)"
    if [ -z "$agent_connect_pool_size" ]; then
      agent_connect_pool_size=32
    fi
    log "starting cratebay-guest-agent (tcp connect ${agent_connect_target} -> ${adapter_sock}, pool=${agent_connect_pool_size})"
    /usr/local/bin/cratebay-guest-agent \
      --connect "${agent_connect_target}" \
      --connect-pool-size "${agent_connect_pool_size}" \
      --engine-sock "$adapter_sock" &
    agent_connect_pid="$!"
  else
    agent_connect_pid=""
  fi

  # Also start TCP listen agent as fallback (e.g. for QEMU hostfwd or direct access).
  log "starting cratebay-guest-agent (tcp listen ${engine_proxy_port} -> ${adapter_sock})"
  /usr/local/bin/cratebay-guest-agent --tcp --port "${engine_proxy_port}" --engine-sock "$adapter_sock" &
  agent_tcp_pid="$!"

  sleep 0.2
  if [ -n "$agent_pid" ] && ! kill -0 "$agent_pid" >/dev/null 2>&1; then
    log "WARN: cratebay-guest-agent (vsock) exited early"
  fi
  if [ -n "$agent_connect_pid" ] && ! kill -0 "$agent_connect_pid" >/dev/null 2>&1; then
    log "WARN: cratebay-guest-agent (tcp connect) exited early"
  fi
  if ! kill -0 "$agent_tcp_pid" >/dev/null 2>&1; then
    log "WARN: cratebay-guest-agent (tcp listen) exited early"
  fi
fi

log "ready"
while true; do
  sleep 3600
done
SH
chmod 0755 "$tmp_dir/initrd-root/init"

echo ""
echo "== Repack initramfs =="
(
  cd "$tmp_dir/initrd-root"
  find . | cpio -o --format=newc --quiet | gzip -9
) >"$tmp_dir/initramfs.out"

echo ""
echo "== Write bundled runtime assets =="
image_id="cratebay-runtime-${arch}"
image_dir="${dest_dir}/${image_id}"

rm -rf "$image_dir"
mkdir -p "$image_dir"

mv "$tmp_dir/vmlinuz" "$image_dir/vmlinuz"
mv "$tmp_dir/initramfs.out" "$image_dir/initramfs"

# Optional: runtime lite is initramfs-first; rootfs.img is not required.
rm -f "$image_dir/rootfs.img"

echo "Runtime assets ready: ${image_dir}"
