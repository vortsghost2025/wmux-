// wmux frontend application — wires UI to Tauri backend
// Runs in WebView2 via window.__TAURI__

// ===== Tauri bridge =====
const TAURI = window.__TAURI__;
const isTauri = !!TAURI;

async function invoke(cmd, args = {}) {
  if (isTauri) {
    return await TAURI.core.invoke(cmd, args);
  }
  // Browser dev fallback — return mock data
  return mockInvoke(cmd, args);
}

// ===== State =====
const state = {
  workspaces: [],
  activeWorkspaceId: null,
  panes: {},              // workspaceId -> [{id, ptyId, agentName, agentStatus}]
  focusedPaneId: null,
  notifications: [],
  toastQueue: [],
  commandPaletteOpen: false,
  sidebarVisible: true,
};

// ===== Workspace Management =====

async function loadWorkspaces() {
  try {
    state.workspaces = await invoke('list_workspaces');
  } catch (e) {
    console.error('Failed to load workspaces:', e);
  }
  renderSidebar();
  renderStatusBar();
}

async function createWorkspace(name, directory) {
  try {
    const ws = await invoke('create_workspace', { name, directory });
    state.workspaces.push(ws);
    renderSidebar();
    switchToWorkspace(ws.id);
    showToast('Workspace created', name, 'normal');
  } catch (e) {
    console.error('create_workspace failed:', e);
  }
}

async function switchToWorkspace(workspaceId) {
  try {
    await invoke('switch_workspace', { workspaceId });
    state.activeWorkspaceId = workspaceId;
    // Mark read
    await invoke('mark_read', { workspaceId });
    // Refresh workspace list to get updated unread counts
    await loadWorkspaces();
    renderPanes();
  } catch (e) {
    console.error('switch_workspace failed:', e);
  }
}

async function renameWorkspace(workspaceId) {
  const ws = state.workspaces.find(w => w.id === workspaceId);
  if (!ws) return;
  const newName = prompt('Rename workspace:', ws.name);
  if (newName && newName !== ws.name) {
    await invoke('rename_workspace', { workspaceId, newName });
    await loadWorkspaces();
  }
}

async function removeWorkspace(workspaceId) {
  if (!confirm('Close this workspace?')) return;
  await invoke('remove_workspace', { workspaceId });
  state.workspaces = state.workspaces.filter(w => w.id !== workspaceId);
  if (state.activeWorkspaceId === workspaceId) {
    state.activeWorkspaceId = state.workspaces[0]?.id || null;
  }
  renderSidebar();
}

// ===== Notification System =====

function showToast(title, body, urgency, agentClass) {
  const container = document.getElementById('toast-container');
  if (!container) return;

  const toast = document.createElement('div');
  toast.className = 'notification-toast';
  toast.innerHTML = `
    <div class="toast-header">
      ${agentClass ? `<span class="agent-badge ${agentClass}" style="font-size:11px">${title.split(' ')[0]}</span>` : ''}
      <span class="toast-title">${title}</span>
      <button class="toast-close" onclick="this.parentElement.parentElement.remove()">✕</button>
    </div>
    <div class="toast-body">${body}</div>
    <div class="toast-meta">${new Date().toLocaleTimeString()} │ <span class="kbd">Ctrl</span>+<span class="kbd">Shift</span>+<span class="kbd">U</span> jump</div>
  `;
  container.appendChild(toast);

  // Auto-dismiss after 6s
  setTimeout(() => {
    toast.style.transition = 'all 0.3s ease-in';
    toast.style.opacity = '0';
    toast.style.transform = 'translateX(120px)';
    setTimeout(() => toast.remove(), 300);
  }, 6000);
}

async function jumpToUnread() {
  // Find workspace with highest unread count
  const unread = state.workspaces
    .filter(w => w.unread_count > 0)
    .sort((a, b) => b.unread_count - a.unread_count);
  if (unread.length > 0) {
    await switchToWorkspace(unread[0].id);
    showToast('Jumped to', unread[0].name, 'normal');
  }
}

// ===== Command Palette =====

function toggleCommandPalette() {
  state.commandPaletteOpen = !state.commandPaletteOpen;
  const palette = document.getElementById('command-palette');
  if (!palette) return;

  if (state.commandPaletteOpen) {
    palette.style.display = 'flex';
    const input = palette.querySelector('.palette-input');
    if (input) {
      input.value = '';
      input.focus();
      renderPaletteResults('');
    }
  } else {
    palette.style.display = 'none';
  }
}

const COMMANDS = [
  { id: 'ws-new', label: 'New Workspace', shortcut: 'Ctrl+N', action: () => promptNewWorkspace() },
  { id: 'ws-rename', label: 'Rename Workspace', shortcut: 'Ctrl+Shift+R', action: () => renameWorkspace(state.activeWorkspaceId) },
  { id: 'ws-close', label: 'Close Workspace', shortcut: 'Ctrl+Shift+W', action: () => removeWorkspace(state.activeWorkspaceId) },
  { id: 'split-right', label: 'Split Right', shortcut: 'Ctrl+D', action: () => splitPane('right') },
  { id: 'split-down', label: 'Split Down', shortcut: 'Ctrl+Shift+D', action: () => splitPane('down') },
  { id: 'jump-unread', label: 'Jump to Unread', shortcut: 'Ctrl+Shift+U', action: () => jumpToUnread() },
  { id: 'save-session', label: 'Save Session', shortcut: '', action: () => invoke('save_session').then(r => showToast('Session', r, 'normal')) },
  { id: 'toggle-sidebar', label: 'Toggle Sidebar', shortcut: 'Ctrl+B', action: () => toggleSidebar() },
  { id: 'wave-status', label: 'Wave Bridge Status', shortcut: '', action: () => invoke('wave_bridge_status').then(s => showToast('Wave Bridge', s.mode, 'normal')) },
  { id: 'wave-ask', label: 'Ask All Terminals', shortcut: '', action: () => waveAskAll() },
];

function renderPaletteResults(query) {
  const list = document.getElementById('palette-results');
  if (!list) return;
  const q = query.toLowerCase();
  const filtered = q ? COMMANDS.filter(c => c.label.toLowerCase().includes(q)) : COMMANDS;
  list.innerHTML = filtered.map((c, i) => `
    <div class="palette-item${i === 0 ? ' selected' : ''}" data-idx="${i}" onclick="executePaletteCommand('${c.id}')">
      <span>${c.label}</span>
      ${c.shortcut ? `<span class="palette-shortcut">${c.shortcut}</span>` : ''}
    </div>
  `).join('');
}

function executePaletteCommand(id) {
  const cmd = COMMANDS.find(c => c.id === id);
  if (cmd) {
    toggleCommandPalette();
    cmd.action();
  }
}

function promptNewWorkspace() {
  const name = prompt('Workspace name:');
  if (!name) return;
  const dir = prompt('Directory path:', 'C:\\repos\\' + name);
  if (dir) createWorkspace(name, dir);
}

function splitPane(direction) {
  showToast('Split', `Split ${direction} (Phase 2 — needs xterm.js)`, 'normal');
}

function toggleSidebar() {
  state.sidebarVisible = !state.sidebarVisible;
  const sidebar = document.querySelector('.sidebar');
  if (sidebar) sidebar.style.display = state.sidebarVisible ? 'flex' : 'none';
}

async function waveAskAll() {
  const query = prompt('Ask all terminals:');
  if (!query) return;
  try {
    const results = await invoke('wave_ask_all', { query, lines: 50 });
    showToast('Wave Ask', `${results.length} terminals responded`, 'normal');
    console.log('Wave ask results:', results);
  } catch (e) {
    console.error('wave_ask_all failed:', e);
  }
}

// ===== Keyboard Shortcuts =====

document.addEventListener('keydown', (e) => {
  // Ctrl+Shift+P — Command palette
  if (e.ctrlKey && e.shiftKey && e.key === 'P') {
    e.preventDefault();
    toggleCommandPalette();
    return;
  }
  // Escape — close palette
  if (e.key === 'Escape' && state.commandPaletteOpen) {
    toggleCommandPalette();
    return;
  }
  // Ctrl+Shift+U — Jump to unread
  if (e.ctrlKey && e.shiftKey && e.key === 'U') {
    e.preventDefault();
    jumpToUnread();
    return;
  }
  // Ctrl+B — Toggle sidebar
  if (e.ctrlKey && !e.shiftKey && e.key === 'b') {
    e.preventDefault();
    toggleSidebar();
    return;
  }
  // Ctrl+N — New workspace
  if (e.ctrlKey && !e.shiftKey && e.key === 'n') {
    e.preventDefault();
    promptNewWorkspace();
    return;
  }
  // Ctrl+1-9 — Switch workspace
  if (e.ctrlKey && !e.shiftKey && e.key >= '1' && e.key <= '9') {
    e.preventDefault();
    const idx = parseInt(e.key) - 1;
    if (idx < state.workspaces.length) {
      switchToWorkspace(state.workspaces[idx].id);
    }
    return;
  }
  // Ctrl+Shift+W — Close workspace
  if (e.ctrlKey && e.shiftKey && e.key === 'W') {
    e.preventDefault();
    if (state.activeWorkspaceId) removeWorkspace(state.activeWorkspaceId);
    return;
  }
  // Ctrl+D — Split right
  if (e.ctrlKey && !e.shiftKey && e.key === 'd') {
    e.preventDefault();
    splitPane('right');
    return;
  }
  // Ctrl+Shift+D — Split down
  if (e.ctrlKey && e.shiftKey && e.key === 'D') {
    e.preventDefault();
    splitPane('down');
    return;
  }
  // Palette navigation
  if (state.commandPaletteOpen) {
    if (e.key === 'Enter') {
      const selected = document.querySelector('.palette-item.selected');
      if (selected) selected.click();
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const items = document.querySelectorAll('.palette-item');
      let idx = Array.from(items).findIndex(i => i.classList.contains('selected'));
      items[idx]?.classList.remove('selected');
      idx = e.key === 'ArrowDown' ? Math.min(idx + 1, items.length - 1) : Math.max(idx - 1, 0);
      items[idx]?.classList.add('selected');
      items[idx]?.scrollIntoView({ block: 'nearest' });
    }
  }
});

// ===== Render Functions =====

function renderSidebar() {
  const list = document.getElementById('workspace-list');
  if (!list) return;

  list.innerHTML = state.workspaces.map(ws => {
    const isActive = ws.id === state.activeWorkspaceId || ws.is_active;
    const hasWaiting = ws.agents?.some(a => a.status === 'waiting');
    const hasError = ws.agents?.some(a => a.status === 'error');
    const indicatorClass = hasWaiting ? 'waiting' : hasError ? 'error' : (ws.agents?.length > 0 ? 'active' : 'idle');
    const classes = ['workspace-item'];
    if (isActive) classes.push('active');
    if (hasWaiting) classes.push('attention');

    const agentLine = ws.agents?.map(a => `${a.name} ${a.status}`).join(' • ') || '';

    return `
      <div class="${classes.join(' ')}" onclick="switchToWorkspace('${ws.id}')" oncontextmenu="event.preventDefault(); workspaceContextMenu('${ws.id}', event)">
        <div class="ws-header">
          <div class="ws-indicator ${indicatorClass}"></div>
          <div class="ws-name">${ws.name}</div>
          ${ws.unread_count > 0 ? `<div class="ws-badge unread">${ws.unread_count}</div>` : ''}
        </div>
        <div class="ws-meta">
          ${ws.git_branch ? `
            <div class="ws-meta-row">
              <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12"><path d="M11.75 2.5a.75.75 0 0 1 .75.75v10.5a.75.75 0 0 1-1.5 0V3.25a.75.75 0 0 1 .75-.75Zm-8.5 0a.75.75 0 0 1 .75.75v5.5a.75.75 0 0 1-1.5 0v-5.5a.75.75 0 0 1 .75-.75Zm4.25 0a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5a.75.75 0 0 1 .75-.75Z"/></svg>
              <span class="branch">${ws.git_branch}</span>
            </div>
          ` : ''}
          ${agentLine ? `
            <div class="ws-meta-row">
              <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12"><path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1ZM0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Z"/><path d="M8 3.5a.5.5 0 0 1 .5.5v3.793l2.354 2.354a.5.5 0 0 1-.708.708l-2.5-2.5A.5.5 0 0 1 7.5 8V4a.5.5 0 0 1 .5-.5Z"/></svg>
              <span class="agent">${agentLine}</span>
            </div>
          ` : ''}
          <div class="ws-meta-row">
            <svg viewBox="0 0 16 16" fill="currentColor" width="12" height="12"><path d="M1.75 1h12.5c.966 0 1.75.784 1.75 1.75v10.5A1.75 1.75 0 0 1 14.25 15H1.75A1.75 1.75 0 0 1 0 13.25V2.75C0 1.784.784 1 1.75 1Zm12.5 1.5H1.75a.25.25 0 0 0-.25.25v10.5c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25V2.75a.25.25 0 0 0-.25-.25Z"/></svg>
            <span style="color:var(--text-muted); font-family:monospace; font-size:10px;">${ws.directory}</span>
          </div>
        </div>
        ${ws.last_notification ? `<div class="ws-notification">⏳ ${ws.last_notification}</div>` : ''}
      </div>
    `;
  }).join('');

  // Update footer counts
  const running = state.workspaces.reduce((n, ws) => n + (ws.agents?.filter(a => a.status === 'running').length || 0), 0);
  const waiting = state.workspaces.reduce((n, ws) => n + (ws.agents?.filter(a => a.status === 'waiting').length || 0), 0);
  const footer = document.getElementById('sidebar-footer-stats');
  if (footer) {
    footer.innerHTML = `
      <div class="stat"><div class="dot running"></div> ${running} running</div>
      <div class="stat"><div class="dot waiting"></div> ${waiting} waiting</div>
    `;
  }
}

function renderPanes() {
  // Phase 2: this will render actual xterm.js terminals
  // For now, update the pane headers with active workspace info
  const ws = state.workspaces.find(w => w.id === state.activeWorkspaceId);
  if (!ws) return;
  const activeLabel = document.getElementById('status-active-workspace');
  if (activeLabel) activeLabel.textContent = `${ws.name}${ws.git_branch ? '/' + ws.git_branch : ''}`;
}

function renderStatusBar() {
  const running = state.workspaces.reduce((n, ws) => n + (ws.agents?.filter(a => a.status === 'running').length || 0), 0);
  const waiting = state.workspaces.reduce((n, ws) => n + (ws.agents?.filter(a => a.status === 'waiting').length || 0), 0);
  const totalAgents = state.workspaces.reduce((n, ws) => n + (ws.agents?.length || 0), 0);

  const left = document.getElementById('status-left');
  if (left) {
    left.innerHTML = `
      <div class="status-item"><div class="dot" style="background:var(--green)"></div><span>${totalAgents} agents</span></div>
      ${waiting > 0 ? `<div class="status-item"><div class="dot" style="background:var(--accent)"></div><span>${waiting} waiting</span></div>` : ''}
      <span>│</span>
      <span id="status-active-workspace">${state.workspaces.find(w => w.is_active)?.name || 'no workspace'}</span>
    `;
  }
}

function workspaceContextMenu(wsId, event) {
  // Simple context menu via prompt for now
  const action = prompt('Action: rename / close / cancel');
  if (action === 'rename') renameWorkspace(wsId);
  else if (action === 'close') removeWorkspace(wsId);
}

// ===== Mock Tauri for browser development =====

const mockWorkspaces = [
  {
    id: 'ws-1', name: 'kucoin-lane', directory: 'C:\\repos\\kucoin-lane',
    git_branch: 'integration/cherry-pick-fixes', git_pr: { number: 847, status: 'open', checks: '3 passing' },
    agents: [
      { name: 'GLM-5.1', status: 'waiting', pane_id: 'p1', elapsed_secs: null },
      { name: 'Kilo', status: 'running', pane_id: 'p2', elapsed_secs: 259 },
    ],
    listening_ports: [8080, 3000], is_active: true, unread_count: 2,
    last_notification: 'GLM-5.1: Waiting for input — test results ready',
    created_at: '2026-06-06T10:00:00Z',
  },
  {
    id: 'ws-2', name: 'kernel-lane', directory: 'C:\\repos\\kernel-lane',
    git_branch: 'main', git_pr: null,
    agents: [{ name: 'Kilo Auto Free', status: 'running', pane_id: 'p3', elapsed_secs: 259 }],
    listening_ports: [], is_active: false, unread_count: 0, last_notification: null,
    created_at: '2026-06-06T10:01:00Z',
  },
  {
    id: 'ws-3', name: 'Archivist-Agent', directory: 'C:\\repos\\Archivist-Agent',
    git_branch: 'master', git_pr: null,
    agents: [{ name: 'housekeeping', status: 'running', pane_id: 'p4', elapsed_secs: null }],
    listening_ports: [], is_active: false, unread_count: 0, last_notification: null,
    created_at: '2026-06-06T10:02:00Z',
  },
  {
    id: 'ws-4', name: 'SwarmMind', directory: 'C:\\repos\\SwarmMind',
    git_branch: 'main', git_pr: null, agents: [],
    listening_ports: [], is_active: false, unread_count: 0, last_notification: null,
    created_at: '2026-06-06T10:03:00Z',
  },
  {
    id: 'ws-5', name: 'federation', directory: 'C:\\repos\\federation',
    git_branch: 'dev', git_pr: null,
    agents: [{ name: 'tests', status: 'error', pane_id: null, elapsed_secs: null }],
    listening_ports: [], is_active: false, unread_count: 0, last_notification: null,
    created_at: '2026-06-06T10:04:00Z',
  },
  {
    id: 'ws-6', name: 'Wave AI Bridge', directory: 'S:\\waveterm',
    git_branch: null, git_pr: null,
    agents: [{ name: 'Wave', status: 'running', pane_id: null, elapsed_secs: null }],
    listening_ports: [], is_active: false, unread_count: 0, last_notification: null,
    created_at: '2026-06-06T10:05:00Z',
  },
];

function mockInvoke(cmd, args) {
  switch (cmd) {
    case 'ping': return 'pong';
    case 'list_workspaces': return [...mockWorkspaces];
    case 'create_workspace': {
      const ws = {
        id: 'ws-' + Date.now(), name: args.name, directory: args.directory,
        git_branch: null, git_pr: null, agents: [], listening_ports: [],
        is_active: false, unread_count: 0, last_notification: null,
        created_at: new Date().toISOString(),
      };
      mockWorkspaces.push(ws);
      return ws;
    }
    case 'switch_workspace':
      mockWorkspaces.forEach(w => w.is_active = w.id === args.workspaceId);
      return null;
    case 'rename_workspace':
      { const w = mockWorkspaces.find(w => w.id === args.workspaceId); if (w) w.name = args.newName; }
      return null;
    case 'remove_workspace':
      { const idx = mockWorkspaces.findIndex(w => w.id === args.workspaceId); if (idx >= 0) mockWorkspaces.splice(idx, 1); }
      return null;
    case 'mark_read':
      { const w = mockWorkspaces.find(w => w.id === args.workspaceId); if (w) w.unread_count = 0; }
      return null;
    case 'send_notification': return { id: 'n-' + Date.now(), ...args, read: false };
    case 'get_all_notifications': return [];
    case 'save_session': return `Session saved (${mockWorkspaces.length} workspaces)`;
    case 'load_session': return null;
    case 'wave_bridge_status': return { mode: 'disabled', bridge_dir: null, api_port: null, connected: false };
    case 'wave_ask_all': return mockWorkspaces.map(w => ({ workspace: w.name, agent: w.agents[0]?.name || null, status: w.agents[0]?.status || 'idle', last_lines: [] }));
    case 'report_agent_status': return null;
    default: console.warn('Unknown mock command:', cmd); return null;
  }
}

// ===== Init =====

window.addEventListener('DOMContentLoaded', async () => {
  console.log('[wmux] Initializing...', isTauri ? 'Tauri mode' : 'Browser dev mode');

  // Ping backend
  try {
    const pong = await invoke('ping');
    console.log('[wmux] Backend:', pong);
  } catch (e) {
    console.warn('[wmux] Backend not available, using mocks');
  }

  // Load workspaces
  await loadWorkspaces();

  // Set first workspace active if none is
  if (!state.activeWorkspaceId && state.workspaces.length > 0) {
    const active = state.workspaces.find(w => w.is_active) || state.workspaces[0];
    state.activeWorkspaceId = active.id;
  }

  renderSidebar();
  renderPanes();
  renderStatusBar();

  // Show initial toast in browser dev mode
  if (!isTauri) {
    setTimeout(() => {
      showToast('GLM-5.1 waiting', 'kucoin-lane: test results ready — DEX module findings need direction', 'high', 'glm');
    }, 1500);
  }

  console.log('[wmux] Ready. Workspaces:', state.workspaces.length);
});

// Export for inline onclick handlers
window.switchToWorkspace = switchToWorkspace;
window.workspaceContextMenu = workspaceContextMenu;
window.executePaletteCommand = executePaletteCommand;
window.toggleCommandPalette = toggleCommandPalette;
window.promptNewWorkspace = promptNewWorkspace;
