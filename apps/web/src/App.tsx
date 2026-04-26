import './App.css'
import { useEffect, useMemo, useState } from 'react'
import type { AlbumDetail, AlbumItem, DupeGroup, FileItem, User } from './api'
import {
  adminAudit,
  adminStats,
  addAlbumItem,
  applyDupes,
  createAlbum,
  createShare,
  dupeGroups,
  getAlbum,
  listAlbums,
  listFiles,
  login,
  logout,
  session,
  uploadFile,
} from './api'

function App() {
  const [me, setMe] = useState<User | null>(null)
  const [csrf, setCsrf] = useState<string>('')
  const [tab, setTab] = useState<'library' | 'upload' | 'albums' | 'dupes' | 'admin'>('library')
  const [selected, setSelected] = useState<Record<string, boolean>>({})
  const selectedIds = useMemo(() => Object.keys(selected).filter((k) => selected[k]), [selected])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string>('')

  useEffect(() => {
    ;(async () => {
      try {
        const s = await session()
        setMe(s.user)
        setCsrf(s.csrf_token)
      } catch {
        void 0
      }
      setLoading(false)
    })()
  }, [])

  async function handleLogout() {
    setError('')
    try {
      await logout(csrf)
    } catch {
      void 0
    }
    setMe(null)
    setCsrf('')
    setSelected({})
    setTab('library')
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">MediaVault</div>
        {me ? (
          <div className="userbar">
            <div className="user">{me.email}</div>
            <button className="btn" onClick={handleLogout}>
              Logout
            </button>
          </div>
        ) : null}
      </header>

      {loading ? <div className="card">Loading…</div> : null}

      {!loading && !me ? (
        <LoginPanel
          onLogin={(u, c) => {
            setMe(u)
            setCsrf(c)
          }}
        />
      ) : null}

      {!loading && me ? (
        <div className="layout">
          <nav className="sidebar">
            <button className={tab === 'library' ? 'nav active' : 'nav'} onClick={() => setTab('library')}>
              Library
            </button>
            <button className={tab === 'upload' ? 'nav active' : 'nav'} onClick={() => setTab('upload')}>
              Upload
            </button>
            <button className={tab === 'albums' ? 'nav active' : 'nav'} onClick={() => setTab('albums')}>
              Albums
            </button>
            <button className={tab === 'dupes' ? 'nav active' : 'nav'} onClick={() => setTab('dupes')}>
              Duplicates
            </button>
            {me.role === 'admin' ? (
              <button className={tab === 'admin' ? 'nav active' : 'nav'} onClick={() => setTab('admin')}>
                Admin
              </button>
            ) : null}
          </nav>

          <main className="main">
            {error ? <div className="error">{error}</div> : null}
            {tab === 'library' ? (
              <LibraryPanel
                csrf={csrf}
                selected={selected}
                setSelected={setSelected}
                onShare={async (id) => {
                  setError('')
                  try {
                    const r = await createShare(csrf, 'file', id)
                    await navigator.clipboard.writeText(r.url)
                    setError(`Share link copied: ${r.url}`)
                  } catch (e) {
                    setError((e as Error).message)
                  }
                }}
              />
            ) : null}
            {tab === 'upload' ? <UploadPanel csrf={csrf} onError={setError} /> : null}
            {tab === 'albums' ? (
              <AlbumsPanel csrf={csrf} selectedFileIds={selectedIds} onError={setError} />
            ) : null}
            {tab === 'dupes' ? <DupesPanel csrf={csrf} onError={setError} /> : null}
            {tab === 'admin' && me.role === 'admin' ? <AdminPanel csrf={csrf} onError={setError} /> : null}
          </main>
        </div>
      ) : null}
    </div>
  )
}

function LoginPanel(props: { onLogin: (user: User, csrf: string) => void }) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError('')
    try {
      const s = await login(email, password)
      props.onLogin(s.user, s.csrf_token)
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="center">
      <form className="card form" onSubmit={submit}>
        <h1>Login</h1>
        {error ? <div className="error">{error}</div> : null}
        <label className="field">
          <div className="label">Email</div>
          <input className="input" value={email} onChange={(e) => setEmail(e.target.value)} autoComplete="email" />
        </label>
        <label className="field">
          <div className="label">Password</div>
          <input
            className="input"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
          />
        </label>
        <button className="btn primary" type="submit" disabled={busy}>
          {busy ? 'Signing in…' : 'Sign in'}
        </button>
      </form>
    </div>
  )
}

function LibraryPanel(props: {
  csrf: string
  selected: Record<string, boolean>
  setSelected: React.Dispatch<React.SetStateAction<Record<string, boolean>>>
  onShare: (id: string) => void
}) {
  const [items, setItems] = useState<FileItem[]>([])
  const [q, setQ] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')

  async function refresh() {
    setBusy(true)
    setError('')
    try {
      const r = await listFiles(props.csrf, q || undefined)
      setItems(r)
    } catch (e) {
      setError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  useEffect(() => {
    const t = setTimeout(() => {
      void refresh()
    }, 0)
    return () => clearTimeout(t)
  }, [])

  return (
    <div className="card">
      <div className="row">
        <div className="title">Library</div>
        <div className="spacer" />
        <input className="input small" placeholder="Search…" value={q} onChange={(e) => setQ(e.target.value)} />
        <button className="btn" onClick={refresh} disabled={busy}>
          {busy ? '…' : 'Refresh'}
        </button>
      </div>
      {error ? <div className="error">{error}</div> : null}
      <div className="table">
        <div className="thead">
          <div />
          <div>Name</div>
          <div>Type</div>
          <div>Size</div>
          <div />
        </div>
        {items.map((f) => (
          <div key={f.id} className="trow">
            <input
              type="checkbox"
              checked={!!props.selected[f.id]}
              onChange={(e) => props.setSelected((s) => ({ ...s, [f.id]: e.target.checked }))}
            />
            <div className="mono">{f.original_name}</div>
            <div className="mono">{f.mime}</div>
            <div className="mono">{formatBytes(f.size_bytes)}</div>
            <div className="actions">
              <a className="btn" href={`/api/files/${encodeURIComponent(f.id)}/download`}>
                Download
              </a>
              <button className="btn" onClick={() => props.onShare(f.id)}>
                Share
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

function UploadPanel(props: { csrf: string; onError: (s: string) => void }) {
  const [busy, setBusy] = useState(false)
  const [done, setDone] = useState<Array<{ name: string; id: string }>>([])

  async function onFiles(files: FileList | null) {
    if (!files?.length) return
    setBusy(true)
    props.onError('')
    try {
      const out: Array<{ name: string; id: string }> = []
      for (const f of Array.from(files)) {
        const r = await uploadFile(props.csrf, f)
        out.push({ name: f.name, id: r.id })
      }
      setDone(out)
    } catch (e) {
      props.onError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="card">
      <div className="row">
        <div className="title">Upload</div>
        <div className="spacer" />
        <label className="btn primary">
          Choose files
          <input type="file" multiple onChange={(e) => onFiles(e.target.files)} style={{ display: 'none' }} />
        </label>
      </div>
      {busy ? <div className="muted">Uploading…</div> : null}
      {done.length ? (
        <div className="list">
          {done.map((x) => (
            <div key={x.id} className="mono">
              {x.name} → {x.id}
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}

function AlbumsPanel(props: { csrf: string; selectedFileIds: string[]; onError: (s: string) => void }) {
  const [albums, setAlbums] = useState<AlbumItem[]>([])
  const [activeId, setActiveId] = useState<string>('')
  const [active, setActive] = useState<AlbumDetail | null>(null)
  const [name, setName] = useState('')

  async function refresh() {
    props.onError('')
    const r = await listAlbums(props.csrf)
    setAlbums(r)
    if (activeId) {
      const d = await getAlbum(props.csrf, activeId)
      setActive(d)
    }
  }

  useEffect(() => {
    const t = setTimeout(() => {
      void refresh().catch((e) => props.onError((e as Error).message))
    }, 0)
    return () => clearTimeout(t)
  }, [])

  async function createNew() {
    props.onError('')
    try {
      const a = await createAlbum(props.csrf, name)
      setName('')
      setActiveId(a.id)
      const d = await getAlbum(props.csrf, a.id)
      setActive(d)
      await refresh()
    } catch (e) {
      props.onError((e as Error).message)
    }
  }

  async function openAlbum(id: string) {
    props.onError('')
    try {
      setActiveId(id)
      const d = await getAlbum(props.csrf, id)
      setActive(d)
    } catch (e) {
      props.onError((e as Error).message)
    }
  }

  async function addSelected() {
    if (!activeId) return
    props.onError('')
    try {
      for (const id of props.selectedFileIds) {
        await addAlbumItem(props.csrf, activeId, id)
      }
      const d = await getAlbum(props.csrf, activeId)
      setActive(d)
    } catch (e) {
      props.onError((e as Error).message)
    }
  }

  return (
    <div className="grid2">
      <div className="card">
        <div className="row">
          <div className="title">Albums</div>
          <div className="spacer" />
          <button className="btn" onClick={() => refresh().catch((e) => props.onError((e as Error).message))}>
            Refresh
          </button>
        </div>
        <div className="list">
          {albums.map((a) => (
            <button key={a.id} className={a.id === activeId ? 'listItem active' : 'listItem'} onClick={() => openAlbum(a.id)}>
              <div className="mono">{a.name}</div>
              <div className="muted mono">{a.id}</div>
            </button>
          ))}
        </div>
        <div className="row">
          <input className="input" placeholder="New album name" value={name} onChange={(e) => setName(e.target.value)} />
          <button className="btn primary" onClick={createNew} disabled={!name.trim()}>
            Create
          </button>
        </div>
      </div>

      <div className="card">
        <div className="row">
          <div className="title">{active ? active.album.name : 'Album'}</div>
          <div className="spacer" />
          <button className="btn" onClick={addSelected} disabled={!activeId || props.selectedFileIds.length === 0}>
            Add selected ({props.selectedFileIds.length})
          </button>
          {activeId ? (
            <button
              className="btn"
              onClick={async () => {
                props.onError('')
                try {
                  const r = await createShare(props.csrf, 'album', activeId)
                  await navigator.clipboard.writeText(r.url)
                  props.onError(`Share link copied: ${r.url}`)
                } catch (e) {
                  props.onError((e as Error).message)
                }
              }}
            >
              Share
            </button>
          ) : null}
        </div>
        {active ? (
          <div className="list">
            {active.items.map((it) => (
              <div key={it.file_id} className="row">
                <div className="mono">{it.original_name}</div>
                <div className="spacer" />
                <a className="btn" href={`/api/files/${encodeURIComponent(it.file_id)}/download`}>
                  Download
                </a>
              </div>
            ))}
          </div>
        ) : (
          <div className="muted">Select an album</div>
        )}
      </div>
    </div>
  )
}

function DupesPanel(props: { csrf: string; onError: (s: string) => void }) {
  const [groups, setGroups] = useState<DupeGroup[]>([])
  const [busy, setBusy] = useState(false)

  async function refresh() {
    setBusy(true)
    props.onError('')
    try {
      const r = await dupeGroups(props.csrf)
      setGroups(r)
    } catch (e) {
      props.onError((e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  useEffect(() => {
    const t = setTimeout(() => {
      void refresh()
    }, 0)
    return () => clearTimeout(t)
  }, [])

  async function applyGroup(g: DupeGroup) {
    const keep = g.file_ids[0]
    const del = g.file_ids.slice(1)
    props.onError('')
    try {
      await applyDupes(props.csrf, keep, del)
      await refresh()
    } catch (e) {
      props.onError((e as Error).message)
    }
  }

  return (
    <div className="card">
      <div className="row">
        <div className="title">Duplicates</div>
        <div className="spacer" />
        <button className="btn" onClick={refresh} disabled={busy}>
          {busy ? '…' : 'Refresh'}
        </button>
      </div>
      <div className="list">
        {groups.map((g) => (
          <div key={g.sha256} className="card sub">
            <div className="row">
              <div className="mono">{g.sha256.slice(0, 16)}…</div>
              <div className="spacer" />
              <div className="mono">
                {g.count} × {formatBytes(g.size_bytes)}
              </div>
              <button className="btn danger" onClick={() => applyGroup(g)}>
                Keep first, delete rest
              </button>
            </div>
            <div className="list">
              {g.file_ids.map((id) => (
                <div key={id} className="mono">
                  {id}
                </div>
              ))}
            </div>
          </div>
        ))}
        {groups.length === 0 ? <div className="muted">No duplicates detected</div> : null}
      </div>
    </div>
  )
}

function AdminPanel(props: { csrf: string; onError: (s: string) => void }) {
  const [stats, setStats] = useState<Awaited<ReturnType<typeof adminStats>> | null>(null)
  const [audit, setAudit] = useState<Awaited<ReturnType<typeof adminAudit>>>([])

  async function refresh() {
    props.onError('')
    try {
      const s = await adminStats(props.csrf)
      const a = await adminAudit(props.csrf)
      setStats(s)
      setAudit(a)
    } catch (e) {
      props.onError((e as Error).message)
    }
  }

  useEffect(() => {
    const t = setTimeout(() => {
      void refresh()
    }, 0)
    return () => clearTimeout(t)
  }, [])

  return (
    <div className="grid2">
      <div className="card">
        <div className="row">
          <div className="title">Stats</div>
          <div className="spacer" />
          <button className="btn" onClick={refresh}>
            Refresh
          </button>
        </div>
        {stats ? (
          <div className="list">
            <div className="row">
              <div className="muted">Files</div>
              <div className="spacer" />
              <div className="mono">{stats.files_count}</div>
            </div>
            <div className="row">
              <div className="muted">Total</div>
              <div className="spacer" />
              <div className="mono">{formatBytes(stats.total_bytes)}</div>
            </div>
            <div className="row">
              <div className="muted">Deleted</div>
              <div className="spacer" />
              <div className="mono">{stats.deleted_count}</div>
            </div>
            <div className="row">
              <div className="muted">Dupe groups</div>
              <div className="spacer" />
              <div className="mono">{stats.dupes_groups}</div>
            </div>
          </div>
        ) : (
          <div className="muted">Loading…</div>
        )}
      </div>

      <div className="card">
        <div className="row">
          <div className="title">Audit</div>
          <div className="spacer" />
          <button className="btn" onClick={refresh}>
            Refresh
          </button>
        </div>
        <div className="list">
          {audit.slice(0, 50).map((x) => (
            <div key={x.id} className="mono">
              {x.created_at} {x.action} {x.target_type}:{x.target_id}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function formatBytes(n: number): string {
  const u = ['B', 'KB', 'MB', 'GB', 'TB']
  let x = n
  let i = 0
  while (x >= 1024 && i < u.length - 1) {
    x /= 1024
    i++
  }
  return `${x.toFixed(i === 0 ? 0 : 1)} ${u[i]}`
}

export default App
