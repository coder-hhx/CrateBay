import { type JSX, useState, useEffect, useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import { I } from "../icons"
import { ErrorInline } from "../components/ErrorDisplay"
import type { K3sStatusDto, K8sPod, K8sService, K8sDeployment } from "../types"

interface KubernetesProps {
  t: (key: string) => string
}

type K8sTab = "overview" | "pods" | "services" | "deployments"

export function Kubernetes({ t }: KubernetesProps) {
  // K3s cluster state
  const [status, setStatus] = useState<K3sStatusDto | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState("")
  const [acting, setActing] = useState("")

  // K8s dashboard state
  const [tab, setTab] = useState<K8sTab>("overview")
  const [namespace, setNamespace] = useState("")
  const [namespaces, setNamespaces] = useState<string[]>([])
  const [pods, setPods] = useState<K8sPod[]>([])
  const [services, setServices] = useState<K8sService[]>([])
  const [deployments, setDeployments] = useState<K8sDeployment[]>([])
  const [k8sLoading, setK8sLoading] = useState(false)
  const [k8sError, setK8sError] = useState("")

  // Pod logs modal
  const [logPod, setLogPod] = useState<K8sPod | null>(null)
  const [podLogs, setPodLogs] = useState("")
  const [logsLoading, setLogsLoading] = useState(false)

  // ── K3s status polling ──────────────────────────────────────────────
  const fetchStatus = useCallback(async () => {
    try {
      const s = await invoke<K3sStatusDto>("k3s_status")
      setStatus(s)
      setError("")
    } catch (e: unknown) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchStatus()
    const iv = setInterval(fetchStatus, 5000)
    return () => clearInterval(iv)
  }, [fetchStatus])

  const doAction = async (action: string, label: string) => {
    setActing(label)
    setError("")
    try {
      await invoke(action)
      await fetchStatus()
    } catch (e: unknown) {
      setError(String(e))
    } finally {
      setActing("")
    }
  }

  // ── K8s dashboard data fetching ─────────────────────────────────────
  const fetchK8sData = useCallback(async () => {
    if (!status?.running) return
    setK8sLoading(true)
    setK8sError("")
    try {
      const [ns, p, s, d] = await Promise.all([
        invoke<string[]>("k8s_list_namespaces"),
        invoke<K8sPod[]>("k8s_list_pods", { namespace: namespace || null }),
        invoke<K8sService[]>("k8s_list_services", { namespace: namespace || null }),
        invoke<K8sDeployment[]>("k8s_list_deployments", { namespace: namespace || null }),
      ])
      setNamespaces(ns)
      setPods(p)
      setServices(s)
      setDeployments(d)
    } catch (e: unknown) {
      setK8sError(String(e))
    } finally {
      setK8sLoading(false)
    }
  }, [status?.running, namespace])

  useEffect(() => {
    if (status?.running) {
      fetchK8sData()
      const iv = setInterval(fetchK8sData, 10000)
      return () => clearInterval(iv)
    }
  }, [fetchK8sData, status?.running])

  const fetchPodLogs = async (pod: K8sPod) => {
    setLogPod(pod)
    setLogsLoading(true)
    setPodLogs("")
    try {
      const logs = await invoke<string>("k8s_pod_logs", {
        name: pod.name,
        namespace: pod.namespace,
        tail: 200,
      })
      setPodLogs(logs)
    } catch (e: unknown) {
      setPodLogs(`Error: ${String(e)}`)
    } finally {
      setLogsLoading(false)
    }
  }

  const isInstalled = status?.installed ?? false
  const isRunning = status?.running ?? false

  const tabCounts: Record<K8sTab, number> = {
    overview: 0,
    pods: pods.length,
    services: services.length,
    deployments: deployments.length,
  }

  const statusClass = (s: string) =>
    s === "Running" ? "running" : s === "Pending" ? "pending" : "failed"

  return (
    <div className="page">
      {/* Toolbar */}
      <div className="toolbar">
        <button type="button" className="btn" onClick={fetchStatus} disabled={loading}>
          <span className="icon">{I.refresh}</span>
          {loading ? t("loading") : t("refresh")}
        </button>
        {isRunning && (
          <>
            <div className="k8s-toolbar-sep" />
            <select
              className="k8s-ns-select"
              value={namespace}
              onChange={(e) => setNamespace(e.target.value)}
              title={t("namespace")}
            >
              <option value="">{t("allNamespaces")}</option>
              {namespaces.map((ns) => (
                <option key={ns} value={ns}>{ns}</option>
              ))}
            </select>
          </>
        )}
        <div className="toolbar-spacer" />
      </div>

      {error && <ErrorInline message={error} onDismiss={() => setError("")} />}
      {k8sError && <ErrorInline message={k8sError} onDismiss={() => setK8sError("")} />}

      {/* K3s Cluster Status Card */}
      <div className="k8s-cluster-card">
        <div className="k8s-cluster-header">
          <div className={`k8s-cluster-icon${isRunning ? "" : " stopped"}`}>
            {I.kubernetes}
          </div>
          <div className="k8s-cluster-title">
            <h2>{t("k3sCluster")}</h2>
            <div className="k8s-cluster-title-sub">
              {status?.version ? `v${status.version}` : t("notInstalled")}
            </div>
          </div>
          <span className={`k8s-status-badge ${isRunning ? "running" : "stopped"}`}>
            <span className={`dot ${isRunning ? "running" : "stopped"}`} />
            {isRunning ? t("running") : t("stopped")}
          </span>
        </div>

        <div className="k8s-cluster-body">
          <div className="vm-stats-grid">
            <div className="vm-stat-card">
              <div className="vm-stat-label">{t("clusterStatus")}</div>
              <div className="vm-stat-value">
                {isInstalled ? (
                  <span className="k8s-stat-installed">{t("installed")}</span>
                ) : (
                  <span className="k8s-stat-not-installed">{t("notInstalled")}</span>
                )}
              </div>
            </div>
            <div className="vm-stat-card">
              <div className="vm-stat-label">{t("k3sVersion")}</div>
              <div className="vm-stat-value mono">
                {status?.version || "-"}
              </div>
            </div>
            <div className="vm-stat-card">
              <div className="vm-stat-label">{t("nodeCount")}</div>
              <div className="vm-stat-value">
                {isRunning ? status?.node_count ?? 0 : "-"}
              </div>
            </div>
            <div className="vm-stat-card">
              <div className="vm-stat-label">{t("kubeconfig")}</div>
              <div className="vm-stat-value mono k8s-stat-kubeconfig">
                {status?.kubeconfig_path || "-"}
              </div>
            </div>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="k8s-cluster-actions">
          {!isInstalled && (
            <button
              type="button"
              className="btn primary"
              disabled={!!acting}
              onClick={() => doAction("k3s_install", "install")}
            >
              <span className="icon">{I.plus}</span>
              {acting === "install" ? t("working") : t("installK3s")}
            </button>
          )}
          {isInstalled && !isRunning && (
            <button
              type="button"
              className="btn primary"
              disabled={!!acting}
              onClick={() => doAction("k3s_start", "start")}
            >
              <span className="icon">{I.play}</span>
              {acting === "start" ? t("working") : t("startCluster")}
            </button>
          )}
          {isRunning && (
            <button
              type="button"
              className="btn"
              disabled={!!acting}
              onClick={() => doAction("k3s_stop", "stop")}
            >
              <span className="icon">{I.stop}</span>
              {acting === "stop" ? t("working") : t("stopCluster")}
            </button>
          )}
          {isInstalled && (
            <button
              type="button"
              className="btn"
              disabled={!!acting || isRunning}
              onClick={() => doAction("k3s_uninstall", "uninstall")}
              title={isRunning ? t("stopCluster") : ""}
            >
              <span className="icon">{I.trash}</span>
              {acting === "uninstall" ? t("working") : t("uninstallK3s")}
            </button>
          )}
        </div>
      </div>

      {/* K8s Dashboard (only when cluster is running) */}
      {isRunning && (
        <div className="k8s-dashboard">
          {/* Dashboard Header */}
          <div className="k8s-dashboard-header">
            <div className="k8s-dashboard-title">
              <span className="k8s-dashboard-title-icon">{I.layers}</span>
              <h3>{t("k3sCluster")} Dashboard</h3>
            </div>
            <div className="k8s-dashboard-actions">
              {k8sLoading && (
                <div className="k8s-tab-loading">
                  <div className="spinner" />
                </div>
              )}
              <button type="button" className="btn xs" onClick={fetchK8sData} disabled={k8sLoading}>
                <span className="icon">{I.refresh}</span>
                {t("refresh")}
              </button>
            </div>
          </div>

          {/* Tabs */}
          <div className="k8s-tabs">
            {(["overview", "pods", "services", "deployments"] as K8sTab[]).map((t2) => {
              const tabIcons: Record<K8sTab, JSX.Element> = {
                overview: I.dashboard,
                pods: I.box,
                services: I.globe,
                deployments: I.layers,
              }
              return (
                <button
                  type="button"
                  key={t2}
                  className={`k8s-tab${tab === t2 ? " active" : ""}`}
                  onClick={() => setTab(t2)}
                >
                  <span className="k8s-tab-icon">{tabIcons[t2]}</span>
                  {t(t2)}
                  {t2 !== "overview" && (
                    <span className="k8s-tab-count">{tabCounts[t2]}</span>
                  )}
                </button>
              )
            })}
          </div>

          <div className="k8s-tab-content">
            {/* Overview Tab */}
            {tab === "overview" && (
              <div className="k8s-overview-grid">
                <div className="k8s-overview-card" onClick={() => setTab("pods")}>
                  <div className="k8s-overview-card-header">
                    <div className="k8s-overview-card-icon purple">{I.box}</div>
                    <div className="k8s-overview-card-label">{t("pods")}</div>
                  </div>
                  <div className="k8s-overview-card-body">
                    <div className="k8s-overview-card-value">{pods.length}</div>
                    <div className="k8s-overview-card-hint">
                      {pods.filter(p => p.status === "Running").length} {t("running").toLowerCase()}
                    </div>
                  </div>
                  <div className="k8s-overview-card-accent purple" />
                </div>
                <div className="k8s-overview-card" onClick={() => setTab("services")}>
                  <div className="k8s-overview-card-header">
                    <div className="k8s-overview-card-icon cyan">{I.globe}</div>
                    <div className="k8s-overview-card-label">{t("services")}</div>
                  </div>
                  <div className="k8s-overview-card-body">
                    <div className="k8s-overview-card-value">{services.length}</div>
                    <div className="k8s-overview-card-hint">
                      {services.filter(s => s.service_type === "ClusterIP").length} ClusterIP
                    </div>
                  </div>
                  <div className="k8s-overview-card-accent cyan" />
                </div>
                <div className="k8s-overview-card" onClick={() => setTab("deployments")}>
                  <div className="k8s-overview-card-header">
                    <div className="k8s-overview-card-icon green">{I.layers}</div>
                    <div className="k8s-overview-card-label">{t("deployments")}</div>
                  </div>
                  <div className="k8s-overview-card-body">
                    <div className="k8s-overview-card-value">{deployments.length}</div>
                    <div className="k8s-overview-card-hint">
                      {deployments.filter(d => d.available > 0).length} {t("available").toLowerCase()}
                    </div>
                  </div>
                  <div className="k8s-overview-card-accent green" />
                </div>
                <div className="k8s-overview-card no-click">
                  <div className="k8s-overview-card-header">
                    <div className="k8s-overview-card-icon neutral">{I.server}</div>
                    <div className="k8s-overview-card-label">{t("namespace")}</div>
                  </div>
                  <div className="k8s-overview-card-body">
                    <div className="k8s-overview-card-value">{namespaces.length}</div>
                    <div className="k8s-overview-card-hint">
                      {namespace || t("allNamespaces").toLowerCase()}
                    </div>
                  </div>
                  <div className="k8s-overview-card-accent neutral" />
                </div>
              </div>
            )}

            {/* Pods Tab */}
            {tab === "pods" && (
              <div className="k8s-table-wrap">
                {pods.length === 0 ? (
                  <div className="k8s-empty">
                    <div className="k8s-empty-icon">{I.box}</div>
                    <div className="k8s-empty-text">{t("noPods")}</div>
                    <div className="k8s-empty-sub">No pod resources found in the current namespace</div>
                  </div>
                ) : (
                  <>
                    <div className="k8s-table-info">
                      <span className="k8s-table-info-count">{pods.length}</span> {t("pods")}
                      <span className="k8s-table-info-sep" />
                      <span className="k8s-table-info-running">{pods.filter(p => p.status === "Running").length} {t("running").toLowerCase()}</span>
                    </div>
                    <div className="k8s-table-scroll">
                      <table className="k8s-table">
                        <thead>
                          <tr>
                            <th>{t("name")}</th>
                            <th>{t("namespace")}</th>
                            <th>{t("status")}</th>
                            <th>{t("ready")}</th>
                            <th>{t("restarts")}</th>
                            <th>{t("age")}</th>
                            <th className="th-actions">{t("actions")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {pods.map((pod) => (
                            <tr key={`${pod.namespace}/${pod.name}`}>
                              <td className="mono td-name">{pod.name}</td>
                              <td><span className="k8s-ns-badge">{pod.namespace}</span></td>
                              <td>
                                <span className={`k8s-pod-status ${statusClass(pod.status)}`}>
                                  <span className="status-dot" />
                                  {pod.status}
                                </span>
                              </td>
                              <td className="mono">{pod.ready}</td>
                              <td>
                                <span className={`k8s-restarts${pod.restarts > 0 ? " warn" : ""}`}>
                                  {pod.restarts}
                                </span>
                              </td>
                              <td className="td-age">{pod.age}</td>
                              <td className="td-actions">
                                <button type="button" className="btn xs" onClick={() => fetchPodLogs(pod)}>
                                  <span className="icon">{I.terminal}</span>
                                  {t("logs")}
                                </button>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </>
                )}
              </div>
            )}

            {/* Services Tab */}
            {tab === "services" && (
              <div className="k8s-table-wrap">
                {services.length === 0 ? (
                  <div className="k8s-empty">
                    <div className="k8s-empty-icon">{I.globe}</div>
                    <div className="k8s-empty-text">{t("noServices")}</div>
                    <div className="k8s-empty-sub">No service resources found in the current namespace</div>
                  </div>
                ) : (
                  <>
                    <div className="k8s-table-info">
                      <span className="k8s-table-info-count">{services.length}</span> {t("services")}
                    </div>
                    <div className="k8s-table-scroll">
                      <table className="k8s-table">
                        <thead>
                          <tr>
                            <th>{t("name")}</th>
                            <th>{t("namespace")}</th>
                            <th>{t("type")}</th>
                            <th>{t("clusterIp")}</th>
                            <th>{t("ports")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {services.map((svc) => (
                            <tr key={`${svc.namespace}/${svc.name}`}>
                              <td className="mono td-name">{svc.name}</td>
                              <td><span className="k8s-ns-badge">{svc.namespace}</span></td>
                              <td>
                                <span className={`k8s-svc-type ${svc.service_type.toLowerCase()}`}>{svc.service_type}</span>
                              </td>
                              <td className="mono">{svc.cluster_ip}</td>
                              <td className="mono">{svc.ports}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </>
                )}
              </div>
            )}

            {/* Deployments Tab */}
            {tab === "deployments" && (
              <div className="k8s-table-wrap">
                {deployments.length === 0 ? (
                  <div className="k8s-empty">
                    <div className="k8s-empty-icon">{I.layers}</div>
                    <div className="k8s-empty-text">{t("noDeployments")}</div>
                    <div className="k8s-empty-sub">No deployment resources found in the current namespace</div>
                  </div>
                ) : (
                  <>
                    <div className="k8s-table-info">
                      <span className="k8s-table-info-count">{deployments.length}</span> {t("deployments")}
                      <span className="k8s-table-info-sep" />
                      <span className="k8s-table-info-running">{deployments.filter(d => d.available > 0).length} {t("available").toLowerCase()}</span>
                    </div>
                    <div className="k8s-table-scroll">
                      <table className="k8s-table">
                        <thead>
                          <tr>
                            <th>{t("name")}</th>
                            <th>{t("namespace")}</th>
                            <th>{t("ready")}</th>
                            <th>{t("upToDate")}</th>
                            <th>{t("available")}</th>
                            <th>{t("age")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {deployments.map((dep) => (
                            <tr key={`${dep.namespace}/${dep.name}`}>
                              <td className="mono td-name">{dep.name}</td>
                              <td><span className="k8s-ns-badge">{dep.namespace}</span></td>
                              <td>
                                <span className={`k8s-dep-ready ${dep.available > 0 ? "ok" : "warn"}`}>{dep.ready}</span>
                              </td>
                              <td className="mono">{dep.up_to_date}</td>
                              <td>
                                <span className={`k8s-dep-avail ${dep.available > 0 ? "ok" : "warn"}`}>{dep.available}</span>
                              </td>
                              <td className="td-age">{dep.age}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Pod Logs Modal */}
      {logPod && (
        <div className="modal-backdrop" onClick={() => setLogPod(null)}>
          <div
            className="k8s-logs-modal"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="k8s-logs-header">
              <div className="k8s-logs-title">
                <div className="k8s-logs-title-icon">{I.terminal}</div>
                <div className="k8s-logs-title-text">
                  <h3>{t("podLogs")}</h3>
                  <span className="logs-pod-name">{logPod.name}</span>
                </div>
              </div>
              <div className="k8s-logs-actions">
                <span className={`k8s-pod-status ${statusClass(logPod.status)}`}>
                  <span className="status-dot" />
                  {logPod.status}
                </span>
                <div className="k8s-logs-actions-sep" />
                <button
                  type="button"
                  className="btn xs"
                  onClick={() => navigator.clipboard.writeText(podLogs)}
                  title="Copy logs"
                >
                  <span className="icon">{I.copy}</span>
                  Copy
                </button>
                <button type="button" className="btn xs" onClick={() => fetchPodLogs(logPod)} title="Refresh logs">
                  <span className="icon">{I.refresh}</span>
                </button>
                <button type="button" className="btn xs" onClick={() => setLogPod(null)}>
                  {t("close")}
                </button>
              </div>
            </div>
            <div className="k8s-logs-body">
              {logsLoading ? (
                <div className="k8s-logs-loading">
                  <div className="spinner" />
                  {t("loading")}
                </div>
              ) : (
                <pre className="k8s-logs-content">
                  {podLogs || t("noLogs")}
                </pre>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
