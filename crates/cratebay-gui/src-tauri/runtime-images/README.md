# CrateBay Runtime Images (Bundled)

This directory is bundled into the desktop app as a resource.

At runtime, CrateBay installs the built-in runtime OS image by copying files
from the app bundle into the per-user data directory (`<data_dir>/images/...`).

## Expected layout

```
runtime-images/
  cratebay-runtime-aarch64/
    vmlinuz
    initramfs
    # optional (older bundles only):
    # rootfs.img
  cratebay-runtime-x86_64/
    vmlinuz
    initramfs
    # optional (older bundles only):
    # rootfs.img
```

Notes:

- Release builds should include **only the matching host arch** to keep the app smaller.
- The runtime OS image must boot a Linux guest that starts CrateBay Engine
  (`containerd`, `cratebay-engine-adapter`, and `cratebay-guest-agent`; see
  `docs/RUNTIME.md`).
- Newer "Runtime Lite" bundles are initramfs-first and do not require a
  prebuilt `rootfs.img`; the VM disk is created on first boot.
