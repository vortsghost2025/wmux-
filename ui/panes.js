// panes.js — Split pane manager with drag-to-resize
// Manages the pane tree: each node is either a leaf (terminal) or a split (H/V container)

const PaneManager = {
  // Pane tree root — mirrors what cmux calls "surfaces"
  root: null,
  panes: {},         // id -> { element, direction, children, ptyId, agentName, ratio }
  focusedId: null,
  nextId: 1,

  init() {
    // Build initial pane tree from the static HTML
    // Phase 2: this will create real xterm.js instances
    this.setupResizeHandles();
    this.setupPaneFocus();
  },

  // ===== Resize handles =====

  setupResizeHandles() {
    // Horizontal resize (column splitters)
    document.querySelectorAll('.resize-h').forEach(handle => {
      handle.addEventListener('mousedown', (e) => {
        e.preventDefault();
        const prevPane = handle.previousElementSibling;
        const nextPane = handle.nextElementSibling;
        if (!prevPane || !nextPane) return;

        const row = handle.parentElement;
        const startX = e.clientX;
        const rowWidth = row.offsetWidth;
        const prevFlex = parseFloat(getComputedStyle(prevPane).flexGrow) || 1;
        const nextFlex = parseFloat(getComputedStyle(nextPane).flexGrow) || 1;
        const totalFlex = prevFlex + nextFlex;

handle.classList.add('active');
document.body.style.cursor = 'col-resize';
document.body.style.userSelect = 'none';

const onMove = (e) => {
const dx = e.clientX - startX;
const dRatio = (dx / rowWidth) * totalFlex;
const newPrev = Math.max(0.15, prevFlex + dRatio);
const newNext = Math.max(0.15, nextFlex - dRatio);
prevPane.style.flex = newPrev;
nextPane.style.flex = newNext;
if (window.terminals) Object.values(window.terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
};

const onUp = () => {
handle.classList.remove('active');
document.body.style.cursor = '';
document.body.style.userSelect = '';
          document.removeEventListener('mousemove', onMove);
          document.removeEventListener('mouseup', onUp);
        };

        document.addEventListener('mousemove', onMove);
        document.addEventListener('mouseup', onUp);
      });
    });

    // Vertical resize (row splitters)
    document.querySelectorAll('.resize-v').forEach(handle => {
      handle.addEventListener('mousedown', (e) => {
        e.preventDefault();
        const prevRow = handle.previousElementSibling;
        const nextRow = handle.nextElementSibling;
        if (!prevRow || !nextRow) return;

        const grid = handle.parentElement;
        const startY = e.clientY;
        const gridHeight = grid.offsetHeight;
        const prevFlex = parseFloat(getComputedStyle(prevRow).flexGrow) || 1;
        const nextFlex = parseFloat(getComputedStyle(nextRow).flexGrow) || 1;
        const totalFlex = prevFlex + nextFlex;

 handle.classList.add('active');
 document.body.style.cursor = 'row-resize';
 document.body.style.userSelect = 'none';

 const onMove = (e) => {
 const dy = e.clientY - startY;
 const dRatio = (dy / gridHeight) * totalFlex;
 const newPrev = Math.max(0.15, prevFlex + dRatio);
 const newNext = Math.max(0.15, nextFlex - dRatio);
 prevRow.style.flex = newPrev;
 nextRow.style.flex = newNext;
 if (window.terminals) Object.values(window.terminals).forEach(t => { try { t.fitAddon.fit(); } catch(e) {} });
 };

 const onUp = () => {
 handle.classList.remove('active');
 document.body.style.cursor = '';
 document.body.style.userSelect = '';
 document.removeEventListener('mousemove', onMove);
 document.removeEventListener('mouseup', onUp);
 };

        document.addEventListener('mousemove', onMove);
        document.addEventListener('mouseup', onUp);
      });
    });
  },

  // ===== Pane focus =====

  setupPaneFocus() {
    document.querySelectorAll('.pane').forEach(pane => {
      pane.addEventListener('mousedown', () => {
        document.querySelectorAll('.pane').forEach(p => p.classList.remove('focused'));
        pane.classList.add('focused');
        this.focusedId = pane.id;
      });
    });
  },

  // ===== Focus navigation =====

  closePane(paneId) {
    const pane = document.getElementById(paneId);
    if (!pane) return;

    const xtermEl = pane.querySelector('.xterm-container');
    if (xtermEl && window.closeTerminal) {
      window.closeTerminal(xtermEl.id);
    }

    const row = pane.parentElement;

    const prevH = pane.previousElementSibling;
    const nextH = pane.nextElementSibling;
    if (prevH && prevH.classList.contains('resize-h')) {
      prevH.remove();
    } else if (nextH && nextH.classList.contains('resize-h')) {
      nextH.remove();
    }
    pane.remove();

    if (row) {
      const remainingPanes = row.querySelectorAll('.pane');
      if (remainingPanes.length > 0) {
        remainingPanes.forEach(p => p.style.flex = '1');
      } else {
        const grid = row.parentElement;
        const prevV = row.previousElementSibling;
        const nextV = row.nextElementSibling;
        if (prevV && prevV.classList.contains('resize-v')) {
          prevV.remove();
        } else if (nextV && nextV.classList.contains('resize-v')) {
          nextV.remove();
        }
        row.remove();

        if (grid) {
          const remainingRows = grid.querySelectorAll('.pane-row');
          if (remainingRows.length > 0) {
            remainingRows.forEach(r => r.style.flex = '1');
          }
        }
      }
    }

    const grid = document.querySelector('.pane-grid');
    if (!grid || !grid.querySelector('.pane')) {
      if (window.splitRight) {
        window.splitRight();
        const newPane = grid?.querySelector('.pane');
        if (newPane) {
          this.focusedId = newPane.id;
          newPane.classList.add('focused');
        }
      }
      return;
    }

    if (this.focusedId === paneId || !document.querySelector('.pane.focused')) {
      const first = grid.querySelector('.pane');
      if (first) {
        document.querySelectorAll('.pane').forEach(p => p.classList.remove('focused'));
        first.classList.add('focused');
        this.focusedId = first.id;
        const firstXterm = first.querySelector('.xterm-container');
        if (firstXterm && window.terminals && window.terminals[firstXterm.id]) {
          window.terminals[firstXterm.id].term.focus();
        }
      }
    }

    if (window.terminals) {
      Object.values(window.terminals).forEach(t => {
        try { t.fitAddon.fit(); } catch(e) {}
      });
    }
  },

  focusDirection(dir) {
    const focused = document.querySelector('.pane.focused') || document.querySelector('.pane');
    if (!focused) return;

    const panes = Array.from(document.querySelectorAll('.pane, .browser-pane'));
    const rect = focused.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;

    let best = null;
    let bestDist = Infinity;

    panes.forEach(p => {
      if (p === focused) return;
      const r = p.getBoundingClientRect();
      const px = r.left + r.width / 2;
      const py = r.top + r.height / 2;

      let valid = false;
      switch (dir) {
        case 'left':  valid = px < cx - 20; break;
        case 'right': valid = px > cx + 20; break;
        case 'up':    valid = py < cy - 20; break;
        case 'down':  valid = py > cy + 20; break;
      }

      if (valid) {
        const dist = Math.hypot(px - cx, py - cy);
        if (dist < bestDist) {
          bestDist = dist;
          best = p;
        }
      }
    });

    if (best) {
      document.querySelectorAll('.pane').forEach(p => p.classList.remove('focused'));
      best.classList.add('focused');
      this.focusedId = best.id;
    }
  },
};

window.closePane = (paneId) => PaneManager.closePane(paneId);

// Init on load
window.addEventListener('DOMContentLoaded', () => {
  PaneManager.init();
});
