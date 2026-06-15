import { Plus, Download, Settings, Hexagon, Play, Square, Trash2, Copy } from 'lucide';

// Tauri injects window.__TAURI__ when withGlobalTauri is true
const invoke = window.__TAURI__?.core?.invoke ?? (() => Promise.reject('Tauri not available'));
const listen = window.__TAURI__?.event?.listen ?? (() => {});

// ─── Lucide icons (rendered from imported icon data) ─────

const LUCIDE = {
  plus: Plus,
  download: Download,
  settings: Settings,
  hexagon: Hexagon,
  play: Play,
  square: Square,
  trash: Trash2,
  copy: Copy,
};

// Each lucide export is an array of `[tag, attrs]` SVG primitives that go
// inside a standard <svg> shell.
function icon(name, opts = {}) {
  const children = LUCIDE[name];
  if (!children) return '';
  const size = opts.size ?? 16;
  const cls = opts.cls ? ` class="${opts.cls}"` : '';
  const inner = children.map(([tag, attrs]) => {
    const a = Object.entries(attrs).map(([k, v]) => `${k}="${v}"`).join(' ');
    return `<${tag} ${a}/>`;
  }).join('');
  return `<svg${cls} xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 24 24" fill="${opts.fill ?? 'none'}" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${inner}</svg>`;
}

function mountIcon(id, name, opts) {
  const el = document.getElementById(id);
  if (el) el.innerHTML = icon(name, opts);
}

// ─── State ───────────────────────────────────────────────

let projects = [];
let running = new Set();
let selected = null; // ProjectInfo | null

// name → { artifacts, startTime, animId, error }
const compilingState = new Map();

// name → GitStatus (cached). Refreshed on demand.
const gitStatusCache = new Map();

// Pending modal targets
let pendingDeleteName = null;
let pendingCopyName = null;
let pendingRemoteName = null;

const RING_RADIUS = 9;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

// ─── Data loading ─────────────────────────────────────────

async function loadProjects() {
  try {
    const [list, run, comp] = await Promise.all([
      invoke('list_projects'),
      invoke('get_running'),
      invoke('get_compiling'),
    ]);
    projects = list;
    running = new Set(run);
    for (const name of comp) {
      if (!compilingState.has(name)) {
        compilingState.set(name, { artifacts: 0, startTime: Date.now(), animId: null, error: null });
      }
    }
  } catch (e) {
    console.error('Failed to load projects:', e);
    projects = [];
    running = new Set();
  }
  renderAll();
}

async function pollRunning() {
  try {
    const names = await invoke('get_running');
    running = new Set(names);
    renderSidebar();
    if (selected && !compilingState.has(selected.name)) renderContent();
  } catch (e) {
    // silently ignore poll failures
  }
}

async function refreshGitStatus(name) {
  try {
    const status = await invoke('git_status_cmd', { name });
    gitStatusCache.set(name, status);
    if (selected?.name === name) renderContent();
  } catch (e) {
    gitStatusCache.set(name, { initialized: false });
    if (selected?.name === name) renderContent();
  }
}

// ─── Event listeners ──────────────────────────────────────

listen('compile-progress', (event) => {
  const { name, artifacts } = event.payload;
  const state = compilingState.get(name);
  if (state) state.artifacts = artifacts;
  if (selected?.name === name) updateRingDisplay(name);
  renderSidebar();
});

listen('compile-result', (event) => {
  const { name, success, cancelled, error } = event.payload;
  const state = compilingState.get(name);
  if (!state) return;

  if (success) {
    setRingProgress(name, 1.0);
    setTimeout(() => {
      compilingState.delete(name);
      running.add(name);
      renderAll();
    }, 400);
  } else if (cancelled) {
    compilingState.delete(name);
    renderAll();
  } else {
    state.error = error ?? 'Unknown error';
    renderAll();
  }
});

// ─── Ring animation ───────────────────────────────────────

function ringProgress(name) {
  const state = compilingState.get(name);
  if (!state) return 0;
  const elapsed = Date.now() - state.startTime;
  const timeFraction = (1 - Math.exp(-elapsed / 25000)) * 0.85;
  const artifactFraction = (1 - Math.exp(-state.artifacts / 40)) * 0.90;
  return Math.max(timeFraction, artifactFraction);
}

function setRingProgress(name, progress) {
  const fg = document.getElementById(`ring-fg-${name}`);
  if (fg) fg.style.strokeDashoffset = RING_CIRCUMFERENCE * (1 - progress);
}

function statusIndicatorHtml(nameAttr, state) {
  return `
    <span class="status-indicator ${state}">
      <span class="status-indicator-ring-wrap">
        <svg class="status-indicator-ring" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
          <circle class="status-indicator-ring-bg" cx="12" cy="12" r="${RING_RADIUS}"/>
          <circle class="status-indicator-ring-fg" id="ring-fg-${nameAttr}" cx="12" cy="12" r="${RING_RADIUS}"
            stroke-dasharray="${RING_CIRCUMFERENCE} ${RING_CIRCUMFERENCE}"
            stroke-dashoffset="${RING_CIRCUMFERENCE}"/>
        </svg>
        <span class="status-indicator-dot"></span>
      </span>
      <span class="status-indicator-label">${state === 'compiling' ? 'Compiling' : 'Running'}</span>
    </span>
  `;
}

function updateRingDisplay(name) {
  setRingProgress(name, ringProgress(name));
}

function startRingLoop(name) {
  const state = compilingState.get(name);
  if (!state || state.animId !== null) return;

  function tick() {
    if (!compilingState.has(name)) return;
    const s = compilingState.get(name);
    if (s.error) return;
    updateRingDisplay(name);
    s.animId = requestAnimationFrame(tick);
  }
  state.animId = requestAnimationFrame(tick);
}

// ─── Rendering ───────────────────────────────────────────

function renderAll() {
  renderSidebar();
  renderContent();
}

function renderSidebar() {
  const list = document.getElementById('project-list');

  if (projects.length === 0) {
    list.innerHTML = '<div class="list-placeholder">No projects yet</div>';
    return;
  }

  list.innerHTML = '';
  for (const project of projects) {
    const isRunning = running.has(project.name);
    const isCompiling = compilingState.has(project.name);
    const isSelected = selected?.name === project.name;

    const item = document.createElement('div');
    item.className = 'project-item' + (isSelected ? ' selected' : '');

    const status = isRunning
      ? '<span class="running-dot" title="Running"></span>'
      : isCompiling
        ? '<span class="compiling-dot" title="Compiling"></span>'
        : '';

    const primaryAction = (isRunning || isCompiling)
      ? `<button class="hover-btn stop-btn" title="Stop" data-act="stop">${icon('square', { size: 13, fill: 'currentColor' })}</button>`
      : `<button class="hover-btn run-btn" title="Run" data-act="run">${icon('play', { size: 13, fill: 'currentColor' })}</button>`;

    item.innerHTML = `
      <span class="project-item-name">${escHtml(project.name)}</span>
      <span class="project-item-status">${status}</span>
      <span class="project-item-actions">
        ${primaryAction}
        <button class="hover-btn delete-btn" title="Delete" data-act="delete">${icon('trash', { size: 13 })}</button>
      </span>
    `;

    item.addEventListener('click', (e) => {
      if (e.target.closest('.project-item-actions')) return;
      selectProject(project);
    });

    item.querySelector('[data-act="run"]')?.addEventListener('click', (e) => {
      e.stopPropagation();
      runProject(project.name);
    });
    item.querySelector('[data-act="stop"]')?.addEventListener('click', (e) => {
      e.stopPropagation();
      if (isCompiling) stopCompile(project.name);
      else stopProject(project.name);
    });
    item.querySelector('[data-act="delete"]')?.addEventListener('click', (e) => {
      e.stopPropagation();
      showDeleteModal(project.name);
    });

    list.appendChild(item);
  }
}

function renderContent() {
  const content = document.getElementById('content');

  if (!selected) {
    content.innerHTML = `
      <div class="empty-state">
        <div class="empty-icon">${icon('hexagon', { size: 48 })}</div>
        <p>Select a project or create a new one</p>
        <div class="empty-state-actions">
          <button class="btn btn-primary" id="btn-new-empty">New Project</button>
          <button class="btn btn-ghost" id="btn-import-empty">Import</button>
        </div>
      </div>
    `;
    document.getElementById('btn-new-empty').addEventListener('click', showNewModal);
    document.getElementById('btn-import-empty').addEventListener('click', showCloneModal);
    return;
  }

  const isRunning = running.has(selected.name);
  const compState = compilingState.get(selected.name);
  const isCompiling = !!compState && !compState.error;
  const hasError = !!compState?.error;

  const headerHtml = `
    <div class="project-detail-header">
      <div>
        <div class="project-detail-name">${escHtml(selected.name)}</div>
        <div class="project-detail-path">${escHtml(selected.path)}</div>
      </div>
      <div class="project-detail-menu">
        <button class="icon-btn" id="btn-copy" title="Copy project">${icon('copy')}</button>
        <button class="icon-btn" id="btn-delete" title="Delete project">${icon('trash')}</button>
      </div>
    </div>
  `;

  const runIcon = icon('play', { size: 14, fill: 'currentColor' });
  const stopIcon = icon('square', { size: 14, fill: 'currentColor' });

  let actionsHtml = '';
  if (hasError) {
    actionsHtml = `
      <div class="project-actions">
        <button class="btn btn-primary" id="btn-run">${runIcon} Run</button>
        <button class="btn btn-ghost" id="btn-editor">Open in Editor</button>
      </div>
      <div class="compile-error-box">
        <span class="compile-error-label">Build failed</span>
        <span class="compile-error-msg">${escHtml(compState.error)}</span>
      </div>
    `;
  } else if (isCompiling || isRunning) {
    const nameAttr = escHtml(selected.name);
    const stopBtnId = isCompiling ? 'btn-stop-compile' : 'btn-stop';
    actionsHtml = `
      <div class="project-actions">
        <button class="btn btn-danger" id="${stopBtnId}">${stopIcon} Stop</button>
        <button class="btn btn-ghost" id="btn-editor">Open in Editor</button>
        ${statusIndicatorHtml(nameAttr, isCompiling ? 'compiling' : 'running')}
      </div>
    `;
  } else {
    actionsHtml = `
      <div class="project-actions">
        <button class="btn btn-primary" id="btn-run">${runIcon} Run</button>
        <button class="btn btn-ghost" id="btn-editor">Open in Editor</button>
      </div>
    `;
  }

  content.innerHTML = `
    <div class="project-detail">
      ${headerHtml}
      ${actionsHtml}
      ${renderGitPanel(selected.name)}
    </div>
  `;

  if (hasError) {
    compilingState.delete(selected.name);
    document.getElementById('btn-run').addEventListener('click', () => runProject(selected.name));
  } else if (isCompiling) {
    document.getElementById('btn-stop-compile').addEventListener('click', () => stopCompile(selected.name));
    startRingLoop(selected.name);
  } else if (isRunning) {
    document.getElementById('btn-stop').addEventListener('click', () => stopProject(selected.name));
  } else {
    document.getElementById('btn-run').addEventListener('click', () => runProject(selected.name));
  }
  document.getElementById('btn-editor').addEventListener('click', () => openEditor(selected.name));

  document.getElementById('btn-copy').addEventListener('click', () => showCopyModal(selected.name));
  document.getElementById('btn-delete').addEventListener('click', () => showDeleteModal(selected.name));

  wireGitPanel(selected.name);
}

function renderGitPanel(name) {
  const status = gitStatusCache.get(name);

  if (!status) {
    return `
      <div class="panel" id="git-panel">
        <div class="panel-header"><span class="panel-title">Git</span></div>
        <div class="panel-row"><span class="value-muted">Loading…</span></div>
      </div>
    `;
  }

  if (!status.initialized) {
    return `
      <div class="panel" id="git-panel">
        <div class="panel-header">
          <span class="panel-title">Git</span>
        </div>
        <div class="panel-row"><span class="value-muted">No repository</span></div>
        <div class="panel-actions">
          <button class="btn btn-ghost btn-sm" id="git-init">Init repository</button>
        </div>
      </div>
    `;
  }

  const branch = status.branch ? escHtml(status.branch) : '<span class="value-muted">detached</span>';
  const remote = status.remote
    ? `<span class="value">${escHtml(status.remote)}</span>`
    : `<span class="value-muted">none</span>`;

  const badges = [];
  badges.push(status.dirty
    ? `<span class="badge badge-dirty">uncommitted changes</span>`
    : `<span class="badge badge-clean">clean</span>`);
  if (status.ahead > 0) badges.push(`<span class="badge badge-ahead">↑ ${status.ahead}</span>`);
  if (status.behind > 0) badges.push(`<span class="badge badge-behind">↓ ${status.behind}</span>`);

  const remoteActions = status.remote
    ? `
      <button class="btn btn-primary btn-sm" id="git-sync">Sync (commit + push)</button>
      <button class="btn btn-ghost btn-sm" id="git-pull">Pull</button>
      <button class="btn btn-ghost btn-sm" id="git-remote">Change remote…</button>
    `
    : `<button class="btn btn-ghost btn-sm" id="git-remote">Set remote…</button>`;

  return `
    <div class="panel" id="git-panel">
      <div class="panel-header">
        <span class="panel-title">Git</span>
        <span style="display:flex;gap:6px;">${badges.join('')}</span>
      </div>
      <div class="panel-row"><span class="label">Branch</span><span class="value">${branch}</span></div>
      <div class="panel-row"><span class="label">Remote</span>${remote}</div>
      <div class="panel-actions">
        ${remoteActions}
      </div>
    </div>
  `;
}

function wireGitPanel(name) {
  const status = gitStatusCache.get(name);
  if (!status) {
    refreshGitStatus(name);
    return;
  }
  document.getElementById('git-init')?.addEventListener('click', () => gitInit(name));
  document.getElementById('git-sync')?.addEventListener('click', () => gitSync(name));
  document.getElementById('git-pull')?.addEventListener('click', () => gitPull(name));
  document.getElementById('git-remote')?.addEventListener('click', () => showRemoteModal(name, status.remote ?? ''));
}

function selectProject(project) {
  selected = project;
  if (!gitStatusCache.has(project.name)) {
    refreshGitStatus(project.name);
  }
  renderAll();
  if (compilingState.has(project.name)) {
    startRingLoop(project.name);
  }
}

// ─── Project actions ──────────────────────────────────────

async function runProject(name) {
  if (compilingState.has(name)) return;
  compilingState.set(name, { artifacts: 0, startTime: Date.now(), animId: null, error: null });
  renderAll();
  startRingLoop(name);
  try {
    await invoke('compile_and_run_cmd', { name });
  } catch (e) {
    const state = compilingState.get(name);
    if (state) state.error = String(e);
    renderAll();
  }
}

async function stopCompile(name) {
  try {
    await invoke('stop_compile_cmd', { name });
  } catch (e) {
    showError(String(e));
  }
}

async function stopProject(name) {
  try {
    await invoke('stop_project_cmd', { name });
    await pollRunning();
  } catch (e) {
    showError(String(e));
  }
}

async function openEditor(name) {
  try {
    await invoke('open_project_cmd', { name });
  } catch (e) {
    showError(String(e));
  }
}

// ─── Git actions ──────────────────────────────────────────

async function gitInit(name) {
  try {
    const status = await invoke('git_init_cmd', { name });
    gitStatusCache.set(name, status);
    renderContent();
  } catch (e) {
    showError(String(e));
  }
}

async function gitSync(name) {
  const btn = document.getElementById('git-sync');
  if (btn) { btn.disabled = true; btn.textContent = 'Syncing…'; }
  try {
    const status = await invoke('git_sync_cmd', { name, message: 'Update' });
    gitStatusCache.set(name, status);
    renderContent();
  } catch (e) {
    showError(String(e));
    refreshGitStatus(name);
  }
}

async function gitPull(name) {
  const btn = document.getElementById('git-pull');
  if (btn) { btn.disabled = true; btn.textContent = 'Pulling…'; }
  try {
    const status = await invoke('git_pull_cmd', { name });
    gitStatusCache.set(name, status);
    renderContent();
  } catch (e) {
    showError(String(e));
    refreshGitStatus(name);
  }
}

// ─── New Project modal ────────────────────────────────────

function showNewModal() {
  document.getElementById('new-project-name').value = '';
  setError('new-project-error', null);
  document.getElementById('modal-new').classList.remove('hidden');
  setTimeout(() => document.getElementById('new-project-name').focus(), 30);
}

function hideNewModal() {
  document.getElementById('modal-new').classList.add('hidden');
}

async function createProject() {
  const name = document.getElementById('new-project-name').value.trim();
  if (!name) return;

  const btn = document.getElementById('create-btn');
  btn.disabled = true;
  setError('new-project-error', null);

  try {
    projects = await invoke('create_project_cmd', { name });
    selected = projects.find(p => p.name === name) ?? null;
    if (selected) refreshGitStatus(selected.name);
    hideNewModal();
    renderAll();
  } catch (e) {
    setError('new-project-error', String(e));
  } finally {
    btn.disabled = false;
  }
}

// ─── Clone modal ──────────────────────────────────────────

function showCloneModal() {
  document.getElementById('clone-url').value = '';
  document.getElementById('clone-name').value = '';
  setError('clone-error', null);
  document.getElementById('modal-clone').classList.remove('hidden');
  setTimeout(() => document.getElementById('clone-url').focus(), 30);
}

function hideCloneModal() {
  document.getElementById('modal-clone').classList.add('hidden');
}

function nameFromGitUrl(url) {
  const cleaned = url.trim().replace(/\/+$/, '').replace(/\.git$/, '');
  const idx = Math.max(cleaned.lastIndexOf('/'), cleaned.lastIndexOf(':'));
  return idx >= 0 ? cleaned.slice(idx + 1) : cleaned;
}

async function cloneProject() {
  const url = document.getElementById('clone-url').value.trim();
  if (!url) return;
  const explicitName = document.getElementById('clone-name').value.trim();
  const name = explicitName || nameFromGitUrl(url);
  if (!name) {
    setError('clone-error', 'Could not determine a folder name from the URL');
    return;
  }
  const btn = document.getElementById('confirm-clone-btn');
  btn.disabled = true;
  btn.textContent = 'Cloning…';
  setError('clone-error', null);
  try {
    projects = await invoke('clone_project_cmd', { url, name });
    selected = projects.find(p => p.name === name) ?? null;
    if (selected) refreshGitStatus(selected.name);
    hideCloneModal();
    renderAll();
  } catch (e) {
    setError('clone-error', String(e));
  } finally {
    btn.disabled = false;
    btn.textContent = 'Clone';
  }
}

// ─── Copy modal ───────────────────────────────────────────

function showCopyModal(name) {
  pendingCopyName = name;
  document.getElementById('copy-name').value = `${name}-copy`;
  setError('copy-error', null);
  document.getElementById('modal-copy').classList.remove('hidden');
  setTimeout(() => {
    const input = document.getElementById('copy-name');
    input.focus();
    input.select();
  }, 30);
}

function hideCopyModal() {
  document.getElementById('modal-copy').classList.add('hidden');
  pendingCopyName = null;
}

async function confirmCopy() {
  if (!pendingCopyName) return;
  const newName = document.getElementById('copy-name').value.trim();
  if (!newName) return;
  const btn = document.getElementById('confirm-copy-btn');
  btn.disabled = true;
  btn.textContent = 'Copying…';
  setError('copy-error', null);
  try {
    projects = await invoke('copy_project_cmd', { name: pendingCopyName, newName });
    selected = projects.find(p => p.name === newName) ?? selected;
    if (selected) refreshGitStatus(selected.name);
    hideCopyModal();
    renderAll();
  } catch (e) {
    setError('copy-error', String(e));
  } finally {
    btn.disabled = false;
    btn.textContent = 'Copy';
  }
}

// ─── Delete modal ─────────────────────────────────────────

function showDeleteModal(name) {
  pendingDeleteName = name;
  document.getElementById('delete-text').textContent = `Delete project "${name}"?`;
  setError('delete-error', null);
  document.getElementById('modal-delete').classList.remove('hidden');
}

function hideDeleteModal() {
  document.getElementById('modal-delete').classList.add('hidden');
  pendingDeleteName = null;
}

async function confirmDelete() {
  if (!pendingDeleteName) return;
  const btn = document.getElementById('confirm-delete-btn');
  btn.disabled = true;
  btn.textContent = 'Deleting…';
  setError('delete-error', null);
  const name = pendingDeleteName;
  try {
    projects = await invoke('delete_project_cmd', { name });
    if (selected?.name === name) selected = null;
    gitStatusCache.delete(name);
    compilingState.delete(name);
    running.delete(name);
    hideDeleteModal();
    renderAll();
  } catch (e) {
    setError('delete-error', String(e));
  } finally {
    btn.disabled = false;
    btn.textContent = 'Delete';
  }
}

// ─── Git remote modal ─────────────────────────────────────

function showRemoteModal(name, current) {
  pendingRemoteName = name;
  document.getElementById('remote-url').value = current ?? '';
  setError('remote-error', null);
  document.getElementById('modal-remote').classList.remove('hidden');
  setTimeout(() => document.getElementById('remote-url').focus(), 30);
}

function hideRemoteModal() {
  document.getElementById('modal-remote').classList.add('hidden');
  pendingRemoteName = null;
}

async function confirmRemote() {
  if (!pendingRemoteName) return;
  const url = document.getElementById('remote-url').value.trim();
  if (!url) {
    setError('remote-error', 'URL required');
    return;
  }
  const btn = document.getElementById('confirm-remote-btn');
  btn.disabled = true;
  setError('remote-error', null);
  try {
    const status = await invoke('git_set_remote_cmd', { name: pendingRemoteName, url });
    gitStatusCache.set(pendingRemoteName, status);
    hideRemoteModal();
    renderContent();
  } catch (e) {
    setError('remote-error', String(e));
  } finally {
    btn.disabled = false;
  }
}

// ─── Settings modal ───────────────────────────────────────

async function showSettingsModal() {
  try {
    const config = await invoke('get_config_cmd');
    document.getElementById('settings-projects-dir').value = config.projects_dir;
    document.getElementById('settings-editor').value = config.editor_cmd;
    document.getElementById('modal-settings').classList.remove('hidden');
    setTimeout(() => document.getElementById('settings-projects-dir').focus(), 30);
  } catch (e) {
    showError(String(e));
  }
}

function hideSettingsModal() {
  document.getElementById('modal-settings').classList.add('hidden');
}

async function saveSettings() {
  const config = {
    projects_dir: document.getElementById('settings-projects-dir').value.trim(),
    editor_cmd: document.getElementById('settings-editor').value.trim(),
  };

  const btn = document.getElementById('save-settings-btn');
  btn.disabled = true;

  try {
    projects = await invoke('save_config_cmd', { config });
    selected = null;
    gitStatusCache.clear();
    hideSettingsModal();
    renderAll();
  } catch (e) {
    showError(String(e));
  } finally {
    btn.disabled = false;
  }
}

// ─── Utilities ────────────────────────────────────────────

function escHtml(str) {
  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function setError(id, msg) {
  const el = document.getElementById(id);
  if (!el) return;
  if (msg) {
    el.textContent = msg;
    el.classList.remove('hidden');
  } else {
    el.textContent = '';
    el.classList.add('hidden');
  }
}

function showError(msg) {
  alert(msg);
}

// ─── Event wiring ─────────────────────────────────────────

document.getElementById('btn-new').addEventListener('click', showNewModal);
document.getElementById('btn-clone').addEventListener('click', showCloneModal);
document.getElementById('btn-settings').addEventListener('click', showSettingsModal);

document.getElementById('create-btn').addEventListener('click', createProject);
document.getElementById('cancel-create-btn').addEventListener('click', hideNewModal);

document.getElementById('confirm-clone-btn').addEventListener('click', cloneProject);
document.getElementById('cancel-clone-btn').addEventListener('click', hideCloneModal);

document.getElementById('confirm-copy-btn').addEventListener('click', confirmCopy);
document.getElementById('cancel-copy-btn').addEventListener('click', hideCopyModal);

document.getElementById('confirm-delete-btn').addEventListener('click', confirmDelete);
document.getElementById('cancel-delete-btn').addEventListener('click', hideDeleteModal);

document.getElementById('confirm-remote-btn').addEventListener('click', confirmRemote);
document.getElementById('cancel-remote-btn').addEventListener('click', hideRemoteModal);

document.getElementById('save-settings-btn').addEventListener('click', saveSettings);
document.getElementById('cancel-settings-btn').addEventListener('click', hideSettingsModal);

document.getElementById('new-project-name').addEventListener('keydown', e => {
  if (e.key === 'Enter') createProject();
  if (e.key === 'Escape') hideNewModal();
});

document.getElementById('clone-url').addEventListener('keydown', e => {
  if (e.key === 'Enter') cloneProject();
  if (e.key === 'Escape') hideCloneModal();
});
document.getElementById('clone-name').addEventListener('keydown', e => {
  if (e.key === 'Enter') cloneProject();
  if (e.key === 'Escape') hideCloneModal();
});

document.getElementById('copy-name').addEventListener('keydown', e => {
  if (e.key === 'Enter') confirmCopy();
  if (e.key === 'Escape') hideCopyModal();
});

document.getElementById('remote-url').addEventListener('keydown', e => {
  if (e.key === 'Enter') confirmRemote();
  if (e.key === 'Escape') hideRemoteModal();
});

document.getElementById('settings-editor').addEventListener('keydown', e => {
  if (e.key === 'Enter') saveSettings();
  if (e.key === 'Escape') hideSettingsModal();
});

for (const id of ['modal-new', 'modal-clone', 'modal-copy', 'modal-delete', 'modal-remote', 'modal-settings']) {
  const el = document.getElementById(id);
  el.addEventListener('click', e => {
    if (e.target === e.currentTarget) el.classList.add('hidden');
  });
}

document.addEventListener('keydown', e => {
  if (e.key !== 'Escape') return;
  for (const id of ['modal-new', 'modal-clone', 'modal-copy', 'modal-delete', 'modal-remote', 'modal-settings']) {
    document.getElementById(id).classList.add('hidden');
  }
});

// ─── Boot ────────────────────────────────────────────────

mountIcon('btn-clone', 'download');
mountIcon('btn-new', 'plus');
mountIcon('btn-settings', 'settings');
// Initial empty-state icon (replaced once a project is selected)
const initialEmptyIcon = document.querySelector('.empty-icon');
if (initialEmptyIcon) initialEmptyIcon.innerHTML = icon('hexagon', { size: 48 });

loadProjects();
setInterval(pollRunning, 1000);
