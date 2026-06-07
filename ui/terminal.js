// terminal.js — xterm.js terminal manager
// Simplified: starts with ONE full terminal, split with Ctrl+D
// Bridges to Tauri PTY backend via:
// Events (Rust → JS): pty_data { ptyId, data } | pty_exit { ptyId, code }
// Commands (JS → Rust): create_pty, write_pty, resize_pty, close_pty

;(function() {
const TAURI = window.__TAURI__;
const isTauri = !!TAURI;
const XTerm = window.Terminal;
const FitAddonClass = window.FitAddon?.FitAddon;
const WebglAddonClass = window.WebglAddon?.WebglAddon;
const WebLinksAddonClass = window.WebLinksAddon?.WebLinksAddon;

const terminals = {};
let paneCounter = 1;

const WMUX_THEME = {
  background: '#0d1117',
  foreground: '#e6edf3',
  cursor: '#e6edf3',
  cursorAccent: '#0d1117',
  selectionBackground: 'rgba(88,166,255,0.3)',
  black: '#484f58',
  red: '#f85149',
  green: '#3fb950',
  yellow: '#d29922',
  blue: '#58a6ff',
  magenta: '#bc8cff',
  cyan: '#39d353',
  white: '#e6edf3',
  brightBlack: '#6e7681',
  brightRed: '#ffa198',
  brightGreen: '#56d364',
  brightYellow: '#e3b341',
  brightBlue: '#79c0ff',
  brightMagenta: '#d2a8ff',
  brightCyan: '#56d364',
  brightWhite: '#ffffff',
};

// ===== Create a terminal in a container =====

async function createTerminal(containerId, cwd, workspaceId) {
  const container = document.getElementById(containerId);
  if (!container || !XTerm || !FitAddonClass) {
    console.error('[terminal] Missing container or xterm:', containerId);
    return null;
  }

  const term = new XTerm({
    theme: WMUX_THEME,
    fontFamily: "'Cascadia Code', 'Cascadia Mono', 'Consolas', monospace",
    fontSize: 14,
    lineHeight: 1.25,
    cursorBlink: true,
    cursorStyle: 'block',
    scrollback: 10000,
    allowProposedApi: true,
    convertEol: true,
  });

  const fitAddon = new FitAddonClass();
  term.loadAddon(fitAddon);

  if (WebLinksAddonClass) {
    try { term.loadAddon(new WebLinksAddonClass()); } catch(e) {}
  }

  term.open(container);

  if (WebglAddonClass) {
    try { term.loadAddon(new WebglAddonClass()); } catch(e) {
      console.warn('[terminal] WebGL failed, using canvas');
    }
  }

  // Fit after a brief delay to let layout settle
  setTimeout(() => fitAddon.fit(), 50);
  setTimeout(() => fitAddon.fit(), 200);

  let ptyId = null;

  if (isTauri) {
    try {
const info = await TAURI.core.invoke('create_pty', {
      cwd: cwd || 'C:\\Users\\seand',
      cols: term.cols,
      rows: term.rows,
      workspaceId: workspaceId || 'default',
    });
      ptyId = info.id;
      console.log(`[terminal] PTY ${ptyId} created (${term.cols}x${term.rows})`);
    } catch (e) {
      console.error('[terminal] create_pty failed:', e);
      term.writeln('\x1b[31mPTY failed: ' + e + '\x1b[0m');
    }
  } else {
    term.writeln('\x1b[38;2;88;166;255mwmux\x1b[0m — browser dev mode (no PTY)');
    term.writeln('Run \x1b[32mnpx tauri dev\x1b[0m for real terminals');
  }

  // Input → PTY
  term.onData((data) => {
    if (ptyId && isTauri) {
      TAURI.core.invoke('write_pty', { ptyId, data }).catch(() => {});
    }
  });

  // Auto-resize
  const ro = new ResizeObserver(() => {
    try {
      fitAddon.fit();
      if (ptyId && isTauri) {
        TAURI.core.invoke('resize_pty', {
          ptyId,
          cols: term.cols,
          rows: term.rows,
        }).catch(() => {});
      }
    } catch(e) {}
  });
  ro.observe(container);

  const entry = { term, fitAddon, ptyId, ro, containerId, cwd: cwd || null };
  terminals[containerId] = entry;
  return entry;
}

// ===== PTY data listener =====

function setupPtyListener() {
  if (!isTauri) return;

  TAURI.event.listen('pty_data', (event) => {
    const { pty_id, data } = event.payload;
    const entry = Object.values(terminals).find(t => t.ptyId === pty_id);
    if (entry) entry.term.write(data);
  });

  TAURI.event.listen('pty_exit', (event) => {
    const { pty_id, code } = event.payload;
    const entry = Object.values(terminals).find(t => t.ptyId === pty_id);
    if (entry) {
      entry.term.writeln(`\r\n\x1b[33m[exited: ${code}]\x1b[0m`);
      entry.ptyId = null;
    }
  });

TAURI.event.listen('agent_waiting', (event) => {
      const { pty_id, workspace_id, title, body } = event.payload;
      if (window.showToast) window.showToast(title, body, 'normal');
      if (window.__wmux_incrementUnread) window.__wmux_incrementUnread(workspace_id);
      for (const [containerId, entry] of Object.entries(terminals)) {
        if (entry.ptyId === pty_id) {
          const pane = document.getElementById(containerId);
          if (pane) pane.closest('.pane')?.classList.add('waiting');
          break;
        }
      }
    });
}

// ===== Split pane =====

function splitRight() {
  paneCounter++;
  const paneId = 'pane-' + paneCounter;
  const xtermId = 'xterm-' + paneCounter;

  // Find the current pane row
  const grid = document.querySelector('.pane-grid');
  const row = grid.querySelector('.pane-row') || grid;

  // Create resize handle
  const handle = document.createElement('div');
  handle.className = 'resize-h';

  // Create new pane
  const pane = document.createElement('div');
  pane.className = 'pane';
  pane.id = paneId;
  pane.innerHTML = `
    <div class="pane-header">
<div class="pane-header-left">
<span class="agent-badge term${(paneCounter % 3) + 1}">Terminal ${paneCounter}</span>
<span>S:\\wmux-</span>
</div>
<button class="pane-close-btn" title="Close (Ctrl+W)">✕</button>
</div>
      <div class="xterm-container" id="${xtermId}"></div>
    `;

pane.querySelector('.pane-close-btn').addEventListener('click', (ev) => { ev.stopPropagation(); closePane(pane); });
pane.addEventListener('mousedown', () => {
    document.querySelectorAll('.pane').forEach(p => { p.classList.remove('focused'); p.classList.remove('waiting'); });
    pane.classList.add('focused');
    if (window.PaneManager) window.PaneManager.focusedId = pane.id;
});

row.appendChild(handle);
row.appendChild(pane);

// Setup resize handle
  setupSingleResizeH(handle);

  setTimeout(() => createTerminal(xtermId, 'S:\\wmux-', window.__wmux_activeWorkspaceId || 'default'), 100);

  // Refit all existing terminals
  setTimeout(() => {
    Object.values(terminals).forEach(t => {
      try { t.fitAddon.fit(); } catch(e) {}
    });
  }, 200);
}

function splitDown() {
  paneCounter++;
  const paneId = 'pane-' + paneCounter;
  const xtermId = 'xterm-' + paneCounter;

  const grid = document.querySelector('.pane-grid');

  // Create resize handle
  const handle = document.createElement('div');
  handle.className = 'resize-v';

  // Create new row with pane
  const newRow = document.createElement('div');
  newRow.className = 'pane-row';
  newRow.innerHTML = `
<div class="pane" id="${paneId}">
<div class="pane-header">
<div class="pane-header-left">
<span class="agent-badge term${(paneCounter % 3) + 1}">Terminal ${paneCounter}</span>
<span>S:\\wmux-</span>
</div>
<button class="pane-close-btn" title="Close (Ctrl+W)">✕</button>
</div>
<div class="xterm-container" id="${xtermId}"></div>
</div>
      </div>
    `;

    const newPane = newRow.querySelector('.pane');
newPane.addEventListener('mousedown', () => {
    document.querySelectorAll('.pane').forEach(p => { p.classList.remove('focused'); p.classList.remove('waiting'); });
    newPane.classList.add('focused');
    if (window.PaneManager) window.PaneManager.focusedId = newPane.id;
});

    grid.appendChild(handle);
    grid.appendChild(newRow);

  setupSingleResizeV(handle);

  setTimeout(() => createTerminal(xtermId, 'S:\\wmux-', window.__wmux_activeWorkspaceId || 'default'), 100);
  setTimeout(() => {
    Object.values(terminals).forEach(t => {
      try { t.fitAddon.fit(); } catch(e) {}
    });
  }, 200);
}

// ===== Resize handle wiring =====

function setupSingleResizeH(handle) {
  handle.addEventListener('mousedown', (e) => {
    e.preventDefault();
    const prev = handle.previousElementSibling;
    const next = handle.nextElementSibling;
    if (!prev || !next) return;

    const row = handle.parentElement;
    const startX = e.clientX;
    const rowW = row.offsetWidth;
    const prevFlex = parseFloat(getComputedStyle(prev).flexGrow) || 1;
    const nextFlex = parseFloat(getComputedStyle(next).flexGrow) || 1;
    const total = prevFlex + nextFlex;

    handle.classList.add('active');
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const onMove = (e) => {
      const dx = e.clientX - startX;
      const d = (dx / rowW) * total;
      prev.style.flex = Math.max(0.1, prevFlex + d);
      next.style.flex = Math.max(0.1, nextFlex - d);
      // Refit terminals
      Object.values(terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
    };
    const onUp = () => {
      handle.classList.remove('active');
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      Object.values(terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  });
}

function setupSingleResizeV(handle) {
  handle.addEventListener('mousedown', (e) => {
    e.preventDefault();
    const prev = handle.previousElementSibling;
    const next = handle.nextElementSibling;
    if (!prev || !next) return;

    const grid = handle.parentElement;
    const startY = e.clientY;
    const gridH = grid.offsetHeight;
    const prevFlex = parseFloat(getComputedStyle(prev).flexGrow) || 1;
    const nextFlex = parseFloat(getComputedStyle(next).flexGrow) || 1;
    const total = prevFlex + nextFlex;

    handle.classList.add('active');
    document.body.style.cursor = 'row-resize';
    document.body.style.userSelect = 'none';

    const onMove = (e) => {
      const dy = e.clientY - startY;
      const d = (dy / gridH) * total;
      prev.style.flex = Math.max(0.1, prevFlex + d);
      next.style.flex = Math.max(0.1, nextFlex - d);
      Object.values(terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
    };
    const onUp = () => {
      handle.classList.remove('active');
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      Object.values(terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  });
}

// ===== Close pane =====

function closePane(paneElement) {
  if (!paneElement) return;

  // Find and kill the PTY
  const xtermContainer = paneElement.querySelector('.xterm-container');
  if (xtermContainer && terminals[xtermContainer.id]) {
    const entry = terminals[xtermContainer.id];
    if (entry.ptyId && isTauri) {
      TAURI.core.invoke('close_pty', { ptyId: entry.ptyId }).catch(() => {});
    }
    entry.ro?.disconnect();
    entry.term.dispose();
    delete terminals[xtermContainer.id];
  }

  const parent = paneElement.parentElement; // .pane-row or .pane-grid
  if (!parent) return;

  // Don't close the last pane — just clear and respawn
  const allPanes = document.querySelectorAll('.pane');
  if (allPanes.length <= 1) {
    // Respawn fresh terminal in same pane
    if (xtermContainer) {
xtermContainer.innerHTML = '';
        setTimeout(() => createTerminal(xtermContainer.id, 'S:\\wmux-', window.__wmux_activeWorkspaceId || 'default'), 50);
    }
    return;
  }

  // Remove adjacent resize handle
  const prevSib = paneElement.previousElementSibling;
  const nextSib = paneElement.nextElementSibling;
  if (prevSib && prevSib.classList.contains('resize-h')) {
    prevSib.remove();
  } else if (nextSib && nextSib.classList.contains('resize-h')) {
    nextSib.remove();
  }

  paneElement.remove();

  // If the row is now empty, remove the row and its resize-v
  if (parent.classList.contains('pane-row') && parent.querySelectorAll('.pane').length === 0) {
    const prevRow = parent.previousElementSibling;
    if (prevRow && prevRow.classList.contains('resize-v')) {
      prevRow.remove();
    }
    const nextRow = parent.nextElementSibling;
    if (nextRow && nextRow.classList.contains('resize-v')) {
      nextRow.remove();
    }
    parent.remove();
  }

  // Refit remaining terminals
  setTimeout(() => {
    Object.values(terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
  }, 100);

  // Focus next available pane
  const remaining = document.querySelector('.pane');
  if (remaining) {
    document.querySelectorAll('.pane').forEach(p => p.classList.remove('focused'));
    remaining.classList.add('focused');
    const entry = terminals[remaining.querySelector('.xterm-container')?.id];
    if (entry) entry.term.focus();
  }
}

function closeFocusedPane() {
  const focused = document.querySelector('.pane.focused') || document.querySelector('.pane');
  if (focused) closePane(focused);
}

// ===== Keyboard shortcuts =====

document.addEventListener('keydown', (e) => {
  // Ctrl+W — close focused pane
  if (e.ctrlKey && !e.shiftKey && (e.key === 'w' || e.key === 'W') && !e.shiftKey) {
    e.preventDefault();
    closeFocusedPane();
    return;
  }
  // Ctrl+D — split right
  if (e.ctrlKey && !e.shiftKey && e.key === 'd') {
    e.preventDefault();
    splitRight();
    return;
  }
  // Ctrl+Shift+D — split down
  if (e.ctrlKey && e.shiftKey && e.key === 'D') {
    e.preventDefault();
    splitDown();
    return;
  }
  // Ctrl+W — close focused pane
  if (e.ctrlKey && !e.shiftKey && (e.key === 'w' || e.key === 'W')) {
    e.preventDefault();
    const focused = document.querySelector('.pane.focused');
    if (focused) window.closePane(focused.id);
    return;
  }

  // Alt+Arrow — focus pane
  if (e.altKey && ['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)) {
    e.preventDefault();
    const focused = document.querySelector('.pane.focused') || document.querySelector('.pane');
    if (!focused) return;
    const panes = Array.from(document.querySelectorAll('.pane'));
    const rect = focused.getBoundingClientRect();
    const cx = rect.left + rect.width/2, cy = rect.top + rect.height/2;
    let best = null, bestDist = Infinity;
    panes.forEach(p => {
      if (p === focused) return;
      const r = p.getBoundingClientRect();
      const px = r.left + r.width/2, py = r.top + r.height/2;
      let ok = false;
      if (e.key === 'ArrowLeft') ok = px < cx - 20;
      if (e.key === 'ArrowRight') ok = px > cx + 20;
      if (e.key === 'ArrowUp') ok = py < cy - 20;
      if (e.key === 'ArrowDown') ok = py > cy + 20;
      if (ok) { const d = Math.hypot(px-cx, py-cy); if (d < bestDist) { bestDist = d; best = p; } }
    });
    if (best) {
      document.querySelectorAll('.pane').forEach(p => p.classList.remove('focused'));
      best.classList.add('focused');
      // Focus the xterm in that pane
      const entry = terminals[best.querySelector('.xterm-container')?.id];
      if (entry) entry.term.focus();
    }
  }
});

// ===== Init =====

window.addEventListener('DOMContentLoaded', () => {
  if (!XTerm) {
    console.error('[terminal] xterm.js not loaded!');
    return;
  }
  console.log('[terminal] xterm.js ready, creating main terminal...');
  setupPtyListener();

  // Create the main terminal — ONE big terminal filling the window
  // Close button delegation
  document.querySelector('.pane-grid').addEventListener('click', (e) => {
    if (e.target.classList.contains('pane-close')) {
      const pane = e.target.closest('.pane');
      if (pane) window.closePane(pane.id);
    }
  });

  setTimeout(() => createTerminal('xterm-1', 'S:\\wmux-', window.__wmux_activeWorkspaceId || 'default'), 50);
});

// Exports for app.js
function closeTerminal(containerId) {
    const entry = terminals[containerId];
    if (!entry) return;
    if (entry.ptyId && isTauri) {
        TAURI.core.invoke('close_pty', { ptyId: entry.ptyId }).catch(() => {});
    }
    try { entry.ro.disconnect(); } catch(e) {}
    try { entry.fitAddon.dispose(); } catch(e) {}
    try { entry.term.dispose(); } catch(e) {}
    delete terminals[containerId];
}

function capturePaneLayout() {
  const grid = document.querySelector('.pane-grid');
  if (!grid) return { rows: [] };

  const rows = [];
  for (const child of grid.children) {
    if (!child.classList.contains('pane-row')) continue;
    const rowFlex = parseFloat(getComputedStyle(child).flexGrow) || 1;
    const panes = [];
    for (const rc of child.children) {
      if (!rc.classList.contains('pane')) continue;
      const paneFlex = parseFloat(getComputedStyle(rc).flexGrow) || 1;
      const xtermEl = rc.querySelector('.xterm-container');
      const xtermId = xtermEl ? xtermEl.id : null;
      const entry = xtermId ? terminals[xtermId] : null;
      panes.push({
        id: rc.id,
        xtermId,
        flex: paneFlex,
        ptyId: entry?.ptyId || null,
        cwd: entry?.cwd || null,
      });
    }
    if (panes.length > 0) rows.push({ flex: rowFlex, panes });
  }
  return { rows };
}

async function restorePaneLayout(layout) {
  if (!layout || !layout.rows || layout.rows.length === 0) return;

  const grid = document.querySelector('.pane-grid');
  if (!grid) return;

  Object.values(terminals).forEach(t => {
    if (t.ptyId && isTauri) TAURI.core.invoke('close_pty', { ptyId: t.ptyId }).catch(() => {});
    try { t.ro?.disconnect(); } catch(_) {}
    try { t.term.dispose(); } catch(_) {}
  });
  for (const k of Object.keys(terminals)) delete terminals[k];

  grid.innerHTML = '';

  paneCounter = 0;

  for (let ri = 0; ri < layout.rows.length; ri++) {
    const rowData = layout.rows[ri];

    if (ri > 0) {
      const vHandle = document.createElement('div');
      vHandle.className = 'resize-v';
      grid.appendChild(vHandle);
      setupSingleResizeV(vHandle);
    }

    const row = document.createElement('div');
    row.className = 'pane-row';
    row.style.flex = rowData.flex || 1;

    for (let pi = 0; pi < rowData.panes.length; pi++) {
      const paneData = rowData.panes[pi];
      paneCounter++;
      const paneId = paneData.id || ('pane-' + paneCounter);
      const xtermId = paneData.xtermId || ('xterm-' + paneCounter);

      if (pi > 0) {
        const hHandle = document.createElement('div');
        hHandle.className = 'resize-h';
        row.appendChild(hHandle);
        setupSingleResizeH(hHandle);
      }

      const pane = document.createElement('div');
      pane.className = 'pane';
      pane.id = paneId;
      pane.style.flex = paneData.flex || 1;
      pane.innerHTML = `
        <div class="pane-header">
          <div class="pane-header-left">
            <span class="agent-badge term${(paneCounter % 3) + 1}">Terminal ${paneCounter}</span>
            <span>${paneData.cwd || 'S:\\wmux-'}</span>
          </div>
          <button class="pane-close-btn" title="Close (Ctrl+W)">✕</button>
        </div>
        <div class="xterm-container" id="${xtermId}"></div>
      `;
      pane.querySelector('.pane-close-btn').addEventListener('click', (ev) => { ev.stopPropagation(); closePane(pane); });
pane.addEventListener('mousedown', () => {
    document.querySelectorAll('.pane').forEach(p => { p.classList.remove('focused'); p.classList.remove('waiting'); });
    pane.classList.add('focused');
    if (window.PaneManager) window.PaneManager.focusedId = pane.id;
});

const cwd = paneData.cwd || null;
    setTimeout(() => createTerminal(xtermId, cwd, window.__wmux_activeWorkspaceId || 'default'), 50 + (ri * rowData.panes.length + pi) * 100);
    }

    grid.appendChild(row);
  }

  setTimeout(() => {
    Object.values(terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
  }, 500);

  const firstPane = grid.querySelector('.pane');
  if (firstPane) firstPane.classList.add('focused');
}

window.splitRight = splitRight;
window.splitDown = splitDown;
window.createTerminal = createTerminal;
window.closeFocusedPane = closeFocusedPane;
window.closePane = closePane;
window.closeTerminal = closeTerminal;
window.capturePaneLayout = capturePaneLayout;
window.restorePaneLayout = restorePaneLayout;
window.terminals = terminals;

})();
