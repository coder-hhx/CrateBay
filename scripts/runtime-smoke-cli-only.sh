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
native_container_name="cbx-native-smoke-${suffix}"
native_run_name="cbx-native-run-${suffix}"
pod_name="cbx-runtime-pod-${suffix}"
packaged_image="cbx-runtime-pack:${suffix}"
tagged_image="cbx-runtime-pack:${suffix}-tag"
volume_name="cbx-runtime-volume-${suffix}"
network_name="cbx-runtime-network-${suffix}"
runtime_image="${CRATEBAY_SMOKE_RUNTIME_IMAGE:-}"
offline_smoke_image=0
env_key="CRATEBAY_E2E"
env_value="smoke-${suffix}"
container_removed=0
pod_removed=0
volume_removed=0
network_removed=0
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

runtime_assets_stale() {
  local marker="$1"
  [[ -f "$marker" ]] || return 0

  local newer_source
  newer_source="$(
    find \
      "$repo_root/crates/cratebay-engine-adapter" \
      "$repo_root/crates/cratebay-guest-agent" \
      "$repo_root/scripts/build-runtime-assets-alpine.sh" \
      "$repo_root/Cargo.toml" \
      "$repo_root/Cargo.lock" \
      -type f \( -name '*.rs' -o -name 'Cargo.toml' -o -name 'Cargo.lock' -o -name 'build-runtime-assets-alpine.sh' \) \
      -newer "$marker" \
      -print -quit 2>/dev/null || true
  )"
  [[ -n "$newer_source" ]]
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

  local image_dir="$repo_root/crates/cratebay-gui/src-tauri/runtime-images/cratebay-runtime-${runtime_arch}"
  if ! ready_runtime_file "$image_dir/vmlinuz" \
    || ! ready_runtime_file "$image_dir/initramfs" \
    || runtime_assets_stale "$image_dir/initramfs"; then
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
    || ! ready_runtime_file "$image_dir/initramfs" \
    || runtime_assets_stale "$image_dir/initramfs"; then
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

run_capture() {
  local output_var="$1"
  shift
  local output status
  set +e
  output="$("$@" 2>&1)"
  status=$?
  set -e
  printf -v "$output_var" '%s' "$output"
  if [[ "$status" -ne 0 ]]; then
    echo "COMMAND FAILED ($status): $*"
    echo "--- output ---"
    printf '%s\n' "$output"
    exit "$status"
  fi
}

run_cleanup_cmd() {
  if command -v perl >/dev/null 2>&1; then
    perl -e '
      my $timeout = shift @ARGV;
      my $pid = fork();
      exit 125 unless defined $pid;
      if ($pid == 0) {
        exec @ARGV;
        exit 127;
      }
      local $SIG{ALRM} = sub {
        kill "TERM", $pid;
        sleep 1;
        kill "KILL", $pid;
        exit 124;
      };
      alarm $timeout;
      waitpid($pid, 0);
      exit(($? >> 8) || 0);
    ' 8 "$@" >/dev/null 2>&1 || true
  else
    "$@" >/dev/null 2>&1 || true
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
use std::io::{self, BufRead, BufReader, Write};
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
        Some("pty") => run_pty_echo(),
        Some(other) => {
            eprintln!("unknown command: {}", other);
            process::exit(64);
        }
        None => println!("cratebay-smoke"),
    }
}

fn run_pty_echo() {
    println!("cratebay-smoke pty ready");
    let _ = io::stdout().flush();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap_or_default();
        if line.trim() == "exit" {
            break;
        }
        println!("pty: {}", line.trim_end());
        let _ = io::stdout().flush();
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
    } else if path == format!("/v2/{}/blobs/{}", state.repo, state.config_digest)
        || path == format!("/v2/{}/blobs/sha256:{}", state.repo, state.config_digest)
    {
        let digest_header = format!("sha256:{}", state.config_digest);
        registry_response(
            "200 OK",
            "application/vnd.docker.container.image.v1+json",
            &state.config,
            vec![
                ("Docker-Distribution-API-Version", "registry/2.0"),
                ("Docker-Content-Digest", digest_header.as_str()),
                ("Cache-Control", "no-cache"),
            ],
            is_head,
        )
    } else if path == format!("/v2/{}/blobs/{}", state.repo, state.layer_digest)
        || path == format!("/v2/{}/blobs/sha256:{}", state.repo, state.layer_digest)
    {
        let digest_header = format!("sha256:{}", state.layer_digest);
        registry_response(
            "200 OK",
            "application/vnd.docker.image.rootfs.diff.tar",
            &state.layer,
            vec![
                ("Docker-Distribution-API-Version", "registry/2.0"),
                ("Docker-Content-Digest", digest_header.as_str()),
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
  import_output="$("$cratebay_bin" --json image import "$archive")"
  printf '%s\n' "$import_output"
  assert_contains "$import_output" '"api": "cratebay.image.import.v1"' "top-level image import should use the native image API"
  assert_contains "$import_output" '"backend": "containerd"' "top-level image import should use containerd"

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
    registry_import_output="$("$cratebay_bin" --json image import "$registry_archive")"
    printf '%s\n' "$registry_import_output"
    assert_contains "$registry_import_output" '"api": "cratebay.image.import.v1"' "top-level registry image import should use the native image API"
    assert_contains "$registry_import_output" '"backend": "containerd"' "top-level registry image import should use containerd"

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
export CRATEBAY_ENGINE_SOCKET_PATH="/tmp/cratebay-smoke-${suffix}.sock"
export CRATEBAY_ENGINE_PROXY_PORT="$((42000 + (suffix % 10000)))"
export CRATEBAY_DOCKER_PROXY_PORT="$CRATEBAY_ENGINE_PROXY_PORT"
export CRATEBAY_LINUX_DOCKER_PORT="$CRATEBAY_ENGINE_PROXY_PORT"
export CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT="$((52000 + (suffix % 10000)))"

cleanup() {
  set +e
  if [[ -x "$cratebay_bin" ]]; then
    if [[ -n "$registry_container_name" ]]; then
      run_cleanup_cmd "$cratebay_bin" container delete "$registry_container_name" --force
    fi
    if [[ "$container_removed" != "1" ]]; then
      run_cleanup_cmd "$cratebay_bin" container delete "$container_name" --force
    fi
    run_cleanup_cmd "$cratebay_bin" engine remove "$native_container_name" --force
    run_cleanup_cmd "$cratebay_bin" engine remove "$native_run_name" --force
    run_cleanup_cmd "$cratebay_bin" pod delete "$pod_name" --force
    run_cleanup_cmd "$cratebay_bin" volume remove "$volume_name" --force
    run_cleanup_cmd "$cratebay_bin" network remove "$network_name"
    if [[ -n "$registry_server_image" ]]; then
      run_cleanup_cmd "$cratebay_bin" image delete "$registry_server_image"
    fi
    run_cleanup_cmd "$cratebay_bin" image delete "$tagged_image"
    run_cleanup_cmd "$cratebay_bin" image delete "$packaged_image"
    if [[ -n "$runtime_image" && "$offline_smoke_image" == "1" ]]; then
      run_cleanup_cmd "$cratebay_bin" image delete "$runtime_image"
    fi
    run_cleanup_cmd "$cratebay_bin" runtime stop
  fi
  local runner_pid_file="$smoke_data_dir/runtime/vm/runner.pid"
  if [[ -f "$runner_pid_file" ]]; then
    local runner_pid
    runner_pid="$(cat "$runner_pid_file" 2>/dev/null || true)"
    if [[ "$runner_pid" =~ ^[0-9]+$ ]] && kill -0 "$runner_pid" >/dev/null 2>&1; then
      kill "$runner_pid" >/dev/null 2>&1 || true
      sleep 1
      kill -9 "$runner_pid" >/dev/null 2>&1 || true
    fi
  fi
  rm -f "${CRATEBAY_ENGINE_SOCKET_PATH:-}" "${CRATEBAY_DOCKER_SOCKET_PATH:-}" >/dev/null 2>&1 || true
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

echo "== Verify CrateBay Engine status =="
engine_status_output="$("$cratebay_bin" system engine-status)"
printf '%s\n' "$engine_status_output"
assert_contains "$engine_status_output" "CrateBay Engine: connected" "system engine-status should connect to built-in runtime"

echo "== Verify native CrateBay Engine contract =="
native_engine_contract="$("$cratebay_bin" --json engine status)"
printf '%s\n' "$native_engine_contract"
assert_contains "$native_engine_contract" '"name": "CrateBay Engine"' "native engine contract should identify CrateBay Engine"
assert_contains "$native_engine_contract" '"kind": "cratebay-containerd"' "native engine contract should use the CrateBay containerd engine"
assert_contains "$native_engine_contract" '"api": "cratebay.engine.v1"' "native engine contract should expose the CrateBay native API"
assert_contains "$native_engine_contract" '"runtime": "containerd"' "native engine contract should report containerd as runtime backend"
assert_contains "$native_engine_contract" '"ociRuntime": "runc"' "native engine contract should report runc as OCI runtime"
assert_contains "$native_engine_contract" '"dockerCompatible": true' "Docker compatibility should be reported as an adapter capability"

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
  run_capture run_output "$cratebay_bin" run --no-pull --network none --read-only --entrypoint /usr/local/bin/cratebay-smoke "$runtime_image" -- exists /usr/local/bin/cratebay-smoke
else
  run_capture run_output "$cratebay_bin" run --network none --read-only "$runtime_image" -- sh -lc "pwd && id"
fi
printf '%s\n' "$run_output"
assert_contains "$run_output" "/" "one-shot run should print command output"

if [[ "$offline_smoke_image" == "1" ]]; then
  echo "== One-shot structured output limit =="
  run_capture bounded_output "$cratebay_bin" --json run --no-pull --max-output-bytes 4 --entrypoint /usr/local/bin/cratebay-smoke "$runtime_image" -- echo 123456789
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
run_capture pod_create_output "$cratebay_bin" pod create "$pod_name"
printf '%s\n' "$pod_create_output"
assert_contains "$pod_create_output" "$pod_name" "pod create should report the new pod"

echo "== Create container in pod (auto-pull if missing) =="
run_capture create_output "$cratebay_bin" container create "$container_name" --image "$runtime_image" --env "${env_key}=${env_value}" --pod "$pod_name" --no-start
printf '%s\n' "$create_output"
assert_contains "$create_output" "$container_name" "container create should report the created container"

echo "== Verify container list =="
run_capture list_output "$cratebay_bin" container list --all
printf '%s\n' "$list_output"
assert_contains "$list_output" "$container_name" "container list should show the created container"
assert_contains "$list_output" "$runtime_image" "container list should show the runtime image"

echo "== Start container =="
run_capture start_output "$cratebay_bin" container start "$container_name"
printf '%s\n' "$start_output"
assert_contains "$start_output" "Started $container_name" "container start should succeed"

run_capture pod_inspect_output "$cratebay_bin" pod inspect "$pod_name"
printf '%s\n' "$pod_inspect_output"
assert_contains "$pod_inspect_output" "$container_name" "pod inspect should show the attached container"

run_capture pod_remove_output "$cratebay_bin" pod remove "$pod_name" "$container_name" --force
printf '%s\n' "$pod_remove_output"
assert_contains "$pod_remove_output" "$container_name" "pod remove should report the container"

run_capture pod_add_output "$cratebay_bin" pod add "$pod_name" "$container_name"
printf '%s\n' "$pod_add_output"
assert_contains "$pod_add_output" "$container_name" "pod add should report the container"

run_capture pod_remove_json "$cratebay_bin" --json pod remove "$pod_name" "$container_name" --force
printf '%s\n' "$pod_remove_json"
assert_contains "$pod_remove_json" '"api": "cratebay.pod.detach.v1"' "structured pod remove should use the native CrateBay pod detach API"
assert_contains "$pod_remove_json" '"detached": true' "structured pod remove should report native detach state"

run_capture pod_add_json "$cratebay_bin" --json pod add "$pod_name" "$container_name"
printf '%s\n' "$pod_add_json"
assert_contains "$pod_add_json" '"api": "cratebay.pod.attach.v1"' "structured pod add should use the native CrateBay pod attach API"
assert_contains "$pod_add_json" '"attached": true' "structured pod add should report native attach state"

echo "== Verify exec and logs =="
if [[ "$offline_smoke_image" == "1" ]]; then
  run_capture exec_output "$cratebay_bin" container exec "$container_name" -- /usr/local/bin/cratebay-smoke env "$env_key"
else
  run_capture exec_output "$cratebay_bin" container exec "$container_name" -- printenv "$env_key"
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
  run_capture exec_bounded_output "$cratebay_bin" --json container exec --max-output-bytes 4 --no-propagate-exit-code "$container_name" -- /usr/local/bin/cratebay-smoke echo 123456789
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
  assert_contains "$exec_timeout_output" "timed out after 1s" "timed-out structured container exec should report timeout"
fi

logs_output="$("$cratebay_bin" container logs "$container_name" --tail 20 || true)"
printf '%s\n' "$logs_output"

if [[ "$offline_smoke_image" == "1" ]]; then
  echo "== Native CrateBay Engine one-shot run =="
  run_capture native_run_output "$cratebay_bin" --json engine run --name "$native_run_name" --no-pull --timeout 30 --max-output-bytes 4096 "$runtime_image" -- /usr/local/bin/cratebay-smoke echo native-run-ok
  printf '%s\n' "$native_run_output"
  assert_contains "$native_run_output" '"api": "cratebay.container.run.v1"' "native engine run should use the native run API"
  assert_contains "$native_run_output" '"backend": "containerd"' "native engine run should execute through containerd"
  assert_contains "$native_run_output" '"exitCode": 0' "native engine run should capture the exit code"
  assert_contains "$native_run_output" "native-run-ok" "native engine run should capture stdout"
  assert_contains "$native_run_output" '"removed": true' "native engine run should remove the one-shot container by default"

  echo "== Native CrateBay Engine container lifecycle =="
  run_capture native_create_output "$cratebay_bin" --json engine create "$native_container_name" --image "$runtime_image" --entrypoint /usr/local/bin/cratebay-smoke --command serve --env "${env_key}=native-${env_value}" --pod "$pod_name" --no-start
  printf '%s\n' "$native_create_output"
  assert_contains "$native_create_output" '"api": "cratebay.container.create.v1"' "native engine create should use the native create API"
  assert_contains "$native_create_output" '"backend": "containerd-pending"' "native engine create should register a containerd-backed container"
  assert_contains "$native_create_output" "$native_container_name" "native engine create should report the container name"

  run_capture native_list_output "$cratebay_bin" --json engine containers
  printf '%s\n' "$native_list_output"
  assert_contains "$native_list_output" '"api": "cratebay.containers.v1"' "native engine containers should use the native list API"
  assert_contains "$native_list_output" '"managedBy": "cratebay"' "native engine containers should be CrateBay-managed"
  assert_contains "$native_list_output" "$native_container_name" "native engine containers should show the native container"

  run_capture native_start_output "$cratebay_bin" --json engine start "$native_container_name"
  printf '%s\n' "$native_start_output"
  assert_contains "$native_start_output" '"api": "cratebay.container.start.v1"' "native engine start should use the native start API"
  assert_contains "$native_start_output" '"backend": "containerd"' "native engine start should run through containerd"

  run_capture native_inspect_output "$cratebay_bin" --json engine inspect "$native_container_name"
  printf '%s\n' "$native_inspect_output"
  assert_contains "$native_inspect_output" '"api": "cratebay.container.inspect.v1"' "native engine inspect should use the native inspect API"
  assert_contains "$native_inspect_output" "$native_container_name" "native engine inspect should include the native container"

  run_capture native_exec_output "$cratebay_bin" --json engine exec "$native_container_name" -- /usr/local/bin/cratebay-smoke env "$env_key"
  printf '%s\n' "$native_exec_output"
  assert_contains "$native_exec_output" '"api": "cratebay.container.exec.v1"' "native engine exec should use the native exec API"
  assert_contains "$native_exec_output" "native-${env_value}" "native engine exec should see the injected env value"

  run_capture native_logs_output "$cratebay_bin" --json engine logs "$native_container_name" --tail 20
  printf '%s\n' "$native_logs_output"
  assert_contains "$native_logs_output" '"api": "cratebay.container.logs.v1"' "native engine logs should use the native logs API"
  assert_contains "$native_logs_output" "cratebay-smoke ready" "native engine logs should include container output"

  run_capture native_stats_output "$cratebay_bin" --json engine stats "$native_container_name"
  printf '%s\n' "$native_stats_output"
  assert_contains "$native_stats_output" '"api": "cratebay.container.stats.v1"' "native engine stats should use the native stats API"
  assert_contains "$native_stats_output" '"backend": "containerd"' "native engine stats should read from containerd metrics"
  assert_contains "$native_stats_output" '"memory"' "native engine stats should include memory metrics"

  echo "== Native CrateBay Engine PTY terminal lifecycle =="
  terminal_session_id="cbx-native-pty-${suffix}"
  run_capture native_terminal_open_output "$cratebay_bin" --json engine terminal-open "$native_container_name" --session-id "$terminal_session_id" --cols 80 --rows 24 -- /usr/local/bin/cratebay-smoke pty
  printf '%s\n' "$native_terminal_open_output"
  assert_contains "$native_terminal_open_output" '"api": "cratebay.container.terminal.open.v1"' "native engine terminal-open should use the native terminal API"
  assert_contains "$native_terminal_open_output" '"backend": "containerd-pty"' "native engine terminal should use the containerd PTY backend"
  assert_contains "$native_terminal_open_output" '"transport": "cratebay-native-pty"' "native engine terminal should use the CrateBay PTY transport"

  run_capture native_terminal_resize_output "$cratebay_bin" --json engine terminal-resize "$native_container_name" --session-id "$terminal_session_id" --cols 120 --rows 33
  printf '%s\n' "$native_terminal_resize_output"
  assert_contains "$native_terminal_resize_output" '"api": "cratebay.container.terminal.resize.v1"' "native engine terminal-resize should use the native resize API"
  assert_contains "$native_terminal_resize_output" '"resized": true' "native engine terminal-resize should apply PTY window size"

  run_capture native_terminal_input_output "$cratebay_bin" --json engine terminal-input "$native_container_name" --session-id "$terminal_session_id" --data $'pty-smoke\n'
  printf '%s\n' "$native_terminal_input_output"
  assert_contains "$native_terminal_input_output" '"api": "cratebay.container.terminal.input.v1"' "native engine terminal-input should use the native input API"

  native_terminal_read_output=""
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    run_capture native_terminal_read_output "$cratebay_bin" --json engine terminal-read "$native_container_name" --session-id "$terminal_session_id"
    if printf '%s\n' "$native_terminal_read_output" | grep -Fq "pty: pty-smoke"; then
      break
    fi
    sleep 0.2
  done
  printf '%s\n' "$native_terminal_read_output"
  assert_contains "$native_terminal_read_output" '"api": "cratebay.container.terminal.read.v1"' "native engine terminal-read should use the native read API"
  assert_contains "$native_terminal_read_output" "cratebay-smoke pty ready" "native engine terminal-read should capture PTY startup output"
  assert_contains "$native_terminal_read_output" "pty: pty-smoke" "native engine terminal-read should capture PTY input response"

  run_capture native_terminal_close_output "$cratebay_bin" --json engine terminal-close "$native_container_name" --session-id "$terminal_session_id"
  printf '%s\n' "$native_terminal_close_output"
  assert_contains "$native_terminal_close_output" '"api": "cratebay.container.terminal.close.v1"' "native engine terminal-close should use the native close API"
  assert_contains "$native_terminal_close_output" '"closed": true' "native engine terminal-close should close the PTY session"

  run_capture native_stop_output "$cratebay_bin" --json engine stop "$native_container_name"
  printf '%s\n' "$native_stop_output"
  assert_contains "$native_stop_output" '"api": "cratebay.container.stop.v1"' "native engine stop should use the native stop API"

  run_capture native_remove_output "$cratebay_bin" --json engine remove "$native_container_name"
  printf '%s\n' "$native_remove_output"
  assert_contains "$native_remove_output" '"api": "cratebay.container.remove.v1"' "native engine remove should use the native remove API"
fi

echo "== Verify image list =="
run_capture image_list_output "$cratebay_bin" image list
printf '%s\n' "$image_list_output"
assert_contains "$image_list_output" "$runtime_image" "image list should include the runtime image"

echo "== Package container into image =="
run_capture pack_output "$cratebay_bin" --json image pack-container "$container_name" "$packaged_image"
printf '%s\n' "$pack_output"
assert_contains "$pack_output" '"api": "cratebay.image.pack.v1"' "top-level image pack should use the native image API"
assert_contains "$pack_output" '"backend": "containerd"' "top-level image pack should use containerd"
assert_contains "$pack_output" "$packaged_image" "pack-container should report the packaged image"

run_capture tag_output "$cratebay_bin" --json image tag "$packaged_image" "$tagged_image"
printf '%s\n' "$tag_output"
assert_contains "$tag_output" '"api": "cratebay.image.tag.v1"' "top-level image tag should use the native image API"
assert_contains "$tag_output" '"backend": "containerd"' "top-level image tag should use containerd"
assert_contains "$tag_output" "$tagged_image" "image tag should report the new tag"

run_capture image_inspect_output "$cratebay_bin" image inspect "$tagged_image"
printf '%s\n' "$image_inspect_output"
assert_contains "$image_inspect_output" "$tagged_image" "image inspect should include the tagged image"

run_capture native_image_inspect_output "$cratebay_bin" --json engine inspect-image "$tagged_image"
printf '%s\n' "$native_image_inspect_output"
assert_contains "$native_image_inspect_output" '"api": "cratebay.image.inspect.v1"' "native engine image inspect should use the native image API"
assert_contains "$native_image_inspect_output" '"backend": "containerd"' "native engine image inspect should read from containerd"

echo "== Volume lifecycle =="
run_capture volume_create_output "$cratebay_bin" volume create "$volume_name" --driver local
printf '%s\n' "$volume_create_output"
assert_contains "$volume_create_output" "$volume_name" "volume create should report the new volume"

run_capture volume_list_output "$cratebay_bin" volume list
printf '%s\n' "$volume_list_output"
assert_contains "$volume_list_output" "$volume_name" "volume list should show the new volume"

run_capture volume_inspect_output "$cratebay_bin" volume inspect "$volume_name"
printf '%s\n' "$volume_inspect_output"
assert_contains "$volume_inspect_output" "Volume: $volume_name" "volume inspect should show the created volume"

run_capture volume_remove_output "$cratebay_bin" volume remove "$volume_name"
printf '%s\n' "$volume_remove_output"
assert_contains "$volume_remove_output" "$volume_name" "volume remove should report the deleted volume"
volume_removed=1

echo "== Network lifecycle =="
run_capture network_create_output "$cratebay_bin" network create "$network_name"
printf '%s\n' "$network_create_output"
assert_contains "$network_create_output" "$network_name" "network create should report the new network"

run_capture network_list_output "$cratebay_bin" network list
printf '%s\n' "$network_list_output"
assert_contains "$network_list_output" "$network_name" "network list should show the new network"

run_capture network_inspect_output "$cratebay_bin" --json network inspect "$network_name"
printf '%s\n' "$network_inspect_output"
assert_contains "$network_inspect_output" '"api": "cratebay.network.inspect.v1"' "network inspect should use the native CrateBay network inspect API"
assert_contains "$network_inspect_output" "$network_name" "network inspect should show the created network"

run_capture network_remove_output "$cratebay_bin" network remove "$network_name"
printf '%s\n' "$network_remove_output"
assert_contains "$network_remove_output" "$network_name" "network remove should report the deleted network"
network_removed=1

echo "== Stop and delete container =="
stop_output="$("$cratebay_bin" container stop "$container_name")"
printf '%s\n' "$stop_output"
assert_contains "$stop_output" "Stopped $container_name" "container stop should succeed"

delete_output="$("$cratebay_bin" container delete "$container_name")"
printf '%s\n' "$delete_output"
assert_contains "$delete_output" "$container_name" "container delete should succeed"
container_removed=1

pod_delete_output="$("$cratebay_bin" pod delete "$pod_name" --force)"
printf '%s\n' "$pod_delete_output"
assert_contains "$pod_delete_output" "$pod_name" "pod delete should succeed"
pod_removed=1

echo "== Image export/import round trip =="
export_archive="$TEMP_DIR/${tagged_image//[:\/]/-}.tar"
run_capture export_output "$cratebay_bin" --json image export --output "$export_archive" "$tagged_image"
printf '%s\n' "$export_output"
assert_contains "$export_output" '"api": "cratebay.image.export.v1"' "top-level image export should use the native image API"
assert_contains "$export_output" '"backend": "containerd"' "top-level image export should use containerd"
assert_contains "$export_output" "$export_archive" "image export should report the archive path"

"$cratebay_bin" image delete "$tagged_image" >/dev/null 2>&1 || true
"$cratebay_bin" image delete "$packaged_image" >/dev/null 2>&1 || true

run_capture import_output "$cratebay_bin" --json image import "$export_archive"
printf '%s\n' "$import_output"
assert_contains "$import_output" '"api": "cratebay.image.import.v1"' "top-level image import should use the native image API"
assert_contains "$import_output" '"backend": "containerd"' "top-level image import should use containerd"

roundtrip_image_list="$("$cratebay_bin" image list)"
printf '%s\n' "$roundtrip_image_list"
assert_contains "$roundtrip_image_list" "$tagged_image" "image import should restore the exported tag"

run_capture native_image_remove_output "$cratebay_bin" --json engine remove-image "$tagged_image" --force
printf '%s\n' "$native_image_remove_output"
assert_contains "$native_image_remove_output" '"api": "cratebay.image.remove.v1"' "native engine image remove should use the native image API"
assert_contains "$native_image_remove_output" '"backend": "containerd"' "native engine image remove should remove from containerd"

echo "CLI-only runtime smoke: PASS"
