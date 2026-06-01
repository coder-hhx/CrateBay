#!/usr/bin/env bash
set -euo pipefail

# End-to-end smoke for the minimum supported product shape: cratebay CLI plus
# the built-in runtime. By default it builds a tiny local Linux image and imports
# it with `cratebay image import`, so the smoke does not depend on Docker Hub.
#
# Useful overrides:
#   CRATEBAY_SMOKE_RUNTIME_IMAGE=image:tag     Use a registry/local image instead.
#   CRATEBAY_SMOKE_OFFLINE_IMAGE=0            Skip local smoke image generation.
#   CRATEBAY_SMOKE_LOCAL_REGISTRY=1           Start a local registry container and
#                                              pull the smoke image back from it.
#   CRATEBAY_SMOKE_FALLBACK_RUNTIME_IMAGE=... Fallback image when offline build is unavailable.
#   CRATEBAY_KEEP_SMOKE_TEMP=1                Keep generated image archives for debugging.
#   CRATEBAY_KEEP_SMOKE_DATA=1                Keep the isolated smoke runtime data dir.

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

suffix="$(date +%s)-$$"
container_name="cbx-runtime-smoke-${suffix}"
pod_name="cbx-runtime-pod-${suffix}"
packaged_image="cbx-runtime-pack:${suffix}"
tagged_image="cbx-runtime-pack:${suffix}-tag"
volume_name="cbx-runtime-volume-${suffix}"
runtime_image="${CRATEBAY_SMOKE_RUNTIME_IMAGE:-}"
offline_smoke_image=0
env_key="CRATEBAY_E2E"
env_value="smoke-${suffix}"
container_removed=0
pod_removed=0
volume_removed=0
registry_container_name=""
registry_server_image=""
registry_seed_dir=""
registry_ref=""

if [[ -x "$repo_root/target/debug/cratebay.exe" ]]; then
  cratebay_bin="$repo_root/target/debug/cratebay.exe"
else
  cratebay_bin="$repo_root/target/debug/cratebay"
fi
TEMP_DIR="$(mktemp -d)"

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

prepare_linux_runtime() {
  local host_arch runtime_arch qemu_bin image_dir helper_dir

  host_arch="$(uname -m)"
  runtime_arch="$host_arch"
  case "$runtime_arch" in
    amd64) runtime_arch="x86_64" ;;
    arm64) runtime_arch="aarch64" ;;
  esac
  if [[ "$runtime_arch" != "x86_64" && "$runtime_arch" != "aarch64" ]]; then
    echo "ERROR: unsupported Linux arch '$host_arch'" >&2
    exit 1
  fi

  if [[ "$runtime_arch" == "aarch64" ]]; then
    qemu_bin="qemu-system-aarch64"
  else
    qemu_bin="qemu-system-x86_64"
  fi

  image_dir="$repo_root/crates/cratebay-gui/src-tauri/runtime-images/cratebay-runtime-${runtime_arch}"
  if ! ready_runtime_file "$image_dir/vmlinuz" \
    || ! ready_runtime_file "$image_dir/initramfs"; then
    echo "== Prepare Linux runtime image assets (${runtime_arch}) =="
    bash "$repo_root/scripts/build-runtime-assets-alpine.sh" "$runtime_arch"
  fi

  helper_dir="$repo_root/crates/cratebay-gui/src-tauri/runtime-linux/cratebay-runtime-linux-${runtime_arch}"
  if ! ready_runtime_file "$helper_dir/$qemu_bin"; then
    echo "== Prepare Linux runtime helper assets (${runtime_arch}) =="
    bash "$repo_root/scripts/build-runtime-assets-linux.sh" "$runtime_arch"
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

sha256_file() {
  local file_path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" | awk '{print $1}'
  else
    shasum -a 256 "$file_path" | awk '{print $1}'
  fi
}

create_offline_smoke_image() {
  local host_arch target_triple docker_arch
  host_arch="$(uname -m)"
  case "$host_arch" in
    x86_64|amd64)
      target_triple="x86_64-unknown-linux-musl"
      docker_arch="amd64"
      ;;
    arm64|aarch64)
      target_triple="aarch64-unknown-linux-musl"
      docker_arch="arm64"
      ;;
    *)
      echo "WARN: unsupported host arch for offline smoke image: $host_arch" >&2
      return 1
      ;;
  esac

  local -a rustc_cmd
  rustc_cmd=(rustc)
  if ! command -v rustc >/dev/null 2>&1 && ! command -v rustup >/dev/null 2>&1; then
    echo "WARN: rustc not found; falling back to registry image for smoke" >&2
    return 1
  fi
  if command -v rustup >/dev/null 2>&1; then
    local toolchain
    toolchain="$(rustup show active-toolchain | awk '{print $1}')"
    if ! rustup target list --installed | grep -Fqx "$target_triple"; then
      echo "WARN: Rust target $target_triple is not installed; falling back to registry image for smoke" >&2
      return 1
    fi
    rustc_cmd=(rustup run "$toolchain" rustc)
  fi

  local image_name source_file smoke_bin rootfs_dir image_dir layer_dir layer_tar layer_digest
  local config_tmp config_digest config_name archive import_output list_output
  local image_repo image_tag
  image_name="cratebay-runtime-smoke:local-${suffix}"
  image_repo="${image_name%:*}"
  image_tag="${image_name##*:}"
  source_file="$TEMP_DIR/cratebay-smoke.rs"
  smoke_bin="$TEMP_DIR/cratebay-smoke"
  rootfs_dir="$TEMP_DIR/rootfs"
  image_dir="$TEMP_DIR/image"
  config_tmp="$image_dir/config.json.tmp"
  archive="$TEMP_DIR/cratebay-runtime-smoke-image.tar"

  cat >"$source_file" <<'RS'
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("-c") {
        args = args
            .get(1)
            .map(|command| command.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
    }

    match args.first().map(String::as_str) {
        Some("serve") => serve_app(),
        Some("registry") => {
            let state_dir = arg_value(&args, "--state-dir")
                .or_else(|| arg_value(&args, "--state"))
                .unwrap_or_else(|| "/registry".to_string());
            let listen = arg_value(&args, "--listen")
                .unwrap_or_else(|| "0.0.0.0:5000".to_string());
            if let Err(e) = serve_registry(Path::new(&state_dir), &listen) {
                eprintln!("cratebay-smoke: registry error: {}", e);
                process::exit(1);
            }
        }
        Some("env") => {
            let key = args.get(1).map(String::as_str).unwrap_or_default();
            println!("{}", env::var(key).unwrap_or_default());
        }
        Some("exists") => {
            let path = args.get(1).map(String::as_str).unwrap_or_default();
            if Path::new(path).exists() {
                println!("exists {}", path);
            } else {
                eprintln!("missing {}", path);
                process::exit(66);
            }
        }
        Some("echo") => {
            println!("{}", args.iter().skip(1).cloned().collect::<Vec<_>>().join(" "));
        }
        Some(other) => {
            eprintln!("unknown command: {}", other);
            process::exit(64);
        }
        None => println!("cratebay-smoke"),
    }
}

fn serve_app() {
    println!("cratebay-smoke ready");
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

struct RegistryState {
    repo: String,
    tag: String,
    manifest: Vec<u8>,
    manifest_digest: String,
    config: Vec<u8>,
    config_digest: String,
    layer: Vec<u8>,
    layer_digest: String,
}

fn serve_registry(state_dir: &Path, listen: &str) -> Result<(), String> {
    let repo = fs::read_to_string(state_dir.join("repo.txt"))
        .map_err(|e| format!("read repo.txt: {}", e))?
        .trim()
        .to_string();
    let tag = fs::read_to_string(state_dir.join("tag.txt"))
        .map_err(|e| format!("read tag.txt: {}", e))?
        .trim()
        .to_string();
    let manifest = fs::read(state_dir.join("manifest.json"))
        .map_err(|e| format!("read manifest.json: {}", e))?;
    let manifest_digest = fs::read_to_string(state_dir.join("manifest.digest"))
        .map_err(|e| format!("read manifest.digest: {}", e))?
        .trim()
        .to_string();
    let config = fs::read(state_dir.join("config.json"))
        .map_err(|e| format!("read config.json: {}", e))?;
    let config_digest = fs::read_to_string(state_dir.join("config.digest"))
        .map_err(|e| format!("read config.digest: {}", e))?
        .trim()
        .to_string();
    let layer = fs::read(state_dir.join("layer.tar"))
        .map_err(|e| format!("read layer.tar: {}", e))?;
    let layer_digest = fs::read_to_string(state_dir.join("layer.digest"))
        .map_err(|e| format!("read layer.digest: {}", e))?
        .trim()
        .to_string();

    let listener = TcpListener::bind(listen).map_err(|e| format!("bind {}: {}", listen, e))?;
    eprintln!("cratebay-smoke registry ready on {}", listen);
    let state = Arc::new(RegistryState {
        repo,
        tag,
        manifest,
        manifest_digest,
        config,
        config_digest,
        layer,
        layer_digest,
    });

    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                let state = state.clone();
                thread::spawn(move || {
                    let _ = handle_registry_connection(stream, state);
                });
            }
            Err(e) => eprintln!("cratebay-smoke registry accept error: {}", e),
        }
    }

    Ok(())
}

fn handle_registry_connection(stream: TcpStream, state: Arc<RegistryState>) -> Result<(), String> {
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("clone stream: {}", e))?;
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("read request line: {}", e))?;
    if request_line.trim().is_empty() {
        return Ok(());
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let raw_path = parts.next().unwrap_or("/").to_string();

    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|e| format!("read request headers: {}", e))?;
        if bytes == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }

    let path = raw_path.split('?').next().unwrap_or(raw_path.as_str());
    let is_head = method.eq_ignore_ascii_case("HEAD");
    let response = if path == "/v2" || path == "/v2/" {
        registry_response(
            "200 OK",
            "application/json",
            b"{}",
            vec![
                ("Docker-Distribution-API-Version", "registry/2.0"),
                ("Cache-Control", "no-cache"),
            ],
            is_head,
        )
    } else if path == format!("/v2/{}/tags/list", state.repo) {
        let body = format!(
            "{{\"name\":\"{}\",\"tags\":[\"{}\"]}}",
            state.repo, state.tag
        );
        registry_response(
            "200 OK",
            "application/json",
            body.as_bytes(),
            vec![
                ("Docker-Distribution-API-Version", "registry/2.0"),
                ("Cache-Control", "no-cache"),
            ],
            is_head,
        )
    } else if path == format!("/v2/{}/manifests/{}", state.repo, state.tag)
        || path == format!("/v2/{}/manifests/{}", state.repo, state.manifest_digest)
    {
        registry_response(
            "200 OK",
            "application/vnd.docker.distribution.manifest.v2+json",
            &state.manifest,
            vec![
                ("Docker-Distribution-API-Version", "registry/2.0"),
                ("Docker-Content-Digest", &state.manifest_digest),
                ("Cache-Control", "no-cache"),
            ],
            is_head,
        )
    } else if path == format!("/v2/{}/blobs/{}", state.repo, state.config_digest) {
        registry_response(
            "200 OK",
            "application/vnd.docker.container.image.v1+json",
            &state.config,
            vec![
                ("Docker-Distribution-API-Version", "registry/2.0"),
                ("Docker-Content-Digest", &state.config_digest),
                ("Cache-Control", "no-cache"),
            ],
            is_head,
        )
    } else if path == format!("/v2/{}/blobs/{}", state.repo, state.layer_digest) {
        registry_response(
            "200 OK",
            "application/vnd.docker.image.rootfs.diff.tar",
            &state.layer,
            vec![
                ("Docker-Distribution-API-Version", "registry/2.0"),
                ("Docker-Content-Digest", &state.layer_digest),
                ("Cache-Control", "no-cache"),
            ],
            is_head,
        )
    } else {
        let body = format!(
            "{{\"errors\":[{{\"code\":\"NAME_UNKNOWN\",\"message\":\"unsupported path: {}\"}}]}}",
            path
        );
        registry_response(
            "404 Not Found",
            "application/json",
            body.as_bytes(),
            vec![("Docker-Distribution-API-Version", "registry/2.0")],
            is_head,
        )
    };

    writer
        .write_all(&response)
        .and_then(|_| writer.flush())
        .map_err(|e| format!("write response: {}", e))?;
    Ok(())
}

fn registry_response(
    status: &str,
    content_type: &str,
    body: &[u8],
    mut extra_headers: Vec<(&str, &str)>,
    head_only: bool,
) -> Vec<u8> {
    let mut response = String::new();
    response.push_str(&format!("HTTP/1.1 {}\r\n", status));
    response.push_str(&format!("Content-Type: {}\r\n", content_type));
    response.push_str(&format!("Content-Length: {}\r\n", body.len()));
    response.push_str("Connection: close\r\n");
    for (key, value) in extra_headers.drain(..) {
        response.push_str(&format!("{}: {}\r\n", key, value));
    }
    response.push_str("\r\n");
    let mut response = response.into_bytes();
    if !head_only {
        response.extend_from_slice(body);
    }
    response
}
RS

  echo "== Build offline smoke image binary ($target_triple) =="
  if ! "${rustc_cmd[@]}" \
    --target "$target_triple" \
    -C linker=rust-lld \
    -C opt-level=z \
    -C panic=abort \
    -C strip=symbols \
    -o "$smoke_bin" \
    "$source_file"; then
    echo "WARN: failed to build offline smoke binary; falling back to registry image" >&2
    return 1
  fi

  mkdir -p "$rootfs_dir/usr/local/bin" "$image_dir"
  cp "$smoke_bin" "$rootfs_dir/usr/local/bin/cratebay-smoke"
  mkdir -p "$rootfs_dir/bin"
  ln -s /usr/local/bin/cratebay-smoke "$rootfs_dir/bin/sh"
  chmod 0755 "$rootfs_dir/usr/local/bin/cratebay-smoke"
  if command -v xattr >/dev/null 2>&1; then
    xattr -c "$rootfs_dir/usr/local/bin/cratebay-smoke" >/dev/null 2>&1 || true
  fi

  local raw_layer_tar
  raw_layer_tar="$TEMP_DIR/layer.tar"
  COPYFILE_DISABLE=1 tar -C "$rootfs_dir" -cf "$raw_layer_tar" .
  layer_digest="$(sha256_file "$raw_layer_tar")"
  layer_dir="$image_dir/$layer_digest"
  layer_tar="$layer_dir/layer.tar"
  mkdir -p "$layer_dir"
  mv "$raw_layer_tar" "$layer_tar"
  printf '1.0' >"$layer_dir/VERSION"
  cat >"$layer_dir/json" <<JSON
{
  "id": "$layer_digest",
  "created": "2026-01-01T00:00:00Z",
  "container_config": {
    "Cmd": ["/usr/local/bin/cratebay-smoke", "serve"]
  }
}
JSON

  cat >"$config_tmp" <<JSON
{
  "created": "2026-01-01T00:00:00Z",
  "architecture": "$docker_arch",
  "os": "linux",
  "config": {
    "Env": ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"],
    "Cmd": ["/usr/local/bin/cratebay-smoke", "serve"],
    "WorkingDir": "/"
  },
  "rootfs": {
    "type": "layers",
    "diff_ids": ["sha256:$layer_digest"]
  },
  "history": [
    {
      "created": "2026-01-01T00:00:00Z",
      "created_by": "cratebay runtime smoke"
    }
  ]
}
JSON

  config_digest="$(sha256_file "$config_tmp")"
  config_name="${config_digest}.json"
  mv "$config_tmp" "$image_dir/$config_name"

  local registry_repo registry_tag registry_manifest_digest registry_manifest_size config_size layer_size
  registry_repo="cratebay-smoke"
  registry_tag="local"
  registry_ref="127.0.0.1:5000/${registry_repo}:${registry_tag}"
  registry_seed_dir="$TEMP_DIR/registry-seed"
  registry_server_image="cratebay-runtime-smoke-registry:local-${suffix}"
  mkdir -p "$registry_seed_dir"
  printf '%s\n' "$registry_repo" >"$registry_seed_dir/repo.txt"
  printf '%s\n' "$registry_tag" >"$registry_seed_dir/tag.txt"
  cp "$image_dir/$config_name" "$registry_seed_dir/config.json"
  cp "$layer_tar" "$registry_seed_dir/layer.tar"
  printf '%s\n' "$config_digest" >"$registry_seed_dir/config.digest"
  printf '%s\n' "$layer_digest" >"$registry_seed_dir/layer.digest"

  config_size="$(wc -c <"$registry_seed_dir/config.json" | tr -d ' ')"
  layer_size="$(wc -c <"$registry_seed_dir/layer.tar" | tr -d ' ')"

  cat >"$registry_seed_dir/manifest.json" <<JSON
{
  "schemaVersion": 2,
  "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
  "config": {
    "mediaType": "application/vnd.docker.container.image.v1+json",
    "size": $config_size,
    "digest": "sha256:$config_digest"
  },
  "layers": [
    {
      "mediaType": "application/vnd.docker.image.rootfs.diff.tar",
      "size": $layer_size,
      "digest": "sha256:$layer_digest"
    }
  ]
}
JSON

  registry_manifest_digest="sha256:$(sha256_file "$registry_seed_dir/manifest.json")"
  registry_manifest_size="$(wc -c <"$registry_seed_dir/manifest.json" | tr -d ' ')"
  printf '%s\n' "$registry_manifest_digest" >"$registry_seed_dir/manifest.digest"

  cat >"$image_dir/manifest.json" <<JSON
[
  {
    "Config": "$config_name",
    "RepoTags": ["$image_name"],
    "Layers": ["$layer_digest/layer.tar"]
  }
]
JSON

  cat >"$image_dir/repositories" <<JSON
{
  "$image_repo": {
    "$image_tag": "$layer_digest"
  }
}
JSON

  COPYFILE_DISABLE=1 tar -C "$image_dir" -cf "$archive" manifest.json repositories "$config_name" "$layer_digest"

  echo "== Import offline smoke image =="
  import_output="$("$cratebay_bin" image import "$archive")"
  printf '%s\n' "$import_output"

  list_output="$("$cratebay_bin" image list)"
  assert_contains "$list_output" "$image_name" "offline smoke image should be available after import"

  if [[ "${CRATEBAY_SMOKE_LOCAL_REGISTRY:-0}" == "1" ]]; then
    echo "== Import local registry server image =="
    local registry_image_dir registry_config_tmp registry_config_digest registry_config_name registry_archive
    local registry_import_output registry_rootfs_dir registry_raw_layer_tar registry_layer_digest registry_layer_dir registry_layer_tar
    registry_image_dir="$TEMP_DIR/registry-image"
    registry_rootfs_dir="$TEMP_DIR/registry-rootfs"
    registry_config_tmp="$registry_image_dir/config.json.tmp"
    registry_archive="$TEMP_DIR/cratebay-runtime-smoke-registry-image.tar"
    registry_raw_layer_tar="$TEMP_DIR/registry-layer.tar"

    mkdir -p "$registry_rootfs_dir/usr/local/bin" "$registry_rootfs_dir/bin" "$registry_rootfs_dir/registry-seed"
    cp "$smoke_bin" "$registry_rootfs_dir/usr/local/bin/cratebay-smoke"
    ln -s /usr/local/bin/cratebay-smoke "$registry_rootfs_dir/bin/sh"
    chmod 0755 "$registry_rootfs_dir/usr/local/bin/cratebay-smoke"
    if command -v xattr >/dev/null 2>&1; then
      xattr -c "$registry_rootfs_dir/usr/local/bin/cratebay-smoke" >/dev/null 2>&1 || true
    fi
    cp "$registry_seed_dir/repo.txt" \
      "$registry_seed_dir/tag.txt" \
      "$registry_seed_dir/config.json" \
      "$registry_seed_dir/layer.tar" \
      "$registry_seed_dir/config.digest" \
      "$registry_seed_dir/layer.digest" \
      "$registry_seed_dir/manifest.json" \
      "$registry_seed_dir/manifest.digest" \
      "$registry_rootfs_dir/registry-seed/"

    COPYFILE_DISABLE=1 tar -C "$registry_rootfs_dir" -cf "$registry_raw_layer_tar" .
    registry_layer_digest="$(sha256_file "$registry_raw_layer_tar")"
    registry_layer_dir="$registry_image_dir/$registry_layer_digest"
    registry_layer_tar="$registry_layer_dir/layer.tar"
    mkdir -p "$registry_layer_dir"
    mv "$registry_raw_layer_tar" "$registry_layer_tar"
    printf '1.0' >"$registry_layer_dir/VERSION"
    cat >"$registry_layer_dir/json" <<JSON
{
  "id": "$registry_layer_digest",
  "created": "2026-01-01T00:00:00Z",
  "container_config": {
    "Cmd": ["/usr/local/bin/cratebay-smoke", "registry", "--state-dir", "/registry-seed", "--listen", "0.0.0.0:5000"]
  }
}
JSON

    cat >"$registry_config_tmp" <<JSON
{
  "created": "2026-01-01T00:00:00Z",
  "architecture": "$docker_arch",
  "os": "linux",
  "config": {
    "Env": ["PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"],
    "Cmd": ["/usr/local/bin/cratebay-smoke", "registry", "--state-dir", "/registry-seed", "--listen", "0.0.0.0:5000"],
    "WorkingDir": "/"
  },
  "rootfs": {
    "type": "layers",
    "diff_ids": ["sha256:$registry_layer_digest"]
  },
  "history": [
    {
      "created": "2026-01-01T00:00:00Z",
      "created_by": "cratebay runtime smoke local registry"
    }
  ]
}
JSON

    registry_config_digest="$(sha256_file "$registry_config_tmp")"
    registry_config_name="${registry_config_digest}.json"
    mv "$registry_config_tmp" "$registry_image_dir/$registry_config_name"

    cat >"$registry_image_dir/manifest.json" <<JSON
[
  {
    "Config": "$registry_config_name",
    "RepoTags": ["$registry_server_image"],
    "Layers": ["$registry_layer_digest/layer.tar"]
  }
]
JSON

    cat >"$registry_image_dir/repositories" <<JSON
{
  "${registry_server_image%:*}": {
    "${registry_server_image##*:}": "$registry_layer_digest"
  }
}
JSON

    COPYFILE_DISABLE=1 tar -C "$registry_image_dir" -cf "$registry_archive" manifest.json repositories "$registry_config_name" "$registry_layer_digest"
    registry_import_output="$("$cratebay_bin" image import "$registry_archive")"
    printf '%s\n' "$registry_import_output"

    echo "== Start local registry container =="
    registry_container_name="cbx-registry-${suffix}"
    create_registry_output="$("$cratebay_bin" container create "$registry_container_name" \
      --image "$registry_server_image" \
      --publish 5000:5000 \
      --no-start)"
    printf '%s\n' "$create_registry_output"

    start_registry_output="$("$cratebay_bin" container start "$registry_container_name")"
    printf '%s\n' "$start_registry_output"

    echo "== Pull from local registry =="
    local registry_pull_output registry_pull_status
    registry_pull_status=1
    for _ in {1..30}; do
      set +e
      registry_pull_output="$("$cratebay_bin" image pull "$registry_ref" 2>&1)"
      registry_pull_status=$?
      set -e
      if [[ "$registry_pull_status" -eq 0 ]]; then
        break
      fi
      sleep 1
    done
    if [[ "$registry_pull_status" -ne 0 ]]; then
      echo "ERROR: local registry image pull did not succeed" >&2
      echo "--- pull output ---" >&2
      printf '%s\n' "$registry_pull_output" >&2
      echo "--- registry inspect ---" >&2
      "$cratebay_bin" container inspect "$registry_container_name" >&2 || true
      exit 1
    fi
    printf '%s\n' "$registry_pull_output"

    list_output="$("$cratebay_bin" image list)"
    assert_contains "$list_output" "$registry_ref" "local registry image should be available after pull"
    runtime_image="$registry_ref"
  else
    runtime_image="$image_name"
  fi
  offline_smoke_image=1
  return 0
}

prepare_smoke_image() {
  if [[ -n "$runtime_image" ]]; then
    echo "Using CRATEBAY_SMOKE_RUNTIME_IMAGE=$runtime_image"
    return
  fi

  if [[ "${CRATEBAY_SMOKE_OFFLINE_IMAGE:-1}" != "0" ]]; then
    if create_offline_smoke_image; then
      echo "Using offline smoke image: $runtime_image"
      return
    fi
  fi

  runtime_image="${CRATEBAY_SMOKE_FALLBACK_RUNTIME_IMAGE:-nginx:1.27-alpine}"
  echo "Using fallback registry image: $runtime_image"
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
export CRATEBAY_LINUX_DOCKER_PORT="$CRATEBAY_DOCKER_PROXY_PORT"

cleanup() {
  set +e
  if [[ -x "$cratebay_bin" ]]; then
    if [[ -n "$registry_container_name" ]]; then
      "$cratebay_bin" container delete "$registry_container_name" --force >/dev/null 2>&1 || true
    fi
    if [[ "$container_removed" != "1" ]]; then
      "$cratebay_bin" container delete "$container_name" --force >/dev/null 2>&1 || true
    fi
    "$cratebay_bin" pod delete "$pod_name" --force >/dev/null 2>&1 || true
    "$cratebay_bin" volume remove "$volume_name" --force >/dev/null 2>&1 || true
    if [[ -n "$registry_server_image" ]]; then
      "$cratebay_bin" image delete "$registry_server_image" >/dev/null 2>&1 || true
    fi
    "$cratebay_bin" image delete "$tagged_image" >/dev/null 2>&1 || true
    "$cratebay_bin" image delete "$packaged_image" >/dev/null 2>&1 || true
    if [[ -n "$runtime_image" && "$offline_smoke_image" == "1" ]]; then
      "$cratebay_bin" image delete "$runtime_image" >/dev/null 2>&1 || true
    fi
    "$cratebay_bin" runtime stop >/dev/null 2>&1 || true
  fi
  rm -f "$CRATEBAY_DOCKER_SOCKET_PATH" >/dev/null 2>&1 || true
  if [[ "${CRATEBAY_KEEP_SMOKE_TEMP:-0}" == "1" ]]; then
    echo "Keeping smoke temp dir: $TEMP_DIR" >&2
  else
    rm -rf "$TEMP_DIR" >/dev/null 2>&1 || true
  fi
  if [[ "${CRATEBAY_KEEP_SMOKE_DATA:-0}" == "1" || "${CRATEBAY_KEEP_SMOKE_TEMP:-0}" == "1" ]]; then
    echo "Keeping smoke data dir: $smoke_data_dir" >&2
  elif [[ "${OS:-}" == "Windows_NT" || -n "${MSYSTEM:-}" ]]; then
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
elif [[ "$(uname -s)" == "Linux" ]]; then
  prepare_linux_runtime
fi

echo "== Build cratebay CLI =="
cargo build -p cratebay-cli >/dev/null

if [[ ! -x "$cratebay_bin" ]]; then
  echo "ERROR: built cratebay binary not found at $cratebay_bin"
  exit 1
fi

echo "== Structured CLI error contract =="
set +e
structured_error_output="$("$cratebay_bin" --json --docker-host bad-host container list 2>&1)"
structured_error_status=$?
set -e
printf '%s\n' "$structured_error_output"
if [[ "$structured_error_status" -eq 0 ]]; then
  echo "ASSERTION FAILED: structured CLI error path should return a non-zero exit code"
  exit 1
fi
assert_contains "$structured_error_output" '"ok": false' "structured CLI errors should include ok=false"
assert_contains "$structured_error_output" '"kind": "runtime"' "structured CLI errors should expose a runtime kind"

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

echo "== Verify bundled image preload failure is explicit =="
missing_bundle_dir="$TEMP_DIR/missing-bundle-images"
mkdir -p "$missing_bundle_dir"
set +e
missing_bundle_output="$("$cratebay_bin" --json image preload-bundled --dir "$missing_bundle_dir" 2>&1)"
missing_bundle_status=$?
set -e
printf '%s\n' "$missing_bundle_output"
if [[ "$missing_bundle_status" -eq 0 ]]; then
  echo "ASSERTION FAILED: missing bundled image archives should return a non-zero exit code"
  exit 1
fi
assert_contains "$missing_bundle_output" '"loaded": false' "missing bundled image preload should report failed entries"
assert_contains "$missing_bundle_output" "archive not found" "missing bundled image preload should explain missing archives"

prepare_smoke_image

echo "== One-shot run for embedded CLI usage =="
if [[ "$offline_smoke_image" == "1" ]]; then
  run_output="$("$cratebay_bin" run --no-pull --network none --read-only --entrypoint /usr/local/bin/cratebay-smoke "$runtime_image" -- exists /usr/local/bin/cratebay-smoke)"
else
  run_output="$("$cratebay_bin" run --network none --read-only "$runtime_image" -- sh -lc "pwd && id")"
fi
printf '%s\n' "$run_output"
assert_contains "$run_output" "/" "one-shot run should print command output"

if [[ "$offline_smoke_image" == "1" ]]; then
  echo "== One-shot structured output limit =="
  bounded_output="$("$cratebay_bin" --json run --no-pull --max-output-bytes 4 --entrypoint /usr/local/bin/cratebay-smoke "$runtime_image" -- echo 123456789)"
  printf '%s\n' "$bounded_output"
  assert_contains "$bounded_output" '"stdout": "1234"' "bounded one-shot run should truncate stdout"
  assert_contains "$bounded_output" '"stdoutTruncated": true' "bounded one-shot run should mark truncated stdout"

  echo "== One-shot structured result without propagating container exit =="
  set +e
  nonprop_output="$("$cratebay_bin" --json run --no-pull --no-propagate-exit-code --entrypoint /usr/local/bin/cratebay-smoke "$runtime_image" -- exists /does-not-exist 2>&1)"
  nonprop_status=$?
  set -e
  printf '%s\n' "$nonprop_output"
  if [[ "$nonprop_status" -ne 0 ]]; then
    echo "ASSERTION FAILED: --no-propagate-exit-code should leave CLI exit status 0, got $nonprop_status"
    exit 1
  fi
  assert_contains "$nonprop_output" '"exitCode": 66' "non-propagating one-shot run should report the container exit code"
  assert_contains "$nonprop_output" '"timedOut": false' "non-propagating one-shot run should report timeout state"

  echo "== One-shot timeout exit code =="
  set +e
  timeout_output="$("$cratebay_bin" run --no-pull --timeout 1 "$runtime_image" -- /usr/local/bin/cratebay-smoke serve 2>&1)"
  timeout_status=$?
  set -e
  printf '%s\n' "$timeout_output"
  if [[ "$timeout_status" -ne 124 ]]; then
    echo "ASSERTION FAILED: timed-out one-shot run should exit 124, got $timeout_status"
    exit 1
  fi
fi

echo "== Pod lifecycle =="
pod_create_output="$("$cratebay_bin" pod create "$pod_name")"
printf '%s\n' "$pod_create_output"
assert_contains "$pod_create_output" "$pod_name" "pod create should report the new pod"

echo "== Create container in pod (auto-pull if missing) =="
create_output="$("$cratebay_bin" container create "$container_name" --image "$runtime_image" --env "${env_key}=${env_value}" --pod "$pod_name" --no-start)"
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

pod_inspect_output="$("$cratebay_bin" pod inspect "$pod_name")"
printf '%s\n' "$pod_inspect_output"
assert_contains "$pod_inspect_output" "$container_name" "pod inspect should show the attached container"

pod_remove_output="$("$cratebay_bin" pod remove "$pod_name" "$container_name" --force)"
printf '%s\n' "$pod_remove_output"
assert_contains "$pod_remove_output" "$container_name" "pod remove should report the container"

pod_add_output="$("$cratebay_bin" pod add "$pod_name" "$container_name")"
printf '%s\n' "$pod_add_output"
assert_contains "$pod_add_output" "$container_name" "pod add should report the container"

echo "== Verify exec and logs =="
if [[ "$offline_smoke_image" == "1" ]]; then
  exec_output="$("$cratebay_bin" container exec "$container_name" -- /usr/local/bin/cratebay-smoke env "$env_key")"
else
  exec_output="$("$cratebay_bin" container exec "$container_name" -- printenv "$env_key")"
fi
printf '%s\n' "$exec_output"
assert_contains "$exec_output" "$env_value" "container exec should see the injected env value"

if [[ "$offline_smoke_image" == "1" ]]; then
  echo "== Verify table exec exit code =="
  set +e
  exec_table_output="$("$cratebay_bin" container exec "$container_name" -- /usr/local/bin/cratebay-smoke exists /does-not-exist 2>&1)"
  exec_table_status=$?
  set -e
  printf '%s\n' "$exec_table_output"
  if [[ "$exec_table_status" -ne 66 ]]; then
    echo "ASSERTION FAILED: table container exec should exit 66, got $exec_table_status"
    exit 1
  fi
  assert_contains "$exec_table_output" "missing /does-not-exist" "table container exec should print stderr"

  echo "== Verify structured exec exit code =="
  set +e
  exec_json_output="$("$cratebay_bin" --json container exec --no-propagate-exit-code "$container_name" -- /usr/local/bin/cratebay-smoke exists /does-not-exist 2>&1)"
  exec_json_status=$?
  set -e
  printf '%s\n' "$exec_json_output"
  if [[ "$exec_json_status" -ne 0 ]]; then
    echo "ASSERTION FAILED: non-propagating structured container exec should exit 0, got $exec_json_status"
    exit 1
  fi
  assert_contains "$exec_json_output" '"exitCode": 66' "structured container exec should include the process exit code"
  assert_contains "$exec_json_output" '"timedOut": false' "structured container exec should report timeout state"

  echo "== Verify structured exec output limit =="
  exec_bounded_output="$("$cratebay_bin" --json container exec --max-output-bytes 4 --no-propagate-exit-code "$container_name" -- /usr/local/bin/cratebay-smoke echo 123456789)"
  printf '%s\n' "$exec_bounded_output"
  assert_contains "$exec_bounded_output" '"stdout": "1234"' "bounded structured container exec should truncate stdout"
  assert_contains "$exec_bounded_output" '"stdoutTruncated": true' "bounded structured container exec should mark truncated stdout"
  assert_contains "$exec_bounded_output" '"stderrTruncated": false' "bounded structured container exec should keep stderr untruncated"

  echo "== Verify structured exec timeout =="
  set +e
  exec_timeout_output="$("$cratebay_bin" --json container exec --timeout 1 --no-propagate-exit-code "$container_name" -- /usr/local/bin/cratebay-smoke serve 2>&1)"
  exec_timeout_status=$?
  set -e
  printf '%s\n' "$exec_timeout_output"
  if [[ "$exec_timeout_status" -ne 0 ]]; then
    echo "ASSERTION FAILED: non-propagating timed-out container exec should exit 0, got $exec_timeout_status"
    exit 1
  fi
  assert_contains "$exec_timeout_output" '"exitCode": 124' "timed-out structured container exec should include timeout exit code"
  assert_contains "$exec_timeout_output" '"timedOut": true' "timed-out structured container exec should report timeout state"
  assert_contains "$exec_timeout_output" "Execution timed out after 1s" "timed-out structured container exec should report timeout"
fi

logs_output="$("$cratebay_bin" container logs "$container_name" --tail 20 || true)"
printf '%s\n' "$logs_output"

echo "== Verify image list =="
image_list_output="$("$cratebay_bin" image list)"
printf '%s\n' "$image_list_output"
assert_contains "$image_list_output" "$runtime_image" "image list should include the runtime image"

echo "== Package container into image =="
pack_output="$("$cratebay_bin" image pack-container "$container_name" "$packaged_image")"
printf '%s\n' "$pack_output"
assert_contains "$pack_output" "$packaged_image" "pack-container should report the packaged image"

tag_output="$("$cratebay_bin" image tag "$packaged_image" "$tagged_image")"
printf '%s\n' "$tag_output"
assert_contains "$tag_output" "$tagged_image" "image tag should report the new tag"

image_inspect_output="$("$cratebay_bin" image inspect "$tagged_image")"
printf '%s\n' "$image_inspect_output"
assert_contains "$image_inspect_output" "$tagged_image" "image inspect should include the tagged image"

echo "== Volume lifecycle =="
volume_create_output="$("$cratebay_bin" volume create "$volume_name")"
printf '%s\n' "$volume_create_output"
assert_contains "$volume_create_output" "$volume_name" "volume create should report the new volume"

volume_list_output="$("$cratebay_bin" volume list)"
printf '%s\n' "$volume_list_output"
assert_contains "$volume_list_output" "$volume_name" "volume list should show the new volume"

volume_inspect_output="$("$cratebay_bin" volume inspect "$volume_name")"
printf '%s\n' "$volume_inspect_output"
assert_contains "$volume_inspect_output" "\"Name\": \"$volume_name\"" "volume inspect should show the created volume"

volume_remove_output="$("$cratebay_bin" volume remove "$volume_name")"
printf '%s\n' "$volume_remove_output"
assert_contains "$volume_remove_output" "$volume_name" "volume remove should report the deleted volume"
volume_removed=1

echo "== Stop and delete container =="
stop_output="$("$cratebay_bin" container stop "$container_name")"
printf '%s\n' "$stop_output"
assert_contains "$stop_output" "Stopped $container_name" "container stop should succeed"

delete_output="$("$cratebay_bin" container delete "$container_name")"
printf '%s\n' "$delete_output"
assert_contains "$delete_output" "Deleted $container_name" "container delete should succeed"
container_removed=1

pod_delete_output="$("$cratebay_bin" pod delete "$pod_name" --force)"
printf '%s\n' "$pod_delete_output"
assert_contains "$pod_delete_output" "Deleted pod $pod_name" "pod delete should succeed"
pod_removed=1

echo "== Image export/import round trip =="
export_archive="$TEMP_DIR/${tagged_image//[:\/]/-}.tar"
export_output="$("$cratebay_bin" image export --output "$export_archive" "$tagged_image")"
printf '%s\n' "$export_output"
assert_contains "$export_output" "$export_archive" "image export should report the archive path"

"$cratebay_bin" image delete "$tagged_image" >/dev/null 2>&1 || true
"$cratebay_bin" image delete "$packaged_image" >/dev/null 2>&1 || true

import_output="$("$cratebay_bin" image import "$export_archive")"
printf '%s\n' "$import_output"

roundtrip_image_list="$("$cratebay_bin" image list)"
printf '%s\n' "$roundtrip_image_list"
assert_contains "$roundtrip_image_list" "$tagged_image" "image import should restore the exported tag"

echo "CLI-only runtime smoke: PASS"
