export type ApiError = {
  error: string
}

export type User = {
  id: string
  email: string
  role: string
}

export type Session = {
  user: User
  csrf_token: string
}

export type FileItem = {
  id: string
  original_name: string
  mime: string
  size_bytes: number
  sha256: string
  created_at: string
  deleted_at: string | null
}

export type TagItem = {
  id: string
  name: string
  created_at: string
}

export type AlbumItem = {
  id: string
  name: string
  created_by: string
  created_at: string
}

export type AlbumDetail = {
  album: AlbumItem
  items: Array<{
    file_id: string
    position: number
    original_name: string
    mime: string
    size_bytes: number
    sha256: string
    created_at: string
  }>
}

export type DupeGroup = {
  sha256: string
  count: number
  size_bytes: number
  file_ids: string[]
}

export type StatsResponse = {
  files_count: number
  total_bytes: number
  deleted_count: number
  dupes_groups: number
}

export type AuditItem = {
  id: string
  user_id: string
  action: string
  target_type: string
  target_id: string
  meta_json: string
  created_at: string
}

export async function apiFetch<T>(
  path: string,
  opts: RequestInit & { csrf?: string } = {},
): Promise<T> {
  const headers = new Headers(opts.headers ?? {})
  if (opts.csrf) {
    headers.set('x-csrf-token', opts.csrf)
  }
  if (!headers.has('content-type') && opts.body && !(opts.body instanceof FormData)) {
    headers.set('content-type', 'application/json')
  }

  const res = await fetch(path, {
    ...opts,
    headers,
    credentials: 'include',
  })

  if (res.ok) {
    if (res.status === 204) return undefined as T
    const ct = res.headers.get('content-type') ?? ''
    if (ct.includes('application/json')) return (await res.json()) as T
    return (await res.text()) as T
  }

  let msg = `http_${res.status}`
  try {
    const data = (await res.json()) as ApiError
    if (data?.error) msg = data.error
  } catch {
    void 0
  }
  throw new Error(msg)
}

export async function login(email: string, password: string): Promise<Session> {
  const r = await apiFetch<{ csrf_token: string }>('/api/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })
  const s = await session()
  return { ...s, csrf_token: r.csrf_token }
}

export async function logout(csrf: string): Promise<void> {
  await apiFetch('/api/auth/logout', { method: 'POST', csrf })
}

export async function session(): Promise<Session> {
  return await apiFetch<Session>('/api/auth/session')
}

export async function listFiles(csrf: string, q?: string): Promise<FileItem[]> {
  const params = new URLSearchParams()
  if (q) params.set('q', q)
  const qs = params.toString()
  return await apiFetch<FileItem[]>(`/api/files${qs ? `?${qs}` : ''}`, { csrf })
}

export async function uploadFile(csrf: string, file: File): Promise<{ id: string }> {
  const fd = new FormData()
  fd.append('file', file)
  return await apiFetch<{ id: string }>('/api/files/upload', { method: 'POST', body: fd, csrf })
}

export async function deleteFile(csrf: string, id: string): Promise<void> {
  await apiFetch(`/api/files/${encodeURIComponent(id)}`, { method: 'DELETE', csrf })
}

export function downloadUrl(id: string): string {
  return `/api/files/${encodeURIComponent(id)}/download`
}

export async function listTags(csrf: string): Promise<TagItem[]> {
  return await apiFetch<TagItem[]>('/api/tags', { csrf })
}

export async function createTag(csrf: string, name: string): Promise<TagItem> {
  return await apiFetch<TagItem>('/api/tags', { method: 'POST', body: JSON.stringify({ name }), csrf })
}

export async function attachTag(csrf: string, file_id: string, tag_name: string): Promise<void> {
  await apiFetch('/api/tags/attach', { method: 'POST', body: JSON.stringify({ file_id, tag_name }), csrf })
}

export async function detachTag(csrf: string, file_id: string, tag_id: string): Promise<void> {
  await apiFetch('/api/tags/detach', { method: 'POST', body: JSON.stringify({ file_id, tag_id }), csrf })
}

export async function listAlbums(csrf: string): Promise<AlbumItem[]> {
  return await apiFetch<AlbumItem[]>('/api/albums', { csrf })
}

export async function createAlbum(csrf: string, name: string): Promise<AlbumItem> {
  return await apiFetch<AlbumItem>('/api/albums', { method: 'POST', body: JSON.stringify({ name }), csrf })
}

export async function getAlbum(csrf: string, id: string): Promise<AlbumDetail> {
  return await apiFetch<AlbumDetail>(`/api/albums/${encodeURIComponent(id)}`, { csrf })
}

export async function addAlbumItem(csrf: string, albumId: string, file_id: string, position?: number): Promise<void> {
  await apiFetch(`/api/albums/${encodeURIComponent(albumId)}/items`, {
    method: 'POST',
    body: JSON.stringify({ file_id, position }),
    csrf,
  })
}

export async function removeAlbumItem(csrf: string, albumId: string, fileId: string): Promise<void> {
  await apiFetch(`/api/albums/${encodeURIComponent(albumId)}/items/${encodeURIComponent(fileId)}`, { method: 'DELETE', csrf })
}

export async function dupeGroups(csrf: string): Promise<DupeGroup[]> {
  return await apiFetch<DupeGroup[]>('/api/dupes/groups', { csrf })
}

export async function applyDupes(csrf: string, keep_file_id: string, delete_file_ids: string[]): Promise<void> {
  await apiFetch('/api/dupes/apply', { method: 'POST', body: JSON.stringify({ keep_file_id, delete_file_ids }), csrf })
}

export async function createShare(
  csrf: string,
  kind: 'file' | 'album',
  target_id: string,
  expires_in_seconds?: number,
): Promise<{ url: string }> {
  return await apiFetch<{ url: string }>('/api/share', {
    method: 'POST',
    body: JSON.stringify({ kind, target_id, expires_in_seconds }),
    csrf,
  })
}

export async function adminStats(csrf: string): Promise<StatsResponse> {
  return await apiFetch<StatsResponse>('/api/admin/stats', { csrf })
}

export async function adminAudit(csrf: string): Promise<AuditItem[]> {
  return await apiFetch<AuditItem[]>('/api/admin/audit', { csrf })
}
