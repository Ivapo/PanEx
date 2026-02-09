import { invoke } from "@tauri-apps/api/core";
import type { FileEntry, PaneState } from "./types.ts";
import { showContextMenu } from "./context-menu.ts";

export function createPane(id: string, initialPath: string): PaneState {
  return {
    id,
    currentPath: initialPath,
    entries: [],
    selectedIndex: -1,
  };
}

export async function loadDirectory(pane: PaneState): Promise<PaneState> {
  const entries = await invoke<FileEntry[]>("read_dir", {
    path: pane.currentPath,
  });
  return { ...pane, entries, selectedIndex: -1 };
}

export async function navigateUp(pane: PaneState): Promise<PaneState> {
  const parentPath = await invoke<string>("get_parent_dir", {
    path: pane.currentPath,
  });
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
  onOpen: (entry: FileEntry) => void;
  onRename: (entry: FileEntry, newName: string) => void;
  onDelete: (entry: FileEntry) => void;
}

export function renderPane(
  pane: PaneState,
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

  header.appendChild(backBtn);
  header.appendChild(pathDisplay);

  const list = document.createElement("div");
  list.className = "pane-list";

  for (const entry of pane.entries) {
    const row = document.createElement("div");
    row.className = `pane-row${entry.is_dir ? " is-dir" : ""}`;

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
      showContextMenu(e.clientX, e.clientY, [
        {
          label: "Open",
          action: () => {
            if (entry.is_dir) {
              callbacks.onNavigate(entry);
            } else {
              callbacks.onOpen(entry);
            }
          },
        },
        {
          label: "Rename",
          action: () => startInlineRename(row, name, entry, callbacks.onRename),
        },
        {
          label: "Delete",
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
