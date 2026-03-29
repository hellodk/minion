import { Component, createSignal, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId = 'projects' | 'upload' | 'youtube';

interface MediaProjectResponse {
  id: string;
  title: string;
  description: string | null;
  file_path: string;
  duration_seconds: number | null;
  codec: string | null;
  resolution: string | null;
  status: string;
  tags: string | null;
  created_at: string;
}

interface MediaMetadataResponse {
  file_path: string;
  file_name: string;
  file_size: number;
  media_type: string;
  duration_seconds: number | null;
  width: number | null;
  height: number | null;
  codec: string | null;
  bitrate: number | null;
  created_at: string | null;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDuration(seconds: number | null): string {
  if (seconds == null) return '--:--';
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  if (m >= 60) {
    const h = Math.floor(m / 60);
    const rm = m % 60;
    return `${h}h ${rm}m ${s}s`;
  }
  return `${m}:${s.toString().padStart(2, '0')}`;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function statusColor(status: string): string {
  switch (status) {
    case 'draft':
      return 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300';
    case 'processing':
      return 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300';
    case 'published':
      return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300';
    default:
      return 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300';
  }
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

const Media: Component = () => {
  const [activeTab, setActiveTab] = createSignal<TabId>('projects');
  const [projects, setProjects] = createSignal<MediaProjectResponse[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // Upload state
  const [selectedFile, setSelectedFile] = createSignal<string | null>(null);
  const [fileMeta, setFileMeta] = createSignal<MediaMetadataResponse | null>(null);
  const [uploadTitle, setUploadTitle] = createSignal('');
  const [uploadDesc, setUploadDesc] = createSignal('');
  const [uploadTags, setUploadTags] = createSignal('');
  const [importing, setImporting] = createSignal(false);
  const [importSuccess, setImportSuccess] = createSignal<MediaProjectResponse | null>(null);

  // Detail view
  const [expandedId, setExpandedId] = createSignal<string | null>(null);
  const [editingId, setEditingId] = createSignal<string | null>(null);
  const [editTitle, setEditTitle] = createSignal('');
  const [editDesc, setEditDesc] = createSignal('');
  const [editTags, setEditTags] = createSignal('');
  const [editStatus, setEditStatus] = createSignal('');

  // -------------------------------------------------------------------------
  // Data loading
  // -------------------------------------------------------------------------

  const loadProjects = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<MediaProjectResponse[]>('media_list_projects');
      setProjects(data);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(loadProjects);

  // -------------------------------------------------------------------------
  // Upload handlers
  // -------------------------------------------------------------------------

  const handleFileSelect = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const path = await open({
        multiple: false,
        filters: [{ name: 'Video Files', extensions: ['mp4', 'mkv', 'webm', 'avi', 'mov'] }],
      });
      if (path) {
        const filePath = typeof path === 'string' ? path : (path as any).path ?? String(path);
        setSelectedFile(filePath);
        setImportSuccess(null);
        // Get metadata
        try {
          const meta = await invoke<MediaMetadataResponse>('media_get_metadata', { path: filePath });
          setFileMeta(meta);
          // Pre-fill title from file name (without extension)
          const name = meta.file_name.replace(/\.[^.]+$/, '').replace(/[_-]/g, ' ');
          setUploadTitle(name);
        } catch {
          setFileMeta(null);
          const name = filePath.split('/').pop()?.replace(/\.[^.]+$/, '').replace(/[_-]/g, ' ') ?? '';
          setUploadTitle(name);
        }
      }
    } catch (e: any) {
      setError(String(e));
    }
  };

  const handleImport = async () => {
    const path = selectedFile();
    if (!path) return;
    setImporting(true);
    setError(null);
    try {
      const project = await invoke<MediaProjectResponse>('media_import_video', { path });
      // Update title/desc/tags if the user changed them
      const updates: Record<string, string> = {};
      if (uploadTitle() && uploadTitle() !== project.title) updates.title = uploadTitle();
      if (uploadDesc()) updates.description = uploadDesc();
      if (uploadTags()) updates.tags = uploadTags();

      if (Object.keys(updates).length > 0) {
        await invoke('media_update_project', {
          projectId: project.id,
          ...updates,
        });
        project.title = updates.title ?? project.title;
        project.description = updates.description ?? project.description ?? null;
        project.tags = updates.tags ?? project.tags ?? null;
      }

      setImportSuccess(project);
      setSelectedFile(null);
      setFileMeta(null);
      setUploadTitle('');
      setUploadDesc('');
      setUploadTags('');
      // Refresh project list
      await loadProjects();
    } catch (e: any) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  // -------------------------------------------------------------------------
  // Project actions
  // -------------------------------------------------------------------------

  const deleteProject = async (id: string) => {
    try {
      await invoke('media_delete_project', { projectId: id });
      setProjects((prev) => prev.filter((p) => p.id !== id));
      if (expandedId() === id) setExpandedId(null);
    } catch (e: any) {
      setError(String(e));
    }
  };

  const openVideo = async (path: string) => {
    try {
      await invoke('media_open_video', { path });
    } catch (e: any) {
      setError(String(e));
    }
  };

  const startEdit = (p: MediaProjectResponse) => {
    setEditingId(p.id);
    setEditTitle(p.title);
    setEditDesc(p.description ?? '');
    setEditTags(p.tags ?? '');
    setEditStatus(p.status);
  };

  const saveEdit = async () => {
    const id = editingId();
    if (!id) return;
    try {
      await invoke('media_update_project', {
        projectId: id,
        title: editTitle() || undefined,
        description: editDesc() || undefined,
        tags: editTags() || undefined,
        status: editStatus() || undefined,
      });
      setEditingId(null);
      await loadProjects();
    } catch (e: any) {
      setError(String(e));
    }
  };

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <div class="p-6 max-w-7xl mx-auto">
      {/* Header */}
      <div class="mb-6">
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Media Intelligence</h1>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
          Manage video projects, import media, and publish to YouTube
        </p>
      </div>

      {/* Error banner */}
      <Show when={error()}>
        <div class="mb-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-sm flex items-center justify-between">
          <span>{error()}</span>
          <button onClick={() => setError(null)} class="ml-2 text-red-500 hover:text-red-700">&times;</button>
        </div>
      </Show>

      {/* Tabs */}
      <div class="flex gap-1 mb-6 bg-gray-100 dark:bg-gray-800 rounded-lg p-1 w-fit">
        {(['projects', 'upload', 'youtube'] as TabId[]).map((tab) => (
          <button
            onClick={() => {
              setActiveTab(tab);
              if (tab === 'projects') loadProjects();
            }}
            class="px-4 py-2 rounded-md text-sm font-medium transition-colors"
            classList={{
              'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm': activeTab() === tab,
              'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white': activeTab() !== tab,
            }}
          >
            {tab === 'projects' ? 'Projects' : tab === 'upload' ? 'Upload' : 'YouTube'}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <Show when={activeTab() === 'projects'}>
        <ProjectsTab
          projects={projects()}
          loading={loading()}
          expandedId={expandedId()}
          editingId={editingId()}
          editTitle={editTitle()}
          editDesc={editDesc()}
          editTags={editTags()}
          editStatus={editStatus()}
          onToggleExpand={(id) => setExpandedId(expandedId() === id ? null : id)}
          onDelete={deleteProject}
          onOpen={openVideo}
          onStartEdit={startEdit}
          onSaveEdit={saveEdit}
          onCancelEdit={() => setEditingId(null)}
          onEditTitle={setEditTitle}
          onEditDesc={setEditDesc}
          onEditTags={setEditTags}
          onEditStatus={setEditStatus}
          onRefresh={loadProjects}
        />
      </Show>

      <Show when={activeTab() === 'upload'}>
        <UploadTab
          selectedFile={selectedFile()}
          fileMeta={fileMeta()}
          uploadTitle={uploadTitle()}
          uploadDesc={uploadDesc()}
          uploadTags={uploadTags()}
          importing={importing()}
          importSuccess={importSuccess()}
          onSelectFile={handleFileSelect}
          onImport={handleImport}
          onTitleChange={setUploadTitle}
          onDescChange={setUploadDesc}
          onTagsChange={setUploadTags}
          onViewProject={(p) => {
            setActiveTab('projects');
            setExpandedId(p.id);
            loadProjects();
          }}
        />
      </Show>

      <Show when={activeTab() === 'youtube'}>
        <YouTubeTab />
      </Show>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Projects Tab
// ---------------------------------------------------------------------------

interface ProjectsTabProps {
  projects: MediaProjectResponse[];
  loading: boolean;
  expandedId: string | null;
  editingId: string | null;
  editTitle: string;
  editDesc: string;
  editTags: string;
  editStatus: string;
  onToggleExpand: (id: string) => void;
  onDelete: (id: string) => void;
  onOpen: (path: string) => void;
  onStartEdit: (p: MediaProjectResponse) => void;
  onSaveEdit: () => void;
  onCancelEdit: () => void;
  onEditTitle: (v: string) => void;
  onEditDesc: (v: string) => void;
  onEditTags: (v: string) => void;
  onEditStatus: (v: string) => void;
  onRefresh: () => void;
}

const ProjectsTab: Component<ProjectsTabProps> = (props) => {
  return (
    <div>
      <div class="flex items-center justify-between mb-4">
        <p class="text-sm text-gray-500 dark:text-gray-400">
          {props.projects.length} project{props.projects.length !== 1 ? 's' : ''}
        </p>
        <button
          onClick={props.onRefresh}
          class="text-sm px-3 py-1.5 rounded-lg bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
        >
          Refresh
        </button>
      </div>

      <Show when={props.loading}>
        <div class="text-center py-12 text-gray-400">Loading projects...</div>
      </Show>

      <Show when={!props.loading && props.projects.length === 0}>
        <div class="text-center py-16">
          <div class="w-16 h-16 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center mx-auto mb-4">
            <svg class="w-8 h-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
          </div>
          <p class="text-gray-500 dark:text-gray-400 mb-2">No media projects yet</p>
          <p class="text-sm text-gray-400 dark:text-gray-500">Import a video to get started</p>
        </div>
      </Show>

      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        <For each={props.projects}>
          {(project) => (
            <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 overflow-hidden hover:shadow-md transition-shadow">
              {/* Thumbnail placeholder */}
              <div
                class="h-36 bg-gradient-to-br from-gray-200 to-gray-300 dark:from-gray-700 dark:to-gray-600 flex items-center justify-center cursor-pointer relative"
                onClick={() => props.onToggleExpand(project.id)}
              >
                <svg class="w-12 h-12 text-gray-400 dark:text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <Show when={project.duration_seconds != null}>
                  <span class="absolute bottom-2 right-2 text-xs bg-black/70 text-white px-1.5 py-0.5 rounded">
                    {formatDuration(project.duration_seconds)}
                  </span>
                </Show>
              </div>

              {/* Card info */}
              <div class="p-4">
                <div class="flex items-start justify-between gap-2 mb-2">
                  <h3
                    class="font-semibold text-gray-900 dark:text-white truncate cursor-pointer hover:text-blue-600 dark:hover:text-blue-400"
                    onClick={() => props.onToggleExpand(project.id)}
                  >
                    {project.title}
                  </h3>
                  <span class={`text-xs px-2 py-0.5 rounded-full whitespace-nowrap ${statusColor(project.status)}`}>
                    {project.status}
                  </span>
                </div>
                <div class="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400">
                  <Show when={project.resolution}>
                    <span>{project.resolution}</span>
                  </Show>
                  <Show when={project.codec}>
                    <span>{project.codec}</span>
                  </Show>
                  <span>{formatDate(project.created_at)}</span>
                </div>
                <Show when={project.tags}>
                  <div class="mt-2 flex flex-wrap gap-1">
                    <For each={project.tags!.split(',').map((t) => t.trim()).filter(Boolean)}>
                      {(tag) => (
                        <span class="text-xs bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 px-1.5 py-0.5 rounded">
                          {tag}
                        </span>
                      )}
                    </For>
                  </div>
                </Show>
              </div>

              {/* Expanded details */}
              <Show when={props.expandedId === project.id}>
                <div class="px-4 pb-4 border-t border-gray-100 dark:border-gray-700 pt-3 space-y-3">
                  <Show when={props.editingId === project.id} fallback={
                    <>
                      <Show when={project.description}>
                        <p class="text-sm text-gray-600 dark:text-gray-300">{project.description}</p>
                      </Show>
                      <div class="text-xs text-gray-500 dark:text-gray-400 space-y-1">
                        <p><span class="font-medium">Path:</span> {project.file_path}</p>
                      </div>
                      <div class="flex gap-2 pt-1">
                        <button
                          onClick={() => props.onOpen(project.file_path)}
                          class="text-xs px-3 py-1.5 rounded-lg bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900/50 transition-colors"
                        >
                          Open Video
                        </button>
                        <button
                          onClick={() => props.onStartEdit(project)}
                          class="text-xs px-3 py-1.5 rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                        >
                          Edit
                        </button>
                        <button
                          onClick={() => {
                            if (confirm('Delete this project? The video file will not be removed.')) {
                              props.onDelete(project.id);
                            }
                          }}
                          class="text-xs px-3 py-1.5 rounded-lg bg-red-50 dark:bg-red-900/30 text-red-600 dark:text-red-400 hover:bg-red-100 dark:hover:bg-red-900/50 transition-colors"
                        >
                          Delete
                        </button>
                      </div>
                    </>
                  }>
                    {/* Edit form */}
                    <div class="space-y-2">
                      <input
                        type="text"
                        value={props.editTitle}
                        onInput={(e) => props.onEditTitle(e.currentTarget.value)}
                        class="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 outline-none"
                        placeholder="Title"
                      />
                      <textarea
                        value={props.editDesc}
                        onInput={(e) => props.onEditDesc(e.currentTarget.value)}
                        class="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 outline-none resize-none"
                        rows={2}
                        placeholder="Description"
                      />
                      <input
                        type="text"
                        value={props.editTags}
                        onInput={(e) => props.onEditTags(e.currentTarget.value)}
                        class="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 outline-none"
                        placeholder="Tags (comma-separated)"
                      />
                      <select
                        value={props.editStatus}
                        onChange={(e) => props.onEditStatus(e.currentTarget.value)}
                        class="w-full px-3 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 outline-none"
                      >
                        <option value="draft">Draft</option>
                        <option value="processing">Processing</option>
                        <option value="published">Published</option>
                      </select>
                      <div class="flex gap-2">
                        <button
                          onClick={props.onSaveEdit}
                          class="text-xs px-3 py-1.5 rounded-lg bg-blue-600 text-white hover:bg-blue-700 transition-colors"
                        >
                          Save
                        </button>
                        <button
                          onClick={props.onCancelEdit}
                          class="text-xs px-3 py-1.5 rounded-lg bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  </Show>
                </div>
              </Show>
            </div>
          )}
        </For>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Upload Tab
// ---------------------------------------------------------------------------

interface UploadTabProps {
  selectedFile: string | null;
  fileMeta: MediaMetadataResponse | null;
  uploadTitle: string;
  uploadDesc: string;
  uploadTags: string;
  importing: boolean;
  importSuccess: MediaProjectResponse | null;
  onSelectFile: () => void;
  onImport: () => void;
  onTitleChange: (v: string) => void;
  onDescChange: (v: string) => void;
  onTagsChange: (v: string) => void;
  onViewProject: (p: MediaProjectResponse) => void;
}

const UploadTab: Component<UploadTabProps> = (props) => {
  return (
    <div class="max-w-2xl">
      {/* Success message */}
      <Show when={props.importSuccess}>
        {(project) => (
          <div class="mb-6 p-4 rounded-xl bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
            <div class="flex items-center gap-3">
              <div class="w-8 h-8 rounded-full bg-green-100 dark:bg-green-900/40 flex items-center justify-center">
                <svg class="w-5 h-5 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <div>
                <p class="font-medium text-green-800 dark:text-green-300">Video imported successfully!</p>
                <p class="text-sm text-green-600 dark:text-green-400">{project().title}</p>
              </div>
              <button
                onClick={() => props.onViewProject(project())}
                class="ml-auto text-sm px-3 py-1.5 rounded-lg bg-green-600 text-white hover:bg-green-700 transition-colors"
              >
                View Project
              </button>
            </div>
          </div>
        )}
      </Show>

      {/* Drop zone / file selector */}
      <Show when={!props.selectedFile} fallback={
        <div class="mb-6 p-4 rounded-xl bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700">
          <div class="flex items-center gap-3 mb-4">
            <div class="w-10 h-10 rounded-lg bg-blue-50 dark:bg-blue-900/30 flex items-center justify-center">
              <svg class="w-5 h-5 text-blue-600 dark:text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
              </svg>
            </div>
            <div class="flex-1 min-w-0">
              <p class="font-medium text-gray-900 dark:text-white truncate">{props.fileMeta?.file_name ?? props.selectedFile}</p>
              <div class="flex gap-3 text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                <Show when={props.fileMeta}>
                  <span>{formatFileSize(props.fileMeta!.file_size)}</span>
                  <span>{props.fileMeta!.media_type}</span>
                </Show>
              </div>
            </div>
            <button
              onClick={props.onSelectFile}
              class="text-sm text-blue-600 dark:text-blue-400 hover:underline"
            >
              Change
            </button>
          </div>

          {/* Import form */}
          <div class="space-y-3">
            <div>
              <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Title</label>
              <input
                type="text"
                value={props.uploadTitle}
                onInput={(e) => props.onTitleChange(e.currentTarget.value)}
                class="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 outline-none"
                placeholder="Video title"
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Description</label>
              <textarea
                value={props.uploadDesc}
                onInput={(e) => props.onDescChange(e.currentTarget.value)}
                class="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 outline-none resize-none"
                rows={3}
                placeholder="Describe your video..."
              />
            </div>
            <div>
              <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Tags</label>
              <input
                type="text"
                value={props.uploadTags}
                onInput={(e) => props.onTagsChange(e.currentTarget.value)}
                class="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 outline-none"
                placeholder="Comma-separated tags"
              />
            </div>
            <button
              onClick={props.onImport}
              disabled={props.importing}
              class="w-full py-2.5 rounded-lg bg-blue-600 text-white font-medium text-sm hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {props.importing ? 'Importing...' : 'Import Video'}
            </button>
          </div>
        </div>
      }>
        <div
          onClick={props.onSelectFile}
          class="mb-6 p-12 rounded-xl border-2 border-dashed border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-center cursor-pointer hover:border-blue-400 dark:hover:border-blue-500 hover:bg-blue-50/50 dark:hover:bg-blue-900/10 transition-colors"
        >
          <div class="w-16 h-16 rounded-full bg-gray-100 dark:bg-gray-700 flex items-center justify-center mx-auto mb-4">
            <svg class="w-8 h-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
          </div>
          <p class="font-medium text-gray-700 dark:text-gray-300 mb-1">Select a video file</p>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            Supported: MP4, MKV, WebM, AVI, MOV
          </p>
        </div>
      </Show>
    </div>
  );
};

// ---------------------------------------------------------------------------
// YouTube Tab
// ---------------------------------------------------------------------------

const YouTubeTab: Component = () => {
  return (
    <div class="max-w-2xl">
      {/* Connection status */}
      <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6 mb-6">
        <div class="flex items-center gap-4">
          <div class="w-12 h-12 rounded-xl bg-red-50 dark:bg-red-900/30 flex items-center justify-center">
            <svg class="w-7 h-7 text-red-600 dark:text-red-400" viewBox="0 0 24 24" fill="currentColor">
              <path d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"/>
            </svg>
          </div>
          <div class="flex-1">
            <h3 class="font-semibold text-gray-900 dark:text-white">YouTube Integration</h3>
            <p class="text-sm text-gray-500 dark:text-gray-400">
              Connect your YouTube account to upload and manage videos
            </p>
          </div>
          <a
            href="/settings"
            class="px-4 py-2 rounded-lg bg-red-600 text-white text-sm font-medium hover:bg-red-700 transition-colors"
          >
            Connect YouTube
          </a>
        </div>
      </div>

      {/* Placeholder upload form */}
      <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="font-semibold text-gray-900 dark:text-white mb-4">Upload to YouTube</h3>
        <div class="space-y-3">
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Select Project</label>
            <select class="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white outline-none opacity-50 cursor-not-allowed" disabled>
              <option>Connect YouTube first</option>
            </select>
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Visibility</label>
            <select class="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white outline-none opacity-50 cursor-not-allowed" disabled>
              <option value="private">Private</option>
              <option value="unlisted">Unlisted</option>
              <option value="public">Public</option>
            </select>
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Category</label>
            <select class="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white outline-none opacity-50 cursor-not-allowed" disabled>
              <option value="22">People &amp; Blogs</option>
              <option value="28">Science &amp; Technology</option>
              <option value="27">Education</option>
              <option value="24">Entertainment</option>
              <option value="20">Gaming</option>
              <option value="10">Music</option>
            </select>
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Schedule Date (optional)</label>
            <input
              type="datetime-local"
              class="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700 text-gray-900 dark:text-white outline-none opacity-50 cursor-not-allowed"
              disabled
            />
          </div>
          <div class="flex gap-2 pt-2">
            <button
              class="flex-1 py-2.5 rounded-lg bg-red-600 text-white font-medium text-sm opacity-50 cursor-not-allowed"
              disabled
            >
              Publish Now
            </button>
            <button
              class="flex-1 py-2.5 rounded-lg border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 font-medium text-sm opacity-50 cursor-not-allowed"
              disabled
            >
              Schedule
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Media;
