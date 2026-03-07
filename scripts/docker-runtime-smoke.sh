#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

cratebay_bin="$repo_root/target/debug/cratebay"
suffix="$(date +%s)-$$"
container_name="cbx-cli-smoke-${suffix}"
packaged_image="cbx-cli-pack:${suffix}"
volume_name="cbx-cli-volume-${suffix}"
search_query="${CRATEBAY_SMOKE_IMAGE_QUERY:-nginx}"
runtime_image="${CRATEBAY_SMOKE_RUNTIME_IMAGE:-nginx:1.27-alpine}"
env_key="CRATEBAY_E2E"
env_value="smoke-${suffix}"
container_removed=0
volume_removed=0

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command '$1' not found"
    exit 1
  fi
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

cleanup() {
  set +e
  if [[ "$container_removed" != "1" ]]; then
    docker rm -f "$container_name" >/dev/null 2>&1 || true
  fi
  docker image rm -f "$packaged_image" >/dev/null 2>&1 || true
  if [[ "$volume_removed" != "1" ]]; then
    docker volume rm -f "$volume_name" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

require_cmd cargo
require_cmd docker

if ! docker info >/dev/null 2>&1; then
  echo "ERROR: Docker daemon is not available"
  exit 1
fi

echo "== Build cratebay CLI =="
cargo build -p cratebay-cli >/dev/null

if [[ ! -x "$cratebay_bin" ]]; then
  echo "ERROR: built cratebay binary not found at $cratebay_bin"
  exit 1
fi

echo "== Image search =="
search_output="$($cratebay_bin image search "$search_query" --source dockerhub --limit 5)"
printf '%s\n' "$search_output"
assert_contains "$search_output" "$search_query" "image search should return the query term"

echo "== Run container =="
run_output="$($cratebay_bin docker run --pull --name "$container_name" -e "${env_key}=${env_value}" "$runtime_image")"
printf '%s\n' "$run_output"
assert_contains "$run_output" "$container_name" "docker run should report the created container"

echo "== Verify CLI container list =="
ps_output="$($cratebay_bin docker ps)"
printf '%s\n' "$ps_output"
assert_contains "$ps_output" "$container_name" "docker ps should list the created container"
assert_contains "$ps_output" "$runtime_image" "docker ps should show the runtime image"

echo "== Verify runtime state =="
running_state="$(docker inspect -f '{{.State.Running}}' "$container_name")"
if [[ "$running_state" != "true" ]]; then
  echo "ERROR: container $container_name is not running"
  exit 1
fi

docker exec "$container_name" /bin/sh -lc 'echo CRATEBAY_CONTAINER_OK' | grep -Fq 'CRATEBAY_CONTAINER_OK'

echo "== Verify env and login command =="
env_output="$($cratebay_bin docker env "$container_name")"
printf '%s\n' "$env_output"
assert_contains "$env_output" "$env_key" "docker env should include the injected env key"
assert_contains "$env_output" "$env_value" "docker env should include the injected env value"

login_output="$($cratebay_bin docker login-cmd "$container_name")"
printf '%s\n' "$login_output"
assert_contains "$login_output" "docker exec -it $container_name /bin/sh" "login-cmd should print the shell command"

echo "== Stop and start container =="
stop_output="$($cratebay_bin docker stop "$container_name")"
printf '%s\n' "$stop_output"
assert_contains "$stop_output" "Stopped container $container_name" "docker stop should succeed"

stopped_state="$(docker inspect -f '{{.State.Running}}' "$container_name")"
if [[ "$stopped_state" != "false" ]]; then
  echo "ERROR: container $container_name should be stopped"
  exit 1
fi

start_output="$($cratebay_bin docker start "$container_name")"
printf '%s\n' "$start_output"
assert_contains "$start_output" "Started container $container_name" "docker start should succeed"

echo "== Package container into image =="
pack_output="$($cratebay_bin image pack-container "$container_name" "$packaged_image")"
printf '%s\n' "$pack_output"
docker image inspect "$packaged_image" >/dev/null

image_list_output="$($cratebay_bin image list)"
printf '%s\n' "$image_list_output"
assert_contains "$image_list_output" "$packaged_image" "image list should include the packaged image"

image_inspect_output="$($cratebay_bin image inspect "$packaged_image")"
printf '%s\n' "$image_inspect_output"
assert_contains "$image_inspect_output" "$packaged_image" "image inspect should include the packaged tag"

echo "== Volume lifecycle =="
volume_create_output="$($cratebay_bin volume create "$volume_name")"
printf '%s\n' "$volume_create_output"
assert_contains "$volume_create_output" "$volume_name" "volume create should report the new volume"

docker volume inspect "$volume_name" >/dev/null

volume_list_output="$($cratebay_bin volume list)"
printf '%s\n' "$volume_list_output"
assert_contains "$volume_list_output" "$volume_name" "volume list should show the new volume"

volume_inspect_output="$($cratebay_bin volume inspect "$volume_name")"
printf '%s\n' "$volume_inspect_output"
assert_contains "$volume_inspect_output" "\"Name\": \"$volume_name\"" "volume inspect should show the created volume"

volume_remove_output="$($cratebay_bin volume remove "$volume_name")"
printf '%s\n' "$volume_remove_output"
assert_contains "$volume_remove_output" "$volume_name" "volume remove should report the deleted volume"
volume_removed=1

if docker volume inspect "$volume_name" >/dev/null 2>&1; then
  echo "ERROR: volume $volume_name still exists after removal"
  exit 1
fi

echo "== Remove container =="
rm_output="$($cratebay_bin docker rm "$container_name")"
printf '%s\n' "$rm_output"
assert_contains "$rm_output" "Removed container $container_name" "docker rm should remove the container"
container_removed=1

if docker inspect "$container_name" >/dev/null 2>&1; then
  echo "ERROR: container $container_name still exists after removal"
  exit 1
fi

echo "Docker runtime smoke: PASS"
