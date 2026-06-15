# CrateBay Windows WSL Runtime Assets (Bundled)

This directory is bundled into the desktop app as a resource.

CrateBay's Windows runtime uses **WSL2** with a custom distro containing CrateBay Engine
(`containerd` + `runc` + CNI + the CrateBay compatibility adapter).
Provisioning imports a deterministic `rootfs.tar` as a WSL distro.

## Expected layout

```
runtime-wsl/
  cratebay-runtime-wsl-x86_64/
    rootfs.tar
  cratebay-runtime-wsl-aarch64/
    rootfs.tar
```

## How to fetch / generate (maintainers / CI)

Fetch prebuilt assets from GitHub Releases (default tag can be overridden via
`CRATEBAY_RUNTIME_TAG`):

```
./scripts/fetch-wsl-runtime-assets.sh x86_64
./scripts/fetch-wsl-runtime-assets.sh aarch64
```
