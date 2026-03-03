import { useState, useMemo } from "react"
import { I } from "../icons"
import { ErrorBanner } from "../components/ErrorDisplay"
import { EmptyState } from "../components/EmptyState"
import type { VolumeInfo } from "../types"

interface VolumesProps {
  volumes: VolumeInfo[]
  loading: boolean
  error: string
  onFetch: () => void
  onCreate: (name: string, driver: string) => Promise<VolumeInfo>
  onInspect: (name: string) => Promise<VolumeInfo>
  onRemove: (name: string) => Promise<void>
  onToast: (msg: string) => void
  t: (key: string) => string
}

export function Volumes({
  volumes, loading, error,
  onFetch, onCreate, onInspect, onRemove, onToast, t,
}: VolumesProps) {
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [createName, setCreateName] = useState("")
  const [createDriver, setCreateDriver] = useState("local")
  const [createLoading, setCreateLoading] = useState(false)
  const [createError, setCreateError] = useState("")

  const [inspectVolume, setInspectVolume] = useState<VolumeInfo | null>(null)
  const [inspectLoading, setInspectLoading] = useState(false)

  const [confirmDelete, setConfirmDelete] = useState("")
  const [search, setSearch] = useState("")

  const filtered = useMemo(() => {
    if (!search.trim()) return volumes
    const q = search.toLowerCase()
    return volumes.filter(v =>
      v.name.toLowerCase().includes(q) ||
      v.driver.toLowerCase().includes(q) ||
      v.mountpoint?.toLowerCase().includes(q)
    )
  }, [volumes, search])

  const openCreateModal = () => {
    setCreateName("")
    setCreateDriver("local")
    setCreateError("")
    setShowCreateModal(true)
  }

  const handleCreate = async () => {
    if (!createName.trim()) return
    setCreateLoading(true)
    setCreateError("")
    try {
      await onCreate(createName.trim(), createDriver)
      onToast(t("volumeCreated"))
      setShowCreateModal(false)
    } catch (e) {
      setCreateError(String(e))
    } finally {
      setCreateLoading(false)
    }
  }

  const handleInspect = async (name: string) => {
    setInspectLoading(true)
    try {
      const vol = await onInspect(name)
      setInspectVolume(vol)
    } catch (e) {
      onToast(String(e))
    } finally {
      setInspectLoading(false)
    }
  }

  const handleDelete = async (name: string) => {
    try {
      await onRemove(name)
      onToast(t("volumeDeleted"))
    } catch (e) {
      onToast(String(e))
    } finally {
      setConfirmDelete("")
    }
  }

  const formatDate = (dateStr: string) => {
    if (!dateStr) return "-"
    try {
      const d = new Date(dateStr)
      return d.toLocaleString()
    } catch {
      return dateStr
    }
  }

  const labelsCount = (v: VolumeInfo) => {
    if (!v.labels) return 0
    return Object.keys(v.labels).length
  }

  if (loading) {
    return <div className="loading"><div className="spinner" />{t("loading")}</div>
  }
  if (error) {
    return (
      <ErrorBanner
        title={t("connectionError")}
        message={error}
        actionLabel={t("refresh")}
        onAction={onFetch}
      />
    )
  }

  return (
    <div className="page">
      {/* Toolbar */}
      <div className="toolbar">
        <input
          className="input toolbar-search"
          placeholder={t("searchVolumes")}
          value={search}
          onChange={e => setSearch(e.target.value)}
        />
        <div className="toolbar-spacer" />
        <button type="button" className="btn" onClick={onFetch}>
          <span className="icon">{I.refresh}</span>{t("refresh")}
        </button>
        <button type="button" className="btn primary" onClick={openCreateModal}>
          <span className="icon">{I.plus}</span>{t("createVolume")}
        </button>
      </div>

      {/* Volume List */}
      {volumes.length === 0 ? (
        <EmptyState
          icon={I.hardDrive}
          title={t("noVolumes")}
          description={t("createFirstVolume")}
          code="docker volume create my-volume"
        />
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={I.hardDrive}
          title={t("noResults")}
          description={search}
        />
      ) : (
        <div className="image-list">
          {filtered.map(v => {
            const lc = labelsCount(v)
            return (
              <div className="image-item" key={v.name}>
                <div className="image-item-main">
                  <div className="image-item-icon">
                    {I.hardDrive}
                  </div>
                  <div className="image-item-body">
                    <div className="image-item-name">{v.name}</div>
                    <div className="image-item-meta">
                      <span>{v.driver}</span>
                      <span className="meta-sep" />
                      <span>{v.scope || "local"}</span>
                      {lc > 0 && (
                        <>
                          <span className="meta-sep" />
                          <span>{lc} {lc === 1 ? "label" : "labels"}</span>
                        </>
                      )}
                      {v.created_at && (
                        <>
                          <span className="meta-sep" />
                          <span>{formatDate(v.created_at)}</span>
                        </>
                      )}
                    </div>
                  </div>
                </div>
                <div className="image-item-actions">
                  <div className="image-item-actions-group">
                    <button
                      type="button"
                      className="action-btn"
                      onClick={() => handleInspect(v.name)}
                      title={t("inspectVolume")}
                      disabled={inspectLoading}
                    >
                      {I.fileText}
                      <span className="action-label">{t("inspect")}</span>
                    </button>
                    <button
                      type="button"
                      className="action-btn"
                      onClick={() => {
                        navigator.clipboard.writeText(v.name)
                        onToast(t("copied"))
                      }}
                      title={t("copyName")}
                    >
                      {I.copy}
                      <span className="action-label">{t("copy")}</span>
                    </button>
                  </div>
                  <div className="image-item-actions-sep" />
                  <button
                    type="button"
                    className="action-btn danger"
                    onClick={() => setConfirmDelete(v.name)}
                    title={t("deleteVolume")}
                  >
                    {I.trash}
                    <span className="action-label">{t("delete")}</span>
                  </button>
                </div>
              </div>
            )
          })}
        </div>
      )}

      {/* Create Volume Modal */}
      {showCreateModal && (
        <div className="modal-backdrop" onClick={() => setShowCreateModal(false)}>
          <div className="modal vol-modal-sm" onClick={e => e.stopPropagation()}>
            <div className="modal-head">
              <div className="modal-title">{t("createVolume")}</div>
              <div className="modal-actions">
                <button type="button" className="icon-btn" onClick={() => setShowCreateModal(false)} title={t("close")}>x</button>
              </div>
            </div>
            <div className="modal-body">
              <div className="form">
                <div className="row">
                  <label>{t("volumeName")}</label>
                  <input
                    className="input"
                    value={createName}
                    onChange={e => setCreateName(e.target.value)}
                    placeholder="my-volume"
                    autoFocus
                    onKeyDown={e => { if (e.key === "Enter") handleCreate() }}
                  />
                </div>
                <div className="row">
                  <label>{t("driver")}</label>
                  <select className="select" value={createDriver} onChange={e => setCreateDriver(e.target.value)}>
                    <option value="local">local</option>
                  </select>
                </div>
              </div>
              {createError && <div className="hint vol-error">{createError}</div>}
            </div>
            <div className="modal-footer">
              <button type="button" className="btn" onClick={() => setShowCreateModal(false)}>{t("close")}</button>
              <button type="button" className="btn primary vol-footer-btn" disabled={createLoading || !createName.trim()} onClick={handleCreate}>
                {createLoading ? t("creating") : t("create")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Inspect Volume Modal */}
      {inspectVolume && (
        <div className="modal-backdrop" onClick={() => setInspectVolume(null)}>
          <div className="modal vol-modal-md" onClick={e => e.stopPropagation()}>
            <div className="modal-head">
              <div className="modal-title">{t("volumeDetails")} — {inspectVolume.name}</div>
              <div className="modal-actions">
                <button
                  type="button"
                  className="icon-btn"
                  onClick={() => {
                    navigator.clipboard.writeText(JSON.stringify(inspectVolume, null, 2))
                    onToast(t("copied"))
                  }}
                  title={t("copyJson")}
                >
                  {I.copy}
                </button>
                <button type="button" className="icon-btn" onClick={() => setInspectVolume(null)} title={t("close")}>x</button>
              </div>
            </div>
            <div className="modal-body vol-modal-body-flush">
              <div className="vol-inspect-body">
                <div className="form">
                  <div className="row two">
                    <div className="row">
                      <label>{t("volumeName")}</label>
                      <div className="hint vol-inspect-value">{inspectVolume.name}</div>
                    </div>
                    <div className="row">
                      <label>{t("driver")}</label>
                      <div className="hint vol-inspect-value normal">{inspectVolume.driver}</div>
                    </div>
                  </div>
                  <div className="row two">
                    <div className="row">
                      <label>Scope</label>
                      <div className="hint vol-inspect-value normal">{inspectVolume.scope || "local"}</div>
                    </div>
                    <div className="row">
                      <label>{t("created")}</label>
                      <div className="hint vol-inspect-value normal">{formatDate(inspectVolume.created_at)}</div>
                    </div>
                  </div>
                  <div className="row">
                    <label>{t("mountpoint")}</label>
                    <code className="vol-mountpoint">
                      {inspectVolume.mountpoint || "-"}
                    </code>
                  </div>
                  {inspectVolume.labels && Object.keys(inspectVolume.labels).length > 0 && (
                    <div className="row">
                      <label>Labels</label>
                      <div className="vol-kv-list">
                        {Object.entries(inspectVolume.labels).map(([k, val]) => (
                          <code key={k} className="vol-kv-item">
                            <span className="vol-kv-key">{k}</span>
                            <span className="vol-kv-sep"> = </span>
                            <span className="vol-kv-val">{val}</span>
                          </code>
                        ))}
                      </div>
                    </div>
                  )}
                  {inspectVolume.options && Object.keys(inspectVolume.options).length > 0 && (
                    <div className="row">
                      <label>Options</label>
                      <div className="vol-kv-list">
                        {Object.entries(inspectVolume.options).map(([k, val]) => (
                          <code key={k} className="vol-kv-item">
                            <span className="vol-kv-key">{k}</span>
                            <span className="vol-kv-sep"> = </span>
                            <span className="vol-kv-val">{val}</span>
                          </code>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              </div>
              <div className="vol-raw-header">
                <span className="vol-raw-label">RAW JSON</span>
              </div>
              <pre className="modal-pre vol-raw-json">{JSON.stringify(inspectVolume, null, 2)}</pre>
            </div>
            <div className="modal-footer">
              <button type="button" className="btn" onClick={() => setInspectVolume(null)}>{t("close")}</button>
            </div>
          </div>
        </div>
      )}

      {/* Confirm Delete Modal */}
      {confirmDelete && (
        <div className="modal-backdrop" onClick={() => setConfirmDelete("")}>
          <div className="modal vol-modal-xs" onClick={e => e.stopPropagation()}>
            <div className="modal-head">
              <div className="modal-title">{t("deleteVolume")}</div>
              <div className="modal-actions">
                <button type="button" className="icon-btn" onClick={() => setConfirmDelete("")} title={t("close")}>x</button>
              </div>
            </div>
            <div className="modal-body">
              <p className="vol-confirm-text">{t("confirmDeleteVolume")}</p>
              <p className="vol-delete-name">{confirmDelete}</p>
            </div>
            <div className="modal-footer">
              <button type="button" className="btn" onClick={() => setConfirmDelete("")}>{t("close")}</button>
              <button type="button" className="btn primary vol-footer-btn vol-btn-danger" onClick={() => handleDelete(confirmDelete)}>
                {I.trash}
                <span className="vol-delete-btn-icon">{t("delete")}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
