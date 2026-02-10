import { fs, isBrowser } from "./fs.ts";
import type { FileEntry, PaneState, LayoutNode, LayoutSplit, SplitDirection } from "./types.ts";
import { createPane, loadDirectory, navigateInto, navigateUp, renderPane } from "./pane.ts";
import { countLeaves, splitPane, removePane } from "./layout.ts";
import { canAddPane, setLicenseKey } from "./licensing.ts";
import { initTheme, cycleTheme, getTheme } from "./theme.ts";

let layoutRoot: LayoutNode;
const paneMap = new Map<string, PaneState>();
let homePath = "";
let paneCounter = 0;
let bannerDismissed = false;

function nextPaneId(): string {
  return "pane-" + paneCounter++;
}

const THEME_LABELS: Record<string, string> = {
  dark: "Dark",
  light: "Gay",
  "3.1": "3.1",
  tui: "TUI",
};

async function init() {
  initTheme();

  if (isBrowser()) {
    showFolderPicker();
    return;
  }

  homePath = await fs.getHomeDir();
  await initPanes();
}

function showFolderPicker() {
  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";

  const landing = document.createElement("div");
  landing.className = "landing";

  const title = document.createElement("h1");
  title.className = "landing-title";
  title.textContent = "PaneExplorer";

  const themeRow = document.createElement("div");
  themeRow.className = "landing-theme-row";

  const themeLabel = document.createElement("span");
  themeLabel.className = "landing-theme-label";
  themeLabel.textContent = "mode :";

  const themeToggle = document.createElement("button");
  themeToggle.className = "landing-theme";
  themeToggle.textContent = THEME_LABELS[getTheme()] ?? "Dark";
  themeToggle.addEventListener("click", () => {
    cycleTheme();
    themeToggle.textContent = THEME_LABELS[getTheme()] ?? "Dark";
  });

  themeRow.appendChild(themeLabel);
  themeRow.appendChild(themeToggle);

  const subtitle = document.createElement("p");
  subtitle.className = "landing-subtitle";
  subtitle.textContent = "Choose a folder to get started";

  const btn = document.createElement("button");
  btn.className = "landing-btn";
  btn.textContent = "Open Folder";
  btn.addEventListener("click", async () => {
    try {
      homePath = await fs.getHomeDir();
      await initPanes();
    } catch {
      // User cancelled the picker — do nothing
    }
  });

  const note = document.createElement("p");
  note.className = "landing-note";
  note.textContent = "Requires Chrome or Edge. Files never leave your device.";

  landing.appendChild(title);
  landing.appendChild(themeRow);
  landing.appendChild(subtitle);
  landing.appendChild(btn);
  landing.appendChild(note);
  app.appendChild(landing);
}

async function initPanes() {
  const leftId = nextPaneId();
  const rightId = nextPaneId();

  const leftPane = await loadDirectory(createPane(leftId, homePath));
  const rightPane = await loadDirectory(createPane(rightId, homePath));

  paneMap.set(leftId, leftPane);
  paneMap.set(rightId, rightPane);

  layoutRoot = {
    type: "split",
    direction: "vertical",
    first: { type: "leaf", paneId: leftId },
    second: { type: "leaf", paneId: rightId },
    ratio: 0.5,
  };

  renderLayout();
}

function renderLayout() {
  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";

  // Web banner (browser mode only)
  if (isBrowser() && !bannerDismissed) {
    const banner = document.createElement("div");
    banner.className = "web-banner";
    banner.innerHTML =
      'You\'re using the web version — <a href="https://paneexplorer.app" target="_blank">Download the native app</a> for the full experience';
    const closeBtn = document.createElement("button");
    closeBtn.className = "web-banner-close";
    closeBtn.textContent = "[close]";
    closeBtn.addEventListener("click", (e) => {
      e.preventDefault();
      bannerDismissed = true;
      banner.remove();
    });
    banner.appendChild(closeBtn);
    app.appendChild(banner);
  }

  // Global toolbar
  const toolbar = document.createElement("div");
  toolbar.className = "global-toolbar";

  const themeBtn = document.createElement("button");
  themeBtn.className = "theme-btn";
  themeBtn.textContent = THEME_LABELS[getTheme()] ?? "Dark";
  themeBtn.addEventListener("click", () => {
    cycleTheme();
    themeBtn.textContent = THEME_LABELS[getTheme()] ?? "Dark";
  });
  toolbar.appendChild(themeBtn);
  app.appendChild(toolbar);

  const totalPanes = countLeaves(layoutRoot);
  const el = renderNode(layoutRoot, totalPanes);
  app.appendChild(el);
}

function renderNode(node: LayoutNode, totalPanes: number): HTMLElement {
  if (node.type === "leaf") {
    const pane = paneMap.get(node.paneId);
    if (!pane) {
      const placeholder = document.createElement("div");
      placeholder.className = "pane";
      return placeholder;
    }

    const showClose = totalPanes > 2;
    const paneId = node.paneId;

    return renderPane(pane, paneId, {
      onNavigate: (entry: FileEntry) => handleNavigate(paneId, entry),
      onNavigateUp: () => handleNavigateUp(paneId),
      onHome: () => handleHome(paneId),
      onOpen: (entry: FileEntry) => handleOpen(entry),
      onRename: (entry: FileEntry, newName: string) => handleRename(paneId, entry, newName),
      onDelete: (entry: FileEntry) => handleDelete(paneId, entry),
      onDrop: (entry: FileEntry, sourcePaneId: string, copy: boolean) =>
        handleDrop(paneId, entry, sourcePaneId, copy),
      onSplitRight: () => handleSplitPane(paneId, "vertical"),
      onSplitBottom: () => handleSplitPane(paneId, "horizontal"),
      onClose: showClose ? () => handleClosePane(paneId) : undefined,
    });
  }

  // Split node
  const container = document.createElement("div");
  container.className = "split-container";
  container.style.flexDirection = node.direction === "vertical" ? "row" : "column";

  const firstEl = renderNode(node.first, totalPanes);
  const secondEl = renderNode(node.second, totalPanes);

  // Apply ratio via flex-basis
  firstEl.style.flex = `${node.ratio} 1 0`;
  secondEl.style.flex = `${1 - node.ratio} 1 0`;

  const divider = createDivider(node, firstEl, secondEl, container);

  container.appendChild(firstEl);
  container.appendChild(divider);
  container.appendChild(secondEl);

  return container;
}

function createDivider(
  splitNode: LayoutSplit,
  firstEl: HTMLElement,
  secondEl: HTMLElement,
  container: HTMLElement
): HTMLElement {
  const isVertical = splitNode.direction === "vertical";
  const divider = document.createElement("div");
  divider.className = isVertical ? "divider-vertical" : "divider-horizontal";

  function onMouseMove(e: MouseEvent) {
    const rect = container.getBoundingClientRect();
    if (isVertical) {
      const offset = e.clientX - rect.left;
      splitNode.ratio = Math.max(0.1, Math.min(0.9, offset / rect.width));
    } else {
      const offset = e.clientY - rect.top;
      splitNode.ratio = Math.max(0.1, Math.min(0.9, offset / rect.height));
    }
    firstEl.style.flex = `${splitNode.ratio} 1 0`;
    secondEl.style.flex = `${1 - splitNode.ratio} 1 0`;
  }

  function onMouseUp() {
    divider.classList.remove("dragging");
    document.removeEventListener("mousemove", onMouseMove);
    document.removeEventListener("mouseup", onMouseUp);
  }

  divider.addEventListener("mousedown", (e) => {
    e.preventDefault();
    divider.classList.add("dragging");
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  });

  return divider;
}

async function handleSplitPane(paneId: string, direction: SplitDirection) {
  const totalPanes = countLeaves(layoutRoot);
  if (!canAddPane(totalPanes)) {
    const activated = await showLicensePrompt();
    if (!activated) return;
  }

  const sourcePane = paneMap.get(paneId);
  const newId = nextPaneId();
  const newPane = await loadDirectory(
    createPane(newId, sourcePane ? sourcePane.currentPath : homePath)
  );
  paneMap.set(newId, newPane);

  layoutRoot = splitPane(layoutRoot, paneId, newId, direction);
  renderLayout();
}

function handleClosePane(paneId: string) {
  if (countLeaves(layoutRoot) <= 2) return;
  const newRoot = removePane(layoutRoot, paneId);
  if (!newRoot) return;
  layoutRoot = newRoot;
  paneMap.delete(paneId);
  renderLayout();
}

function showLicensePrompt(): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "dialog-overlay";

    const dialog = document.createElement("div");
    dialog.className = "dialog";

    const titleEl = document.createElement("div");
    titleEl.className = "dialog-title";
    titleEl.textContent = "Unlock Unlimited Panes";

    const messageEl = document.createElement("div");
    messageEl.className = "dialog-message";
    messageEl.textContent =
      "Free version supports up to 3 panes. Enter a license key to unlock more.";

    const input = document.createElement("input");
    input.type = "text";
    input.className = "license-input";
    input.placeholder = "Enter license key\u2026";

    const actions = document.createElement("div");
    actions.className = "dialog-actions";

    const cancelBtn = document.createElement("button");
    cancelBtn.className = "dialog-btn";
    cancelBtn.textContent = "Cancel";

    const buyBtn = document.createElement("button");
    buyBtn.className = "dialog-btn";
    buyBtn.textContent = "Buy License ($4.99)";

    const activateBtn = document.createElement("button");
    activateBtn.className = "dialog-btn dialog-btn-primary";
    activateBtn.textContent = "Activate";

    actions.appendChild(cancelBtn);
    actions.appendChild(buyBtn);
    actions.appendChild(activateBtn);

    dialog.appendChild(titleEl);
    dialog.appendChild(messageEl);
    dialog.appendChild(input);
    dialog.appendChild(actions);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    input.focus();

    function cleanup(result: boolean) {
      overlay.remove();
      document.removeEventListener("keydown", onKeyDown);
      resolve(result);
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") cleanup(false);
    }

    cancelBtn.addEventListener("click", () => cleanup(false));

    buyBtn.addEventListener("click", () => {
      window.open("https://paneexplorer.lemonsqueezy.com", "_blank");
    });

    activateBtn.addEventListener("click", () => {
      const key = input.value.trim();
      if (key && setLicenseKey(key)) {
        cleanup(true);
      } else {
        input.classList.add("input-error");
        setTimeout(() => input.classList.remove("input-error"), 600);
      }
    });

    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        activateBtn.click();
      }
    });

    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) cleanup(false);
    });
    document.addEventListener("keydown", onKeyDown);
  });
}

async function handleNavigate(paneId: string, entry: FileEntry) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  paneMap.set(paneId, await navigateInto(pane, entry));
  renderLayout();
}

async function handleNavigateUp(paneId: string) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  paneMap.set(paneId, await navigateUp(pane));
  renderLayout();
}

async function handleHome(paneId: string) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  paneMap.set(paneId, await loadDirectory({ ...pane, currentPath: homePath }));
  renderLayout();
}

async function handleOpen(entry: FileEntry) {
  try {
    await fs.openEntry(entry.path);
  } catch (e) {
    alert(`Failed to open: ${e}`);
  }
}

async function handleRename(paneId: string, entry: FileEntry, newName: string) {
  try {
    await fs.renameEntry(entry.path, newName);
    const pane = paneMap.get(paneId);
    if (pane) {
      await refreshPanesShowingPaths(pane.currentPath);
    }
  } catch (e) {
    alert(`Rename failed: ${e}`);
  }
}

async function handleDelete(paneId: string, entry: FileEntry) {
  const deleteMessage = isBrowser()
    ? "This item will be permanently deleted."
    : "This item will be moved to the Trash.";
  const deleteAction = isBrowser() ? "Delete" : "Move to Trash";

  const confirmed = await showConfirmDialog(
    isBrowser()
      ? `Permanently delete "${entry.name}"?`
      : `Move "${entry.name}" to Trash?`,
    deleteMessage,
    deleteAction
  );
  if (!confirmed) return;

  try {
    await fs.deleteEntry(entry.path);
    const pane = paneMap.get(paneId);
    if (pane) {
      await refreshPanesShowingPaths(pane.currentPath);
    }
  } catch (e) {
    alert(`Delete failed: ${e}`);
  }
}

async function handleDrop(
  targetPaneId: string,
  entry: FileEntry,
  sourcePaneId: string,
  isCopy: boolean
) {
  const targetPane = paneMap.get(targetPaneId);
  if (!targetPane) return;
  const destDir = targetPane.currentPath;

  // Check for name conflict
  const conflict = targetPane.entries.some((e) => e.name === entry.name);
  if (conflict) {
    const choice = await showConflictDialog(entry.name);
    if (choice === "cancel") return;

    if (choice === "keep-both") {
      const newName = generateUniqueName(entry.name, targetPane.entries);
      try {
        if (isCopy) {
          await fs.copyEntry(entry.path, destDir);
        } else {
          await fs.moveEntry(entry.path, destDir);
        }
        const destPath = destDir + "/" + entry.name;
        await fs.renameEntry(destPath, newName);
      } catch (e) {
        alert(`Operation failed: ${e}`);
      }
      await refreshPanesShowingPaths(
      paneMap.get(sourcePaneId)?.currentPath ?? "",
      paneMap.get(targetPaneId)?.currentPath ?? ""
    );
      return;
    }

    // choice === "replace": delete existing then proceed
    const existingPath = destDir + "/" + entry.name;
    try {
      await fs.deleteEntry(existingPath);
    } catch (e) {
      alert(`Failed to replace: ${e}`);
      return;
    }
  }

  try {
    if (isCopy) {
      await fs.copyEntry(entry.path, destDir);
    } else {
      await fs.moveEntry(entry.path, destDir);
    }
  } catch (e) {
    alert(`Operation failed: ${e}`);
  }

  await refreshPanesShowingPaths(
      paneMap.get(sourcePaneId)?.currentPath ?? "",
      paneMap.get(targetPaneId)?.currentPath ?? ""
    );
}

async function refreshPanesShowingPaths(...paths: string[]) {
  const pathSet = new Set(paths);
  const reloads: Promise<void>[] = [];
  for (const [id, pane] of paneMap) {
    if (pathSet.has(pane.currentPath)) {
      reloads.push(
        loadDirectory(pane).then((updated) => { paneMap.set(id, updated); })
      );
    }
  }
  await Promise.all(reloads);
  renderLayout();
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

function showConfirmDialog(title: string, message: string, actionLabel = "Move to Trash"): Promise<boolean> {
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
    confirmBtn.textContent = actionLabel;

    actions.appendChild(cancelBtn);
    actions.appendChild(confirmBtn);

    dialog.appendChild(titleEl);
    dialog.appendChild(messageEl);
    dialog.appendChild(actions);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

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
