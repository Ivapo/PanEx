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
    const el = renderPane(pane, {
      onNavigate: (entry: FileEntry) => handleNavigate(i, entry),
      onNavigateUp: () => handleNavigateUp(i),
      onOpen: (entry: FileEntry) => handleOpen(entry),
      onRename: (entry: FileEntry, newName: string) => handleRename(i, entry, newName),
      onDelete: (entry: FileEntry) => handleDelete(i, entry),
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
