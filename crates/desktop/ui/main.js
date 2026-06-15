// Tauri injects window.__TAURI__ when withGlobalTauri is true
const invoke = window.__TAURI__?.core?.invoke ?? (() => Promise.reject('Tauri not available'));

// ─── State ───────────────────────────────────────────────

let projects = [];
let running = new Set();
let selected = null; // ProjectInfo | null

// ─── Data loading ─────────────────────────────────────────

async function loadProjects() {
  try {
    projects = await invoke('list_projects');
    running = new Set(await invoke('get_running'));
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
    if (selected) renderContent();
  } catch (e) {
    // silently ignore poll failures
  }
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
    const isSelected = selected?.name === project.name;

    const item = document.createElement('div');
    item.className = 'project-item' + (isSelected ? ' selected' : '');
    item.innerHTML = `
      <span class="project-item-name">${escHtml(project.name)}</span>
      ${isRunning ? '<span class="running-dot" title="Running"></span>' : ''}
    `;
    item.addEventListener('click', () => selectProject(project));
    list.appendChild(item);
  }
}

function renderContent() {
  const content = document.getElementById('content');

  if (!selected) {
    content.innerHTML = `
      <div class="empty-state">
        <div class="empty-icon">⬡</div>
        <p>Select a project or create a new one</p>
        <button class="btn btn-primary" id="btn-new-empty">New Project</button>
      </div>
    `;
    document.getElementById('btn-new-empty').addEventListener('click', showNewModal);
    return;
  }

  const isRunning = running.has(selected.name);

  content.innerHTML = `
    <div class="project-detail">
      <div class="project-detail-name">${escHtml(selected.name)}</div>
      <div class="project-detail-path">${escHtml(selected.path)}</div>

      <div class="project-actions">
        ${isRunning
          ? `<button class="btn btn-danger" id="btn-stop">⏹ Stop</button>
             <span class="status-running">Running</span>`
          : `<button class="btn btn-primary" id="btn-run">▶ Run</button>`
        }
        <button class="btn btn-ghost" id="btn-editor">Open in Editor</button>
      </div>
    </div>
  `;

  if (isRunning) {
    document.getElementById('btn-stop').addEventListener('click', () => stopProject(selected.name));
  } else {
    document.getElementById('btn-run').addEventListener('click', () => runProject(selected.name));
  }
  document.getElementById('btn-editor').addEventListener('click', () => openEditor(selected.name));
}

function selectProject(project) {
  selected = project;
  renderAll();
}

// ─── Project actions ──────────────────────────────────────

async function runProject(name) {
  try {
    await invoke('run_project_cmd', { name });
    await pollRunning();
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

// ─── New Project modal ────────────────────────────────────

function showNewModal() {
  document.getElementById('new-project-name').value = '';
  setNewProjectError(null);
  document.getElementById('modal-new').classList.remove('hidden');
  setTimeout(() => document.getElementById('new-project-name').focus(), 30);
}

function hideNewModal() {
  document.getElementById('modal-new').classList.add('hidden');
}

function setNewProjectError(msg) {
  const el = document.getElementById('new-project-error');
  if (msg) {
    el.textContent = msg;
    el.classList.remove('hidden');
  } else {
    el.textContent = '';
    el.classList.add('hidden');
  }
}

async function createProject() {
  const name = document.getElementById('new-project-name').value.trim();
  if (!name) return;

  const btn = document.getElementById('create-btn');
  btn.disabled = true;
  setNewProjectError(null);

  try {
    projects = await invoke('create_project_cmd', { name });
    // Select the newly created project
    selected = projects.find(p => p.name === name) ?? null;
    hideNewModal();
    renderAll();
  } catch (e) {
    setNewProjectError(String(e));
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
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function showError(msg) {
  // Simple inline alert — could be improved with a toast
  alert(msg);
}

// ─── Event wiring ─────────────────────────────────────────

document.getElementById('btn-new').addEventListener('click', showNewModal);
document.getElementById('btn-settings').addEventListener('click', showSettingsModal);

document.getElementById('create-btn').addEventListener('click', createProject);
document.getElementById('cancel-create-btn').addEventListener('click', hideNewModal);

document.getElementById('save-settings-btn').addEventListener('click', saveSettings);
document.getElementById('cancel-settings-btn').addEventListener('click', hideSettingsModal);

document.getElementById('new-project-name').addEventListener('keydown', e => {
  if (e.key === 'Enter') createProject();
  if (e.key === 'Escape') hideNewModal();
});

document.getElementById('settings-editor').addEventListener('keydown', e => {
  if (e.key === 'Enter') saveSettings();
  if (e.key === 'Escape') hideSettingsModal();
});

// Close modals on overlay click
document.getElementById('modal-new').addEventListener('click', e => {
  if (e.target === e.currentTarget) hideNewModal();
});
document.getElementById('modal-settings').addEventListener('click', e => {
  if (e.target === e.currentTarget) hideSettingsModal();
});

// ─── Boot ────────────────────────────────────────────────

loadProjects();
setInterval(pollRunning, 1000);
