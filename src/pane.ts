import { fs } from "./fs.ts";
import type { FileEntry, PaneState } from "./types.ts";
import { showContextMenu } from "./context-menu.ts";

export function createPane(id: string, initialPath: string): PaneState {
  return {
    id,
    currentPath: initialPath,
    entries: [],
    selectedPaths: new Set(),
    lastClickedPath: null,
    expandedPaths: new Set(),
    childrenCache: new Map(),
    focusIndex: -1,
  };
}

export async function loadDirectory(pane: PaneState): Promise<PaneState> {
  const entries = await fs.readDir(pane.currentPath);
  return { ...pane, entries, selectedPaths: new Set(), lastClickedPath: null, focusIndex: entries.length > 0 ? 0 : -1 };
}

export async function navigateUp(pane: PaneState): Promise<PaneState> {
  const parentPath = await fs.getParentDir(pane.currentPath);
  const updated = { ...pane, currentPath: parentPath };
  return loadDirectory(updated);
}

export async function navigateInto(
  pane: PaneState,
  entry: FileEntry
): Promise<PaneState> {
  if (!entry.is_dir) return pane;
  const updated = { ...pane, currentPath: entry.path };
  return loadDirectory(updated);
}

export interface PaneCallbacks {
  onNavigate: (entry: FileEntry) => void;
  onNavigateUp: () => void;
  onHome: () => void;
  onOpen: (entry: FileEntry) => void;
  onRename: (entry: FileEntry, newName: string) => void;
  onDelete: (entry: FileEntry) => void;
  onDrop: (entries: FileEntry[], sourcePaneId: string, copy: boolean) => void;
  getDragEntries: (entry: FileEntry) => FileEntry[];
  onToggleExpand: (entry: FileEntry) => void;
  onSelect: (entry: FileEntry, modifiers: { shift: boolean; metaOrCtrl: boolean }) => void;
  onCopy: () => void;
  onCut: () => void;
  onPaste: () => void;
  onSplitRight?: () => void;
  onSplitBottom?: () => void;
  onClose?: () => void;
}

export function renderPane(
  pane: PaneState,
  paneId: string,
  callbacks: PaneCallbacks
): HTMLElement {
  const container = document.createElement("div");
  container.className = "pane";
  container.dataset.paneId = pane.id;

  const header = document.createElement("div");
  header.className = "pane-header";

  const backBtn = document.createElement("button");
  backBtn.className = "back-btn";
  backBtn.textContent = "\u2190";
  backBtn.title = "Go up";
  backBtn.addEventListener("click", callbacks.onNavigateUp);

  const pathDisplay = document.createElement("span");
  pathDisplay.className = "pane-path";
  pathDisplay.textContent = pane.currentPath;

  const homeBtn = document.createElement("button");
  homeBtn.className = "back-btn";
  homeBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 7l5-5 5 5"/><path d="M3 6.5V12h3V9h2v3h3V6.5"/></svg>`;
  homeBtn.title = "Go home";
  homeBtn.addEventListener("click", callbacks.onHome);

  const nav = document.createElement("div");
  nav.className = "pane-nav";
  nav.appendChild(backBtn);
  nav.appendChild(homeBtn);

  header.appendChild(nav);
  header.appendChild(pathDisplay);

  const actions = document.createElement("div");
  actions.className = "pane-header-actions";

  if (callbacks.onSplitRight) {
    const splitRightBtn = document.createElement("button");
    splitRightBtn.className = "pane-action-btn";
    splitRightBtn.title = "Split right";
    splitRightBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="12" height="12" rx="1.5"/><line x1="7" y1="1" x2="7" y2="13"/></svg>`;
    splitRightBtn.addEventListener("click", callbacks.onSplitRight);
    actions.appendChild(splitRightBtn);
  }

  if (callbacks.onSplitBottom) {
    const splitBottomBtn = document.createElement("button");
    splitBottomBtn.className = "pane-action-btn";
    splitBottomBtn.title = "Split down";
    splitBottomBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="1" y="1" width="12" height="12" rx="1.5"/><line x1="1" y1="7" x2="13" y2="7"/></svg>`;
    splitBottomBtn.addEventListener("click", callbacks.onSplitBottom);
    actions.appendChild(splitBottomBtn);
  }

  if (callbacks.onClose) {
    const closeBtn = document.createElement("button");
    closeBtn.className = "pane-action-btn close-pane-btn";
    closeBtn.textContent = "\u00d7";
    closeBtn.title = "Close pane";
    closeBtn.addEventListener("click", callbacks.onClose);
    actions.appendChild(closeBtn);
  }

  header.appendChild(actions);

  const list = document.createElement("div");
  list.className = "pane-list";

  // Drop zone handlers on pane-list
  list.addEventListener("dragover", (e) => {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = e.altKey ? "copy" : "move";
    }
    list.classList.add("drop-target");
  });

  list.addEventListener("dragleave", (e) => {
    // Only remove if leaving the list itself (not entering a child)
    if (e.relatedTarget && list.contains(e.relatedTarget as Node)) return;
    list.classList.remove("drop-target");
  });

  list.addEventListener("drop", (e) => {
    e.preventDefault();
    list.classList.remove("drop-target");

    const json = e.dataTransfer?.getData("text/plain");
    if (!json) return;

    const data = JSON.parse(json) as { entries: FileEntry[]; sourcePaneId: string };
    // Prevent drop onto the same pane
    if (data.sourcePaneId === paneId) return;

    callbacks.onDrop(data.entries, data.sourcePaneId, e.altKey);
  });

  const displayList = buildDisplayList(
    pane.entries,
    pane.expandedPaths,
    pane.childrenCache,
    0
  );

  for (let i = 0; i < displayList.length; i++) {
    const { entry, depth } = displayList[i]!;
    const row = document.createElement("div");
    const selected = pane.selectedPaths.has(entry.path);
    row.className = `pane-row${entry.is_dir ? " is-dir" : ""}${selected ? " selected" : ""}`;
    row.draggable = true;
    row.dataset.path = entry.path;
    row.style.setProperty("--depth", String(depth));

    // Drag handlers on row
    row.addEventListener("dragstart", (e) => {
      row.classList.add("dragging");
      if (e.dataTransfer) {
        const dragEntries = callbacks.getDragEntries(entry);
        e.dataTransfer.effectAllowed = "copyMove";
        e.dataTransfer.setData(
          "text/plain",
          JSON.stringify({ entries: dragEntries, sourcePaneId: paneId })
        );

        // Custom compact drag image
        const ghost = document.createElement("div");
        ghost.className = "drag-ghost";
        if (dragEntries.length > 1) {
          ghost.textContent = `${dragEntries.length} items`;
        } else {
          ghost.textContent = `${entry.is_dir ? "\uD83D\uDCC1" : "\uD83D\uDCC4"} ${entry.name}`;
        }
        document.body.appendChild(ghost);
        e.dataTransfer.setDragImage(ghost, 0, 0);
        requestAnimationFrame(() => ghost.remove());
      }
    });

    row.addEventListener("dragend", () => {
      row.classList.remove("dragging");
    });

    // Toggle arrow for directories
    const toggle = document.createElement("span");
    toggle.className = "folder-toggle";
    if (entry.is_dir) {
      toggle.textContent = pane.expandedPaths.has(entry.path) ? "\u25BC" : "\u25B6";
      toggle.addEventListener("click", (e) => {
        e.stopPropagation();
        callbacks.onToggleExpand(entry);
      });
    }
    row.appendChild(toggle);

    const icon = document.createElement("span");
    icon.className = "entry-icon";
    icon.textContent = entry.is_dir ? "\uD83D\uDCC1" : "\uD83D\uDCC4";

    const name = document.createElement("span");
    name.className = "entry-name";
    name.textContent = entry.name;

    const size = document.createElement("span");
    size.className = "entry-size";
    size.textContent = entry.is_dir ? "" : formatSize(entry.size);

    row.appendChild(icon);
    row.appendChild(name);
    row.appendChild(size);

    // Prevent native text selection on shift+click
    row.addEventListener("mousedown", (e) => {
      if (e.shiftKey) e.preventDefault();
    });

    // Single-click → select
    row.addEventListener("click", (e) => {
      e.preventDefault();
      callbacks.onSelect(entry, {
        shift: e.shiftKey,
        metaOrCtrl: e.metaKey || e.ctrlKey,
      });
    });

    row.addEventListener("dblclick", (e) => {
      e.preventDefault();
      window.getSelection()?.removeAllRanges();
      if (entry.is_dir) {
        callbacks.onNavigate(entry);
      } else {
        callbacks.onOpen(entry);
      }
    });

    row.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      const isMac = navigator.platform.toUpperCase().includes("MAC");
      const mod = isMac ? "\u2318" : "Ctrl+";
      showContextMenu(e.clientX, e.clientY, [
        {
          label: "Open",
          shortcut: "Enter",
          action: () => {
            if (entry.is_dir) {
              callbacks.onNavigate(entry);
            } else {
              callbacks.onOpen(entry);
            }
          },
        },
        {
          label: "Copy",
          shortcut: `${mod}C`,
          action: () => callbacks.onCopy(),
        },
        {
          label: "Cut",
          shortcut: `${mod}X`,
          action: () => callbacks.onCut(),
        },
        {
          label: "Paste",
          shortcut: `${mod}V`,
          action: () => callbacks.onPaste(),
        },
        {
          label: "Rename",
          shortcut: "F2",
          action: () => startInlineRename(row, name, entry, callbacks.onRename),
        },
        {
          label: "Delete",
          shortcut: isMac ? "\u2318\u232B" : "Del",
          action: () => callbacks.onDelete(entry),
        },
      ]);
    });

    list.appendChild(row);
  }

  container.appendChild(header);
  container.appendChild(list);

  return container;
}

export function buildDisplayList(
  entries: FileEntry[],
  expandedPaths: Set<string>,
  childrenCache: Map<string, FileEntry[]>,
  depth: number
): Array<{ entry: FileEntry; depth: number }> {
  const result: Array<{ entry: FileEntry; depth: number }> = [];
  for (const entry of entries) {
    result.push({ entry, depth });
    if (entry.is_dir && expandedPaths.has(entry.path)) {
      const children = childrenCache.get(entry.path) ?? [];
      result.push(
        ...buildDisplayList(children, expandedPaths, childrenCache, depth + 1)
      );
    }
  }
  return result;
}

function startInlineRename(
  row: HTMLElement,
  nameSpan: HTMLSpanElement,
  entry: FileEntry,
  onRename: (entry: FileEntry, newName: string) => void
) {
  const input = document.createElement("input");
  input.type = "text";
  input.className = "rename-input";
  input.value = entry.name;

  nameSpan.replaceWith(input);
  input.focus();

  // Select name without extension for files
  if (!entry.is_dir) {
    const dotIndex = entry.name.lastIndexOf(".");
    if (dotIndex > 0) {
      input.setSelectionRange(0, dotIndex);
    } else {
      input.select();
    }
  } else {
    input.select();
  }

  let committed = false;

  function commit() {
    if (committed) return;
    committed = true;
    const newName = input.value.trim();
    input.replaceWith(nameSpan);
    if (newName && newName !== entry.name) {
      onRename(entry, newName);
    }
  }

  function cancel() {
    if (committed) return;
    committed = true;
    input.replaceWith(nameSpan);
  }

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancel();
    }
  });

  input.addEventListener("blur", () => {
    // Small delay to allow click events to fire first
    requestAnimationFrame(cancel);
  });
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const size = bytes / Math.pow(1024, i);
  return `${size.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}
