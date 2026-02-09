import { invoke } from "@tauri-apps/api/core";
import type { FileEntry, PaneState } from "./types.ts";
import { createPane, loadDirectory, navigateInto, navigateUp, renderPane } from "./pane.ts";

let panes: PaneState[] = [];

async function init() {
  const homePath = await invoke<string>("get_home_dir");

  const leftPane = createPane("left", homePath);
  const rightPane = createPane("right", homePath);

  panes = [
    await loadDirectory(leftPane),
    await loadDirectory(rightPane),
  ];

  renderAllPanes();
}

function renderAllPanes() {
  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";

  for (let i = 0; i < panes.length; i++) {
    const pane = panes[i]!;
    const el = renderPane(pane, i, {
      onNavigate: (entry: FileEntry) => handleNavigate(i, entry),
      onNavigateUp: () => handleNavigateUp(i),
      onOpen: (entry: FileEntry) => handleOpen(entry),
      onRename: (entry: FileEntry, newName: string) => handleRename(i, entry, newName),
      onDelete: (entry: FileEntry) => handleDelete(i, entry),
      onDrop: (entry: FileEntry, sourcePaneIndex: number, copy: boolean) =>
        handleDrop(i, entry, sourcePaneIndex, copy),
    });
    app.appendChild(el);

    if (i < panes.length - 1) {
      app.appendChild(createDivider(app, el));
    }
  }
}

function createDivider(container: HTMLElement, leftPane: HTMLElement): HTMLElement {
  const divider = document.createElement("div");
  divider.className = "divider";

  let startX = 0;
  let startWidth = 0;

  function onMouseMove(e: MouseEvent) {
    const delta = e.clientX - startX;
    leftPane.style.flex = "none";
    leftPane.style.width = `${Math.max(100, startWidth + delta)}px`;
  }

  function onMouseUp() {
    divider.classList.remove("dragging");
    document.removeEventListener("mousemove", onMouseMove);
    document.removeEventListener("mouseup", onMouseUp);
  }

  divider.addEventListener("mousedown", (e) => {
    e.preventDefault();
    startX = e.clientX;
    startWidth = leftPane.getBoundingClientRect().width;
    divider.classList.add("dragging");
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  });

  return divider;
}

async function handleNavigate(paneIndex: number, entry: FileEntry) {
  panes[paneIndex] = await navigateInto(panes[paneIndex]!, entry);
  renderAllPanes();
}

async function handleNavigateUp(paneIndex: number) {
  panes[paneIndex] = await navigateUp(panes[paneIndex]!);
  renderAllPanes();
}

async function handleOpen(entry: FileEntry) {
  try {
    await invoke("open_entry", { path: entry.path });
  } catch (e) {
    alert(`Failed to open: ${e}`);
  }
}

async function handleRename(paneIndex: number, entry: FileEntry, newName: string) {
  try {
    await invoke("rename_entry", { path: entry.path, newName });
    panes[paneIndex] = await loadDirectory(panes[paneIndex]!);
    renderAllPanes();
  } catch (e) {
    alert(`Rename failed: ${e}`);
  }
}

async function handleDelete(paneIndex: number, entry: FileEntry) {
  const confirmed = await showConfirmDialog(
    `Move "${entry.name}" to Trash?`,
    "This item will be moved to the Trash."
  );
  if (!confirmed) return;

  try {
    await invoke("delete_entry", { path: entry.path });
    panes[paneIndex] = await loadDirectory(panes[paneIndex]!);
    renderAllPanes();
  } catch (e) {
    alert(`Delete failed: ${e}`);
  }
}

async function handleDrop(
  targetPaneIndex: number,
  entry: FileEntry,
  sourcePaneIndex: number,
  isCopy: boolean
) {
  const targetPane = panes[targetPaneIndex]!;
  const destDir = targetPane.currentPath;

  // Check for name conflict
  const conflict = targetPane.entries.some((e) => e.name === entry.name);
  if (conflict) {
    const choice = await showConflictDialog(entry.name);
    if (choice === "cancel") return;

    if (choice === "keep-both") {
      // Auto-rename: add " (2)" suffix
      const newName = generateUniqueName(entry.name, targetPane.entries);
      // Copy/move to a temp name by renaming at source first won't work cross-pane.
      // Instead, perform the operation and then rename at destination.
      try {
        if (isCopy) {
          await invoke("copy_entry", { source: entry.path, destDir });
        } else {
          await invoke("move_entry", { source: entry.path, destDir });
        }
        // Rename the conflicting copy to the unique name
        const destPath = destDir + "/" + entry.name;
        await invoke("rename_entry", { path: destPath, newName });
      } catch (e) {
        alert(`Operation failed: ${e}`);
      }
      await refreshPanes(sourcePaneIndex, targetPaneIndex);
      return;
    }

    // choice === "replace": delete existing then proceed
    const existingPath = destDir + "/" + entry.name;
    try {
      await invoke("delete_entry", { path: existingPath });
    } catch (e) {
      alert(`Failed to replace: ${e}`);
      return;
    }
  }

  try {
    if (isCopy) {
      await invoke("copy_entry", { source: entry.path, destDir });
    } else {
      await invoke("move_entry", { source: entry.path, destDir });
    }
  } catch (e) {
    alert(`Operation failed: ${e}`);
  }

  await refreshPanes(sourcePaneIndex, targetPaneIndex);
}

async function refreshPanes(sourcePaneIndex: number, targetPaneIndex: number) {
  panes[sourcePaneIndex] = await loadDirectory(panes[sourcePaneIndex]!);
  panes[targetPaneIndex] = await loadDirectory(panes[targetPaneIndex]!);
  renderAllPanes();
}

function generateUniqueName(name: string, entries: FileEntry[]): string {
  const existingNames = new Set(entries.map((e) => e.name));
  const dotIndex = name.lastIndexOf(".");
  const baseName = dotIndex > 0 ? name.slice(0, dotIndex) : name;
  const ext = dotIndex > 0 ? name.slice(dotIndex) : "";

  let counter = 2;
  let candidate = `${baseName} (${counter})${ext}`;
  while (existingNames.has(candidate)) {
    counter++;
    candidate = `${baseName} (${counter})${ext}`;
  }
  return candidate;
}

function showConflictDialog(name: string): Promise<"replace" | "keep-both" | "cancel"> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "dialog-overlay";

    const dialog = document.createElement("div");
    dialog.className = "dialog";

    const titleEl = document.createElement("div");
    titleEl.className = "dialog-title";
    titleEl.textContent = `A file named "${name}" already exists`;

    const messageEl = document.createElement("div");
    messageEl.className = "dialog-message";
    messageEl.textContent = "What would you like to do?";

    const actions = document.createElement("div");
    actions.className = "dialog-actions";

    const cancelBtn = document.createElement("button");
    cancelBtn.className = "dialog-btn";
    cancelBtn.textContent = "Cancel";

    const keepBothBtn = document.createElement("button");
    keepBothBtn.className = "dialog-btn";
    keepBothBtn.textContent = "Keep Both";

    const replaceBtn = document.createElement("button");
    replaceBtn.className = "dialog-btn dialog-btn-danger";
    replaceBtn.textContent = "Replace";

    actions.appendChild(cancelBtn);
    actions.appendChild(keepBothBtn);
    actions.appendChild(replaceBtn);

    dialog.appendChild(titleEl);
    dialog.appendChild(messageEl);
    dialog.appendChild(actions);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    cancelBtn.focus();

    function cleanup(result: "replace" | "keep-both" | "cancel") {
      overlay.remove();
      document.removeEventListener("keydown", onKeyDown);
      resolve(result);
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") cleanup("cancel");
    }

    cancelBtn.addEventListener("click", () => cleanup("cancel"));
    keepBothBtn.addEventListener("click", () => cleanup("keep-both"));
    replaceBtn.addEventListener("click", () => cleanup("replace"));
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) cleanup("cancel");
    });
    document.addEventListener("keydown", onKeyDown);
  });
}

function showConfirmDialog(title: string, message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "dialog-overlay";

    const dialog = document.createElement("div");
    dialog.className = "dialog";

    const titleEl = document.createElement("div");
    titleEl.className = "dialog-title";
    titleEl.textContent = title;

    const messageEl = document.createElement("div");
    messageEl.className = "dialog-message";
    messageEl.textContent = message;

    const actions = document.createElement("div");
    actions.className = "dialog-actions";

    const cancelBtn = document.createElement("button");
    cancelBtn.className = "dialog-btn";
    cancelBtn.textContent = "Cancel";

    const confirmBtn = document.createElement("button");
    confirmBtn.className = "dialog-btn dialog-btn-danger";
    confirmBtn.textContent = "Move to Trash";

    actions.appendChild(cancelBtn);
    actions.appendChild(confirmBtn);

    dialog.appendChild(titleEl);
    dialog.appendChild(messageEl);
    dialog.appendChild(actions);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    // Focus Cancel by default
    cancelBtn.focus();

    function cleanup(result: boolean) {
      overlay.remove();
      document.removeEventListener("keydown", onKeyDown);
      resolve(result);
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") cleanup(false);
    }

    cancelBtn.addEventListener("click", () => cleanup(false));
    confirmBtn.addEventListener("click", () => cleanup(true));
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) cleanup(false);
    });
    document.addEventListener("keydown", onKeyDown);
  });
}

document.addEventListener("DOMContentLoaded", init);
