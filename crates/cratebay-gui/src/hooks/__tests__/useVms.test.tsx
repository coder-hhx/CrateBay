import { describe, it, expect, vi, beforeEach } from "vitest"
import { renderHook, waitFor, act } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { useVms } from "../useVms"
import type { VmInfoDto, OsImageDto } from "../../types"

function mkVm(overrides: Partial<VmInfoDto> = {}): VmInfoDto {
  return {
    id: "vm-1",
    name: "vm-1",
    state: "running",
    cpus: 2,
    memory_mb: 2048,
    disk_gb: 20,
    rosetta_enabled: false,
    mounts: [],
    port_forwards: [],
    os_image: null,
    ...overrides,
  }
}

describe("useVms", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("infers SSH login port from guest port 22 forwarding", async () => {
    const vm = mkVm({
      port_forwards: [{ host_port: 2228, guest_port: 22, protocol: "tcp" }],
    })

    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "vm_list") return [vm] satisfies VmInfoDto[]
      if (cmd === "image_catalog") return [] satisfies OsImageDto[]
      if (cmd === "vm_login_cmd") return "ssh root@127.0.0.1 -p 2228"
      return null
    })

    const { result, unmount } = renderHook(() => useVms())
    await waitFor(() => expect(result.current.vms).toHaveLength(1))

    await act(async () => {
      result.current.setVmLoginPort("")
    })
    await act(async () => {
      await result.current.getLoginCmd(vm)
    })

    expect(vi.mocked(invoke)).toHaveBeenCalledWith("vm_login_cmd", {
      name: "vm-1",
      user: "root",
      host: "127.0.0.1",
      port: 2228,
      portForwards: [{ host_port: 2228, guest_port: 22, protocol: "tcp" }],
    })

    unmount()
  })

  it("prefers manually entered SSH port over inferred forwarding", async () => {
    const vm = mkVm({
      port_forwards: [{ host_port: 2228, guest_port: 22, protocol: "tcp" }],
    })

    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "vm_list") return [vm] satisfies VmInfoDto[]
      if (cmd === "image_catalog") return [] satisfies OsImageDto[]
      if (cmd === "vm_login_cmd") return "ssh root@127.0.0.1 -p 2200"
      return null
    })

    const { result, unmount } = renderHook(() => useVms())
    await waitFor(() => expect(result.current.vms).toHaveLength(1))

    await act(async () => {
      result.current.setVmLoginPort(2200)
    })
    await act(async () => {
      await result.current.getLoginCmd(vm)
    })

    expect(vi.mocked(invoke)).toHaveBeenCalledWith("vm_login_cmd", {
      name: "vm-1",
      user: "root",
      host: "127.0.0.1",
      port: 2200,
      portForwards: [{ host_port: 2228, guest_port: 22, protocol: "tcp" }],
    })

    unmount()
  })
})
