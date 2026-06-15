#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

runtime_paths=(
  scripts/build-runtime-assets-alpine.sh
  scripts/build-runtime-assets-wsl.sh
  scripts/build-runtime-assets-linux.sh
  crates/cratebay-gui/src-tauri/runtime-images/README.md
  crates/cratebay-gui/src-tauri/runtime-wsl/README.md
  docs/specs/runtime-spec.md
)

runtime_script_paths=(
  scripts/build-runtime-assets-alpine.sh
  scripts/build-runtime-assets-wsl.sh
  scripts/build-runtime-assets-linux.sh
)

runtime_source_paths=(
  crates/cratebay-engine-adapter/src/main.rs
)

existing_paths=()
for path in "${runtime_paths[@]}"; do
  if [[ -e "$path" ]]; then
    existing_paths+=("$path")
  fi
done

existing_script_paths=()
for path in "${runtime_script_paths[@]}"; do
  if [[ -e "$path" ]]; then
    existing_script_paths+=("$path")
  fi
done

existing_source_paths=()
for path in "${runtime_source_paths[@]}"; do
  if [[ -e "$path" ]]; then
    existing_source_paths+=("$path")
  fi
done

if [[ ${#existing_paths[@]} -eq 0 ]]; then
  echo "Runtime native guard: no paths to scan"
  exit 0
fi

status=0

forbidden_runtime_pattern='(^|[^[:alnum:]_-])(dockerd|docker-engine|docker-openrc|docker-ce|docker-cli|docker\.io|moby-engine|moby-cli)([^[:alnum:]_-]|$)|/etc/init\.d/docker\b|rc-service[[:space:]]+docker\b|service[[:space:]]+docker\b|/var/lib/docker\b|/var/run/dockerd\.pid\b|exec[[:space:]]+dockerd\b|start[[:space:]]+dockerd\b'

if rg \
  --hidden \
  --line-number \
  --pcre2 \
  --regexp "$forbidden_runtime_pattern" \
  -- "${existing_paths[@]}"; then
  echo
  echo "Runtime native guard: found actual Docker daemon/package/service dependency in built-in runtime paths." >&2
  echo "The default CrateBay runtime must stay self-managed through CrateBay Engine + containerd/runc/CNI." >&2
  echo "Docker-compatible sockets, DOCKER_HOST overrides, Docker Hub references, and legacy field names are allowed only as compatibility surfaces." >&2
  status=1
fi

if [[ ${#existing_script_paths[@]} -gt 0 ]]; then
  package_install_pattern='(apk|apt-get|apt|dnf|yum|pacman|zypper|brew|choco)[[:space:]]+(add|install)[^\n]*(^|[[:space:]"'\''=])(docker|dockerd|moby)([[:space:]"'\''.]|$)'
  package_root_pattern='(^|[",[:space:]\[])(docker|dockerd|docker-engine|docker-openrc|docker-ce|docker-cli|docker\.io|moby-engine|moby-cli)([",[:space:]\]]|$)'

  if rg \
    --hidden \
    --line-number \
    --pcre2 \
    --regexp "$package_install_pattern" \
    -- "${existing_script_paths[@]}"; then
    echo
    echo "Runtime native guard: found package-manager installation of Docker/Moby in runtime asset scripts." >&2
    echo "Install containerd, runc, CNI, and CrateBay Engine components instead." >&2
    status=1
  fi

  if rg \
    --hidden \
    --line-number \
    --pcre2 \
    --regexp "$package_root_pattern" \
    -- "${existing_script_paths[@]}"; then
    echo
    echo "Runtime native guard: found Docker/Moby package root in runtime asset scripts." >&2
    echo "Runtime package roots must remain native CrateBay/containerd components." >&2
    status=1
  fi
fi

if [[ ${#existing_source_paths[@]} -gt 0 ]]; then
  source_dependency_pattern='(^|[^[:alnum:]_-])(dockerd|docker-engine|docker-openrc|docker-ce|docker-cli|moby-engine|moby-cli)([^[:alnum:]_-]|$)|/etc/init\.d/docker\b|rc-service[[:space:]]+docker\b|service[[:space:]]+docker\b|/var/lib/docker\b|/var/run/dockerd\.pid\b|exec[[:space:]]+dockerd\b|start[[:space:]]+dockerd\b'

  if rg \
    --hidden \
    --line-number \
    --pcre2 \
    --regexp "$source_dependency_pattern" \
    -- "${existing_source_paths[@]}"; then
    echo
    echo "Runtime native guard: found real Docker daemon/package/service dependency in native Engine source paths." >&2
    echo "Engine adapter code must stay backed by CrateBay Engine, containerd, runc, and CNI; Docker Hub image references and compatibility API names are allowed." >&2
    status=1
  fi
fi

if [[ "$status" -ne 0 ]]; then
  exit "$status"
fi

echo "Runtime native guard: PASS"
