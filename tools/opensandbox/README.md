# OpenSandbox (Local) — Optional Compatibility Reference for CrateBay

CrateBay’s primary sandbox path is the built-in **managed sandbox** flow backed by Docker.

This folder stays as an **optional external compatibility reference** for local experiments and status visibility only. It is **not** part of the current `v1.0` critical path.

## What you get here

- `sandbox.example.toml`: a conservative example config (Docker runtime, host/bridge networking notes).

## Local install/run (optional, experimental)

1) Install `opensandbox-server`

- With `uv` (recommended by OpenSandbox docs):

```bash
uv pip install opensandbox-server
```

- Or with `pip`:

```bash
python3 -m pip install --user opensandbox-server
```

2) Create a config file

```bash
cp ./sandbox.example.toml ~/.sandbox.toml
```

3) Start the server

```bash
opensandbox-server --config ~/.sandbox.toml
```

If you want to experiment locally, the API docs are usually exposed at:

- `http://localhost:8080/docs`

## Notes

- **Current roadmap**: CrateBay is not prioritizing a full OpenSandbox lifecycle integration in the current milestone.
- **Docker socket access**: OpenSandbox needs to talk to Docker to create sandboxes.
- **Networking mode**:
  - `host`: simplest; higher performance; typically single active sandbox at a time.
  - `bridge`: isolated networking; requires correct `host_ip` for endpoint resolution in some deployments.
