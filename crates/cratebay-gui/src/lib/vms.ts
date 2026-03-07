import type { PortForwardDto } from "../types"

export function detectVmSshPort(portForwards: PortForwardDto[]): number | null {
  const sshForward = portForwards.find(
    (portForward) => portForward.guest_port === 22 && portForward.protocol.toLowerCase() === "tcp"
  )
  return sshForward ? sshForward.host_port : null
}
