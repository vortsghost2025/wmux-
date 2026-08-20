// panes.js — Recursive split-pane manager with drag-to-resize
// Manages a tree of pane nodes (horizontal/vertical splits) and leaf panes (terminals/browsers)

const PaneManager = {
  root: null,           // Root node of the pane tree
  leaves: {},           // leafId -> { element, ptyId, agentName, type, cwd }
  focusedLeafId: null,
  nextId: 1,
  gridElement: null,

  init() {
    this.gridElement = document.getElementById('pane-grid');
    if (!this.gridElement) {
      console.error('[PaneManager] pane-grid not found');
      return;
    }
    this.setupGlobalResizeHandlers();
    this.setupGlobalFocusHandling();
  },

  // ===== Tree structure =====

  // Create root leaf pane (initial terminal)
  createRootLeaf(containerId, type = 'terminal') {
    const leafId = 'leaf-' + this.nextId++;
    const node = {
      type: 'leaf',
      id: leafId,
      containerId,
      paneType: type,
      ratio: 1,
    };
    this.root = node;
    this.renderTree();
    return leafId;
  },

  // Split a leaf into two leaves with a resize handle between them
  splitLeaf(leafId, direction) {
    const leafNode = this.findLeaf(leafId);
    if (!leafNode) return;

    const newLeafId = 'leaf-' + this.nextId++;
    const parent = leafNode.parent;
    const isHorizontal = direction === 'right';
    const isVertical = direction === 'down';

    // If the leaf's parent has a different direction, we need to wrap
    if (parent && parent.type !== 'root') {
      const parentIsHorizontal = parent.direction === 'horizontal';
      if ((isHorizontal && !parentIsHorizontal) || (isVertical && parentIsHorizontal)) {
        // Need to create a new intermediate node
        this.wrapAndSplit(parent, leafNode, direction, newLeafId);
        return newLeafId;
      }
    }

    // Create new leaf node
    const newLeafNode = {
      type: 'leaf',
      id: newLeafId,
      containerId: 'xterm-' + this.nextId++,
      paneType: leafNode.paneType,
      ratio: 0.5,
      parent: null,
    };

    // Replace leaf with a split node
    const splitNode = {
      type: 'split',
      direction: isHorizontal ? 'horizontal' : 'vertical',
      children: [leafNode, newLeafNode],
      ratio: 0.5,
      parent: parent,
    };

    leafNode.parent = splitNode;
    newLeafNode.parent = splitNode;

    if (parent) {
      const idx = parent.children.indexOf(leafNode);
      parent.children[idx] = splitNode;
    } else {
      this.root = splitNode;
    }

    this.renderTree();
    this.focusLeaf(newLeafId);
    return newLeafId;
  },

  wrapAndSplit(parent, leafNode, direction, newLeafId) {
    const isHorizontal = direction === 'right';
    const grandParent = parent.parent;
    const parentIdx = grandParent ? grandParent.children.indexOf(parent) : -1;

    const newSplitNode = {
      type: 'split',
      direction: isHorizontal ? 'horizontal' : 'vertical',
      children: [leafNode, {
        type: 'leaf',
        id: newLeafId,
        containerId: 'xterm-' + (this.nextId++),
        paneType: leafNode.paneType,
        ratio: 0.5,
        parent: null,
      }],
      ratio: 0.5,
      parent: grandParent,
    };

    leafNode.parent = newSplitNode;
    newSplitNode.children[1].parent = newSplitNode;

    if (grandParent) {
      grandParent.children[parentIdx] = newSplitNode;
    } else {
      this.root = newSplitNode;
    }

    parent.parent = newSplitNode;
    this.renderTree();
  },

  findLeaf(leafId) {
    return this._findLeafInNode(this.root, leafId);
  },

  _findLeafInNode(node, leafId) {
    if (!node) return null;
    if (node.type === 'leaf' && node.id === leafId) return node;
    if (node.children) {
      for (const child of node.children) {
        const found = this._findLeafInNode(child, leafId);
        if (found) return found;
      }
    }
    return null;
  },

  closeLeaf(leafId) {
    const leafNode = this.findLeaf(leafId);
    if (!leafNode) return;

    const parent = leafNode.parent;
    if (!parent || parent.type === 'root') {
      // Last pane - don't close, just clear
      const entry = window.terminals?.[leafNode.containerId];
      if (entry) {
        entry.ro?.disconnect();
        entry.term?.dispose();
        delete window.terminals[leafNode.containerId];
      }
      setTimeout(() => {
        const container = document.getElementById(leafNode.containerId);
        if (container) container.innerHTML = '';
        window.createTerminal(leafNode.containerId, 'S:\\\\wmux-', window.__wmux_activeWorkspaceId || 'default');
      }, 50);
      return;
    }

    const sibling = parent.children.find(c => c.id !== leafId);
    const grandParent = parent.parent;

    // Clean up terminal
    const entry = window.terminals?.[leafNode.containerId];
    if (entry) {
      entry.ro?.disconnect();
      entry.term?.dispose();
      delete window.terminals[leafNode.containerId];
    }

    if (grandParent) {
      const idx = grandParent.children.indexOf(parent);
      sibling.parent = grandParent;
      grandParent.children[idx] = sibling;
    } else {
      sibling.parent = null;
      this.root = sibling;
    }

    this.renderTree();
    this.focusLeaf(sibling.id);
  },

  // ===== Rendering =====

  renderTree() {
    if (!this.gridElement) return;
    this.gridElement.innerHTML = '';
    if (this.root) {
      const element = this.renderNode(this.root);
      this.gridElement.appendChild(element);
    }
  },

  renderNode(node) {
    if (node.type === 'leaf') {
      return this.renderLeaf(node);
    } else {
      return this.renderSplit(node);
    }
  },

  renderLeaf(node) {
    const el = document.createElement('div');
    el.className = 'pane-leaf';
    el.id = node.id;
    el.dataset.leafId = node.id;

    let badgeClass = 'term1';
    let label = 'Terminal';
    if (node.paneType === 'browser') {
      badgeClass = 'browser';
      label = 'Browser';
    } else if (node.paneType === 'agent') {
      badgeClass = 'agent';
      label = 'Agent';
    }

    el.innerHTML = `
      <div class="pane-header">
        <div class="pane-header-left">
          <span class="agent-badge ${badgeClass}">${label}</span>
          <span class="pane-cwd">S:\\wmux-</span>
        </div>
        <button class="pane-close-btn" title="Close (Ctrl+W)">✕</button>
      </div>
      <div class="xterm-container" id="${node.containerId}"></div>
    `;

    // Close button handler
    const closeBtn = el.querySelector('.pane-close-btn');
    closeBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      this.closeLeaf(node.id);
    });

    // Focus handler
    el.addEventListener('mousedown', (e) => {
      e.stopPropagation();
      this.focusLeaf(node.id);
    });

    // Store reference
    this.leaves[node.id] = { element: el, ...node };

    // Create terminal after a brief delay
    if (node.paneType !== 'browser') {
      setTimeout(() => {
        window.createTerminal(node.containerId, 'S:\\\\wmux-', window.__wmux_activeWorkspaceId || 'default');
      }, 50);
    }

    return el;
  },

  renderSplit(node) {
    const el = document.createElement('div');
    el.className = `pane-node ${node.direction}`;
    el.dataset.splitId = node.id;

    node.children.forEach((child, idx) => {
      const childEl = this.renderNode(child);
      childEl.style.flex = child.ratio || 1;
      el.appendChild(childEl);

      if (idx < node.children.length - 1) {
        const handle = document.createElement('div');
        handle.className = `resize-handle ${node.direction}`;
        handle.dataset.handleFor = node.id;
        handle.dataset.childIdx = idx;
        el.appendChild(handle);
      }
    });

    return el;
  },

  // ===== Resize handling =====

  setupGlobalResizeHandlers() {
    document.addEventListener('mousedown', (e) => {
      if (!e.target.classList.contains('resize-handle')) return;
      e.preventDefault();
      const handle = e.target;
      
      const parentEl = handle.parentElement;
      const idx = parseInt(handle.dataset.childIdx);
      const children = Array.from(parentEl.children).filter(c => c.classList?.contains('pane-node') || c.classList?.contains('pane-leaf'));
      const prevChild = children[idx];
      const nextChild = children[idx + 1];
      if (!prevChild || !nextChild) return;

      const isHorizontal = handle.classList.contains('horizontal');
      const startPos = isHorizontal ? e.clientX : e.clientY;
      const parentSize = isHorizontal ? parentEl.offsetWidth : parentEl.offsetHeight;
      const prevFlex = parseFloat(prevChild.style.flex) || 1;
      const nextFlex = parseFloat(nextChild.style.flex) || 1;
      const totalFlex = prevFlex + nextFlex;

      handle.classList.add('active');
      document.body.style.cursor = isHorizontal ? 'col-resize' : 'row-resize';
      document.body.style.userSelect = 'none';

      const onMove = (e) => {
        const delta = (isHorizontal ? e.clientX : e.clientY) - startPos;
        const dRatio = (delta / parentSize) * totalFlex;
        const newPrev = Math.max(0.15, prevFlex + dRatio);
        const newNext = Math.max(0.15, nextFlex - dRatio);

        prevChild.style.flex = newPrev;
        nextChild.style.flex = newNext;

        // Refit terminals
        if (window.terminals) {
          Object.values(window.terminals).forEach(t => {
            try { t.fitAddon.fit(); } catch(e) {}
          });
        }
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
  },

  // ===== Focus handling =====

  setupGlobalFocusHandling() {
    document.addEventListener('mousedown', (e) => {
      const leafEl = e.target.closest('.pane-leaf');
      if (leafEl) {
        const leafId = leafEl.dataset.leafId;
        if (leafId) this.focusLeaf(leafId);
      }
    });
  },

  focusLeaf(leafId) {
    Object.values(this.leaves).forEach(l => {
      l.element?.classList.remove('focused');
    });
    const leaf = this.leaves[leafId];
    if (leaf) {
      leaf.element.classList.add('focused');
      this.focusedLeafId = leafId;
    }
  },

  getFocusedLeaf() {
    return this.focusedLeafId ? this.findLeaf(this.focusedLeafId) : null;
  },

  // ===== Split helpers for keyboard shortcuts =====

  splitFocused(direction) {
    const focused = this.getFocusedLeaf();
    if (focused) {
      return this.splitLeaf(focused.id, direction);
    }
    return null;
  },

  closeFocused() {
    if (this.focusedLeafId) {
      this.closeLeaf(this.focusedLeafId);
    }
  },

  // ===== Layout capture/restore =====

  captureLayout() {
    return this.root ? this._captureNode(this.root) : null;
  },

  _captureNode(node) {
    if (node.type === 'leaf') {
      return {
        type: 'leaf',
        paneType: node.paneType,
        ratio: node.ratio,
      };
    } else {
      return {
        type: 'split',
        direction: node.direction,
        ratio: node.ratio,
        children: node.children.map(c => this._captureNode(c)),
      };
    }
  },

  restoreLayout(layout) {
    if (!layout) return;
    this.nextId = 1;
    this.leaves = {};
    this.root = this._restoreNode(layout);
    this.renderTree();
  },

  _restoreNode(node) {
    if (node.type === 'leaf') {
      const leafId = 'leaf-' + this.nextId++;
      const restored = {
        type: 'leaf',
        id: leafId,
        containerId: 'xterm-' + this.nextId++,
        paneType: node.paneType || 'terminal',
        ratio: node.ratio || 1,
        parent: null,
      };
      return restored;
    } else {
      const splitId = 'split-' + this.nextId++;
      const children = node.children.map(c => {
        const child = this._restoreNode(c);
        child.parent = null;
        return child;
      });
      const restored = {
        type: 'split',
        id: splitId,
        direction: node.direction,
        ratio: node.ratio || 1,
        children,
        parent: null,
      };
      children.forEach(c => c.parent = restored);
      return restored;
    }
  },
};

// Expose globally
window.PaneManager = PaneManager;
window.splitRight = () => PaneManager.splitFocused('right');
window.splitDown = () => PaneManager.splitFocused('down');
window.closeFocusedPane = () => PaneManager.closeFocused();
window.capturePaneLayout = () => PaneManager.captureLayout();
window.restorePaneLayout = (layout) => PaneManager.restoreLayout(layout);

// Init on load
window.addEventListener('DOMContentLoaded', () => {
  PaneManager.init();
});
