#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

suffix="$(date +%s)-$$"
container_name="cbx-runtime-smoke-${suffix}"
packaged_image="cbx-runtime-pack:${suffix}"
volume_name="cbx-runtime-volume-${suffix}"
runtime_image="${CRATEBAY_SMOKE_RUNTIME_IMAGE:-nginx:1.27-alpine}"
env_key="CRATEBAY_E2E"
env_value="smoke-${suffix}"

if [[ -x "$repo_root/target/debug/cratebay.exe" ]]; then
  cratebay_bin="$repo_root/target/debug/cratebay.exe"
else
  cratebay_bin="$repo_root/target/debug/cratebay"
fi

ready_runtime_file() {
  local file_path="$1"
  [[ -f "$file_path" ]] || return 1
  local file_size
  file_size="$(wc -c <"$file_path" | tr -d ' ')"
  if [[ "$file_size" -lt 1024 ]]; then
    if grep -Fq "PLACEHOLDER" "$file_path" 2>/dev/null; then
      return 1
    fi
    if grep -Fq "version https://git-lfs.github.com/spec/v1" "$file_path" 2>/dev/null; then
      return 1
    fi
  fi
  return 0
}

has_virtualization_entitlements() {
  local binary_path="$1"
  command -v codesign >/dev/null 2>&1 || return 1
  # Try both XML plist (:-) and human-readable (-) formats for macOS compat
  local output
  output="$(codesign -d --entitlements :- "$binary_path" 2>&1; codesign -d --entitlements - "$binary_path" 2>&1)"
  echo "$output" | grep -Fq "com.apple.security.virtualization"
}

prepare_macos_runtime() {
  local host_arch runtime_arch runner_path entitlements

  host_arch="$(uname -m)"
  runtime_arch="$host_arch"
  if [[ "$runtime_arch" == "arm64" ]]; then
    runtime_arch="aarch64"
  fi
  if [[ "$runtime_arch" != "aarch64" && "$runtime_arch" != "x86_64" ]]; then
    echo "ERROR: unsupported macOS arch '$host_arch'" >&2
    exit 1
  fi

  if ! ready_runtime_file "$repo_root/crates/cratebay-gui/src-tauri/runtime-images/cratebay-runtime-${runtime_arch}/vmlinuz" \
    || ! ready_runtime_file "$repo_root/crates/cratebay-gui/src-tauri/runtime-images/cratebay-runtime-${runtime_arch}/initramfs"; then
    echo "== Prepare macOS runtime assets (${runtime_arch}) =="
    bash "$repo_root/scripts/build-runtime-assets-alpine.sh" "$runtime_arch"
  fi

  runner_path="${CRATEBAY_VZ_RUNNER_PATH:-}"
  if [[ -z "$runner_path" ]]; then
    echo "== Build cratebay-vz =="
    cargo build -p cratebay-vz >/dev/null
    runner_path="$repo_root/target/debug/cratebay-vz"
  fi

  if [[ ! -x "$runner_path" ]]; then
    echo "ERROR: macOS VM runner not found: $runner_path" >&2
    exit 1
  fi

  if [[ "${CRATEBAY_SKIP_CODESIGN:-0}" != "1" ]] && command -v codesign >/dev/null 2>&1; then
    entitlements="$repo_root/scripts/macos-entitlements.plist"
    if [[ -f "$entitlements" ]]; then
      echo "== Codesign cratebay-vz for Virtualization.framework =="
      codesign --force --sign "${CRATEBAY_CODESIGN_IDENTITY:--}" --options runtime --entitlements "$entitlements" "$runner_path"
    else
      echo "WARN: entitlements plist not found: $entitlements" >&2
    fi
  fi

  if [[ "${CRATEBAY_SKIP_CODESIGN:-0}" != "1" ]] && ! has_virtualization_entitlements "$runner_path"; then
    echo "ERROR: macOS VM runner is missing virtualization entitlements: $runner_path" >&2
    exit 1
  fi

  export CRATEBAY_VZ_RUNNER_PATH="$runner_path"
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local message="$3"
  if ! printf '%s\n' "$haystack" | grep -Fq -- "$needle"; then
    echo "ASSERTION FAILED: $message"
    echo "--- output ---"
    printf '%s\n' "$haystack"
    exit 1
  fi
}

native_path() {
  local path="$1"
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$path"
  else
    printf '%s' "$path"
  fi
}

smoke_data_dir="$repo_root/target/runtime-smoke-${suffix}"
mkdir -p "$smoke_data_dir"
export CRATEBAY_DATA_DIR="$(native_path "$smoke_data_dir")"
# Keep smoke runs isolated from the user's normal runtime socket/port.
# macOS Unix sockets have a short path limit, so prefer a short /tmp socket path.
export CRATEBAY_DOCKER_SOCKET_PATH="/tmp/cratebay-smoke-${suffix}.sock"
export CRATEBAY_DOCKER_PROXY_PORT="$((42000 + (suffix % 10000)))"

cleanup() {
  set +e
  "$cratebay_bin" container delete "$container_name" --force >/dev/null 2>&1 || true
  if [[ "${OS:-}" == "Windows_NT" || -n "${MSYSTEM:-}" ]]; then
    for _ in {1..10}; do
      rm -rf "$smoke_data_dir" >/dev/null 2>&1 && break
      sleep 1
    done
  else
    rm -rf "$smoke_data_dir"
  fi
}
trap cleanup EXIT

if [[ "$(uname -s)" == "Darwin" ]]; then
  prepare_macos_runtime
fi

echo "== Build cratebay CLI =="
cargo build -p cratebay-cli >/dev/null

if [[ ! -x "$cratebay_bin" ]]; then
  echo "ERROR: built cratebay binary not found at $cratebay_bin"
  exit 1
fi

if [[ "${OS:-}" == "Windows_NT" || -n "${MSYSTEM:-}" ]]; then
  export CRATEBAY_RUNTIME_PROGRESS="${CRATEBAY_RUNTIME_PROGRESS:-1}"
fi

echo "== Bootstrap built-in runtime via container list =="
bootstrap_output="$("$cratebay_bin" container list --all)"
printf '%s\n' "$bootstrap_output"

echo "== Verify Docker status =="
docker_status_output="$("$cratebay_bin" system docker-status)"
printf '%s\n' "$docker_status_output"
assert_contains "$docker_status_output" "Docker: connected" "system docker-status should connect to built-in runtime"

echo "== Create container (auto-pull if missing) =="
create_output="$("$cratebay_bin" container create "$container_name" --image "$runtime_image" --env "${env_key}=${env_value}" --no-start)"
printf '%s\n' "$create_output"
assert_contains "$create_output" "$container_name" "container create should report the created container"

echo "== Verify container list =="
list_output="$("$cratebay_bin" container list --all)"
printf '%s\n' "$list_output"
assert_contains "$list_output" "$container_name" "container list should show the created container"
assert_contains "$list_output" "$runtime_image" "container list should show the runtime image"

echo "== Start container =="
start_output="$("$cratebay_bin" container start "$container_name")"
printf '%s\n' "$start_output"
assert_contains "$start_output" "Started $container_name" "container start should succeed"

echo "== Verify exec and logs =="
exec_output="$("$cratebay_bin" container exec "$container_name" -- printenv "$env_key")"
printf '%s\n' "$exec_output"
assert_contains "$exec_output" "$env_value" "container exec should see the injected env value"

logs_output="$("$cratebay_bin" container logs "$container_name" --tail 20 || true)"
printf '%s\n' "$logs_output"

echo "== Verify image list =="
image_list_output="$("$cratebay_bin" image list)"
printf '%s\n' "$image_list_output"
assert_contains "$image_list_output" "$runtime_image" "image list should include the runtime image"

echo "== Stop and delete container =="
stop_output="$("$cratebay_bin" container stop "$container_name")"
printf '%s\n' "$stop_output"
assert_contains "$stop_output" "Stopped $container_name" "container stop should succeed"

delete_output="$("$cratebay_bin" container delete "$container_name")"
printf '%s\n' "$delete_output"
assert_contains "$delete_output" "Deleted $container_name" "container delete should succeed"

echo "CLI-only runtime smoke: PASS"
