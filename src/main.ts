import { fs, isBrowser } from "./fs.ts";
import type { FileEntry, PaneState, LayoutNode, LayoutSplit, SplitDirection } from "./types.ts";
import { createPane, loadDirectory, navigateInto, navigateUp, renderPane, buildDisplayList } from "./pane.ts";
import { countLeaves, splitPane, removePane, collectLeafIds } from "./layout.ts";
import { canAddPane, setLicenseKey } from "./licensing.ts";
import { initTheme, cycleTheme, getTheme } from "./theme.ts";

let layoutRoot: LayoutNode;
const paneMap = new Map<string, PaneState>();
let homePath = "";
let paneCounter = 0;
let bannerDismissed = false;
let activePaneId: string | null = null;
let showHidden = false;
let fileClipboard: { entries: FileEntry[]; mode: "copy" | "cut" } | null = null;

function nextPaneId(): string {
  return "pane-" + paneCounter++;
}

const THEME_LABELS: Record<string, string> = {
  dark: "Dark",
  light: "Gay",
  "3.1": "3.1",
  tui: "TUI",
};

function filterHidden(pane: PaneState): PaneState {
  if (showHidden) return pane;
  const entries = pane.entries.filter((e) => !e.name.startsWith("."));
  return { ...pane, entries, focusIndex: entries.length > 0 ? Math.min(pane.focusIndex, entries.length - 1) : -1 };
}

function getActivePane(): PaneState | null {
  if (!activePaneId) return null;
  return paneMap.get(activePaneId) ?? null;
}

function getDisplayList(pane: PaneState): Array<{ entry: FileEntry; depth: number }> {
  return buildDisplayList(pane.entries, pane.expandedPaths, pane.childrenCache, 0);
}

function updateFocusCursorDOM() {
  for (const [id, pane] of paneMap) {
    const container = document.querySelector(`.pane[data-pane-id="${id}"]`);
    if (!container) continue;
    const rows = container.querySelectorAll<HTMLElement>(".pane-row");
    rows.forEach((row, i) => {
      row.classList.toggle("focus-cursor", id === activePaneId && i === pane.focusIndex);
    });
  }
}

function updateActivePaneDOM() {
  document.querySelectorAll<HTMLElement>(".pane").forEach((el) => {
    el.classList.toggle("active-pane", el.dataset.paneId === activePaneId);
  });
}

function scrollFocusedRowIntoView() {
  if (!activePaneId) return;
  const container = document.querySelector(`.pane[data-pane-id="${activePaneId}"]`);
  if (!container) return;
  const focused = container.querySelector<HTMLElement>(".pane-row.focus-cursor");
  if (focused) focused.scrollIntoView({ block: "nearest" });
}

function setKeyboardNav(on: boolean) {
  document.querySelectorAll<HTMLElement>(".pane").forEach((el) => {
    el.classList.toggle("keyboard-nav", on);
  });
}

function setupKeyboardShortcuts() {
  // Mouse movement exits keyboard-nav mode
  document.addEventListener("mousemove", () => {
    setKeyboardNav(false);
  });

  document.addEventListener("keydown", (e) => {
    // Skip if a dialog is open
    if (document.querySelector(".dialog-overlay")) return;
    // Skip if an input/textarea is focused
    const tag = (document.activeElement?.tagName ?? "").toLowerCase();
    if (tag === "input" || tag === "textarea" || tag === "select") return;

    const isMac = navigator.platform.toUpperCase().includes("MAC");
    const mod = isMac ? e.metaKey : e.ctrlKey;
    const web = isBrowser();
    const pane = getActivePane();

    // Split right: Cmd+Right (desktop only)
    if (e.key === "ArrowRight" && mod && !web) {
      if (!activePaneId) return;
      e.preventDefault();
      handleSplitPane(activePaneId, "vertical");
      return;
    }

    // Split down: Cmd+Down (desktop only)
    if (e.key === "ArrowDown" && mod && !web) {
      if (!activePaneId) return;
      e.preventDefault();
      handleSplitPane(activePaneId, "horizontal");
      return;
    }

    // Arrow up/down — move focus cursor (must be after split checks)
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !mod) {
      e.preventDefault();
      setKeyboardNav(true);
      if (!pane || !activePaneId) return;
      const dl = getDisplayList(pane);
      if (dl.length === 0) return;

      const delta = e.key === "ArrowUp" ? -1 : 1;
      let newIndex = pane.focusIndex + delta;
      if (newIndex < 0) newIndex = 0;
      if (newIndex >= dl.length) newIndex = dl.length - 1;

      const focusedEntry = dl[newIndex]!.entry;

      if (e.shiftKey) {
        // Shift+arrow: extend selection
        const selectedPaths = new Set(pane.selectedPaths);
        selectedPaths.add(focusedEntry.path);
        paneMap.set(activePaneId, { ...pane, focusIndex: newIndex, selectedPaths, lastClickedPath: focusedEntry.path });
      } else {
        // Plain arrow: move + single select
        paneMap.set(activePaneId, {
          ...pane,
          focusIndex: newIndex,
          selectedPaths: new Set([focusedEntry.path]),
          lastClickedPath: focusedEntry.path,
        });
      }
      updateSelectionDOM();
      updateFocusCursorDOM();
      scrollFocusedRowIntoView();
      return;
    }

    // Enter — open / navigate into focused item
    if (e.key === "Enter") {
      if (!pane || !activePaneId) return;
      const dl = getDisplayList(pane);
      const focused = dl[pane.focusIndex];
      if (!focused) return;
      e.preventDefault();
      if (focused.entry.is_dir) {
        handleNavigate(activePaneId, focused.entry);
      } else {
        handleOpen(focused.entry);
      }
      return;
    }

    // Cmd+Backspace (Mac) or Delete key — delete selected items
    if ((e.key === "Backspace" && mod) || e.key === "Delete") {
      if (!pane || !activePaneId) return;
      e.preventDefault();
      const dl = getDisplayList(pane);
      const selected = dl.filter((item) => pane.selectedPaths.has(item.entry.path)).map((item) => item.entry);
      if (selected.length === 1) {
        handleDelete(activePaneId, selected[0]!);
      } else if (selected.length > 1) {
        handleDeleteMultiple(activePaneId, selected);
      }
      return;
    }

    // Escape — deselect and exit keyboard nav
    if (e.key === "Escape") {
      if (!pane || !activePaneId) return;
      e.preventDefault();
      paneMap.set(activePaneId, { ...pane, selectedPaths: new Set(), lastClickedPath: null, focusIndex: -1 });
      setKeyboardNav(false);
      updateSelectionDOM();
      updateFocusCursorDOM();
      return;
    }

    // Backspace (plain) — navigate up
    if (e.key === "Backspace") {
      if (!activePaneId) return;
      e.preventDefault();
      handleNavigateUp(activePaneId);
      return;
    }

    // Home — go to home directory
    if (e.key === "Home") {
      if (!activePaneId) return;
      e.preventDefault();
      handleHome(activePaneId);
      return;
    }

    // Tab — switch pane focus
    if (e.key === "Tab" && !mod && !e.shiftKey) {
      e.preventDefault();
      const leafIds = collectLeafIds(layoutRoot);
      if (leafIds.length === 0) return;
      const currentIdx = activePaneId ? leafIds.indexOf(activePaneId) : -1;
      const nextIdx = (currentIdx + 1) % leafIds.length;
      // Clear selection in old pane
      if (activePaneId) {
        const old = paneMap.get(activePaneId);
        if (old) paneMap.set(activePaneId, { ...old, selectedPaths: new Set(), lastClickedPath: null });
      }
      activePaneId = leafIds[nextIdx]!;
      // Select focused item in new pane
      const newPane = paneMap.get(activePaneId);
      if (newPane) {
        const dl = getDisplayList(newPane);
        const idx = newPane.focusIndex >= 0 && newPane.focusIndex < dl.length ? newPane.focusIndex : 0;
        const entry = dl[idx];
        if (entry) {
          paneMap.set(activePaneId, { ...newPane, focusIndex: idx, selectedPaths: new Set([entry.entry.path]), lastClickedPath: entry.entry.path });
        }
      }
      updateSelectionDOM();
      updateFocusCursorDOM();
      updateActivePaneDOM();
      scrollFocusedRowIntoView();
      return;
    }

    // Cmd+A / Ctrl+A — select all
    if (mod && e.key === "a") {
      e.preventDefault();
      if (!pane || !activePaneId) return;
      const dl = getDisplayList(pane);
      const allPaths = new Set(dl.map((item) => item.entry.path));
      paneMap.set(activePaneId, { ...pane, selectedPaths: allPaths });
      updateSelectionDOM();
      return;
    }

    // F2 — rename focused item
    if (e.key === "F2") {
      if (!pane || !activePaneId) return;
      e.preventDefault();
      const dl = getDisplayList(pane);
      const focused = dl[pane.focusIndex];
      if (!focused) return;
      // Trigger inline rename on the focused row
      const container = document.querySelector(`.pane[data-pane-id="${activePaneId}"]`);
      if (!container) return;
      const rows = container.querySelectorAll<HTMLElement>(".pane-row");
      const row = rows[pane.focusIndex];
      if (!row) return;
      const nameSpan = row.querySelector<HTMLSpanElement>(".entry-name");
      if (!nameSpan) return;
      startInlineRenameFromKeyboard(row, nameSpan, focused.entry, activePaneId);
      return;
    }

    // Cmd+C / Ctrl+C — copy
    if (mod && e.key === "c") {
      if (!activePaneId) return;
      e.preventDefault();
      handleCopy(activePaneId);
      return;
    }

    // Cmd+X / Ctrl+X — cut
    if (mod && e.key === "x") {
      if (!activePaneId) return;
      e.preventDefault();
      handleCut(activePaneId);
      return;
    }

    // Cmd+V / Ctrl+V — paste
    if (mod && e.key === "v") {
      if (!activePaneId || !fileClipboard) return;
      e.preventDefault();
      handlePaste(activePaneId);
      return;
    }

    // Close pane: Cmd+W (desktop only)
    if (e.key === "w" && mod && !web) {
      if (!activePaneId) return;
      e.preventDefault();
      handleClosePane(activePaneId);
      return;
    }

    // Toggle hidden files: Cmd+. / Ctrl+.
    if (mod && e.key === ".") {
      e.preventDefault();
      showHidden = !showHidden;
      refilterAllPanes();
      return;
    }
  });
}

function startInlineRenameFromKeyboard(
  row: HTMLElement,
  nameSpan: HTMLSpanElement,
  entry: FileEntry,
  paneId: string
) {
  const input = document.createElement("input");
  input.type = "text";
  input.className = "rename-input";
  input.value = entry.name;

  nameSpan.replaceWith(input);
  input.focus();

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
      handleRename(paneId, entry, newName);
    }
  }

  function cancel() {
    if (committed) return;
    committed = true;
    input.replaceWith(nameSpan);
  }

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") { e.preventDefault(); commit(); }
    else if (e.key === "Escape") { e.preventDefault(); cancel(); }
  });

  input.addEventListener("blur", () => {
    requestAnimationFrame(cancel);
  });
}

async function refilterAllPanes() {
  // Reload all panes from disk to get fresh entries, then filter
  const reloads: Promise<void>[] = [];
  for (const [id, pane] of paneMap) {
    reloads.push(
      loadDirectory(pane).then((updated) => {
        paneMap.set(id, filterHidden(updated));
      })
    );
  }
  await Promise.all(reloads);
  renderLayout();
}

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

  const leftPane = filterHidden(await loadDirectory(createPane(leftId, homePath)));
  const rightPane = filterHidden(await loadDirectory(createPane(rightId, homePath)));

  paneMap.set(leftId, leftPane);
  paneMap.set(rightId, rightPane);

  activePaneId = leftId;

  layoutRoot = {
    type: "split",
    direction: "vertical",
    first: { type: "leaf", paneId: leftId },
    second: { type: "leaf", paneId: rightId },
    ratio: 0.5,
  };

  setupKeyboardShortcuts();
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
  updateActivePaneDOM();
  updateFocusCursorDOM();
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

    const paneEl = renderPane(pane, paneId, {
      onNavigate: (entry: FileEntry) => handleNavigate(paneId, entry),
      onNavigateUp: () => handleNavigateUp(paneId),
      onHome: () => handleHome(paneId),
      onOpen: (entry: FileEntry) => handleOpen(entry),
      onRename: (entry: FileEntry, newName: string) => handleRename(paneId, entry, newName),
      onDelete: (entry: FileEntry) => handleDelete(paneId, entry),
      onDrop: (entries: FileEntry[], sourcePaneId: string, copy: boolean) =>
        handleDrop(paneId, entries, sourcePaneId, copy),
      getDragEntries: (entry: FileEntry) => {
        const currentPane = paneMap.get(paneId);
        if (!currentPane || !currentPane.selectedPaths.has(entry.path)) {
          return [entry];
        }
        const dl = buildDisplayList(currentPane.entries, currentPane.expandedPaths, currentPane.childrenCache, 0);
        return dl.filter((item) => currentPane.selectedPaths.has(item.entry.path)).map((item) => item.entry);
      },
      onToggleExpand: (entry: FileEntry) => handleToggleExpand(paneId, entry),
      onSelect: (entry: FileEntry, modifiers: { shift: boolean; metaOrCtrl: boolean }) =>
        handleSelect(paneId, entry, modifiers),
      onCopy: () => handleCopy(paneId),
      onCut: () => handleCut(paneId),
      onPaste: () => handlePaste(paneId),
      onSplitRight: () => handleSplitPane(paneId, "vertical"),
      onSplitBottom: () => handleSplitPane(paneId, "horizontal"),
      onClose: showClose ? () => handleClosePane(paneId) : undefined,
    });

    paneEl.addEventListener("mousedown", () => {
      if (activePaneId !== paneId) {
        activePaneId = paneId;
        updateActivePaneDOM();
      }
    });

    return paneEl;
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
  const newPane = filterHidden(await loadDirectory(
    createPane(newId, sourcePane ? sourcePane.currentPath : homePath)
  ));
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
  if (activePaneId === paneId) {
    const leafIds = collectLeafIds(layoutRoot);
    activePaneId = leafIds[0] ?? null;
  }
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
    makeDialogDraggable(dialog, titleEl);

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

function handleSelect(
  paneId: string,
  entry: FileEntry,
  modifiers: { shift: boolean; metaOrCtrl: boolean }
) {
  // If clicking in a different pane, clear previous pane's selection
  if (activePaneId && activePaneId !== paneId) {
    const prevPane = paneMap.get(activePaneId);
    if (prevPane) {
      paneMap.set(activePaneId, {
        ...prevPane,
        selectedPaths: new Set(),
        lastClickedPath: null,
      });
    }
  }
  activePaneId = paneId;

  const pane = paneMap.get(paneId);
  if (!pane) return;

  const displayList = buildDisplayList(pane.entries, pane.expandedPaths, pane.childrenCache, 0);
  const clickedIndex = displayList.findIndex((item) => item.entry.path === entry.path);
  const selectedPaths = new Set(pane.selectedPaths);

  if (modifiers.metaOrCtrl) {
    // Cmd/Ctrl+click: toggle individual item
    if (selectedPaths.has(entry.path)) {
      selectedPaths.delete(entry.path);
    } else {
      selectedPaths.add(entry.path);
    }
    paneMap.set(paneId, { ...pane, selectedPaths, lastClickedPath: entry.path, focusIndex: clickedIndex });
  } else if (modifiers.shift && pane.lastClickedPath) {
    // Shift+click: range select
    const paths = displayList.map((item) => item.entry.path);
    const anchorIdx = paths.indexOf(pane.lastClickedPath);
    const targetIdx = paths.indexOf(entry.path);

    if (anchorIdx !== -1 && targetIdx !== -1) {
      const start = Math.min(anchorIdx, targetIdx);
      const end = Math.max(anchorIdx, targetIdx);
      const rangeSelection = new Set<string>();
      for (let i = start; i <= end; i++) {
        const p = paths[i];
        if (p) rangeSelection.add(p);
      }
      paneMap.set(paneId, { ...pane, selectedPaths: rangeSelection, focusIndex: clickedIndex });
    }
  } else {
    // Plain click: clear selection, select this one
    paneMap.set(paneId, {
      ...pane,
      selectedPaths: new Set([entry.path]),
      lastClickedPath: entry.path,
      focusIndex: clickedIndex,
    });
  }

  updateSelectionDOM();
  updateFocusCursorDOM();
  updateActivePaneDOM();
}

function updateSelectionDOM() {
  for (const [id, pane] of paneMap) {
    const container = document.querySelector(`.pane[data-pane-id="${id}"]`);
    if (!container) continue;
    const rows = container.querySelectorAll<HTMLElement>(".pane-row");
    for (const row of rows) {
      const path = row.dataset.path;
      if (path && pane.selectedPaths.has(path)) {
        row.classList.add("selected");
      } else {
        row.classList.remove("selected");
      }
    }
  }
}

async function handleToggleExpand(paneId: string, entry: FileEntry) {
  const pane = paneMap.get(paneId);
  if (!pane || !entry.is_dir) return;

  const expandedPaths = new Set(pane.expandedPaths);
  const childrenCache = new Map(pane.childrenCache);

  if (expandedPaths.has(entry.path)) {
    // Collapse: remove this path and any nested expanded paths
    expandedPaths.delete(entry.path);
    childrenCache.delete(entry.path);
    // Also collapse any children that were expanded under this path
    for (const p of expandedPaths) {
      if (p.startsWith(entry.path + "/")) {
        expandedPaths.delete(p);
        childrenCache.delete(p);
      }
    }
  } else {
    // Expand: load children
    let children = await fs.readDir(entry.path);
    if (!showHidden) children = children.filter((e) => !e.name.startsWith("."));
    expandedPaths.add(entry.path);
    childrenCache.set(entry.path, children);
  }

  paneMap.set(paneId, { ...pane, expandedPaths, childrenCache });
  renderLayout();
}

async function handleNavigate(paneId: string, entry: FileEntry) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  const navigated = await navigateInto(pane, entry);
  // Reset expansion state when navigating into a new directory
  paneMap.set(paneId, filterHidden({
    ...navigated,
    selectedPaths: new Set(),
    lastClickedPath: null,
    expandedPaths: new Set(),
    childrenCache: new Map(),
  }));
  renderLayout();
}

async function handleNavigateUp(paneId: string) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  const updated = await navigateUp(pane);
  paneMap.set(paneId, filterHidden({ ...updated, selectedPaths: new Set(), lastClickedPath: null, expandedPaths: new Set(), childrenCache: new Map() }));
  renderLayout();
}

async function handleHome(paneId: string) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  const updated = await loadDirectory({ ...pane, currentPath: homePath });
  paneMap.set(paneId, filterHidden({ ...updated, selectedPaths: new Set(), lastClickedPath: null, expandedPaths: new Set(), childrenCache: new Map() }));
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

function handleCopy(paneId: string) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  const dl = getDisplayList(pane);
  const selected = dl.filter((item) => pane.selectedPaths.has(item.entry.path)).map((item) => item.entry);
  if (selected.length === 0) return;
  fileClipboard = { entries: selected, mode: "copy" };
}

function handleCut(paneId: string) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  const dl = getDisplayList(pane);
  const selected = dl.filter((item) => pane.selectedPaths.has(item.entry.path)).map((item) => item.entry);
  if (selected.length === 0) return;
  fileClipboard = { entries: selected, mode: "cut" };
}

function handlePaste(paneId: string) {
  const pane = paneMap.get(paneId);
  if (!pane || !fileClipboard) return;
  const clipEntries = fileClipboard.entries;
  const clipMode = fileClipboard.mode;
  const destDir = pane.currentPath;
  const srcDir = clipEntries[0]?.path.substring(0, clipEntries[0]!.path.lastIndexOf("/")) ?? "";

  if (clipMode === "copy" && srcDir === destDir) {
    handleSameDirPaste(paneId, clipEntries);
  } else {
    let sourcePaneId = "";
    for (const [id, p] of paneMap) {
      if (p.currentPath === srcDir) { sourcePaneId = id; break; }
    }
    if (!sourcePaneId) sourcePaneId = paneId;
    handleDrop(paneId, clipEntries, sourcePaneId, clipMode === "copy");
    if (clipMode === "cut") fileClipboard = null;
  }
}

async function handleDeleteMultiple(paneId: string, entries: FileEntry[]) {
  const deleteMessage = isBrowser()
    ? `${entries.length} items will be permanently deleted.`
    : `${entries.length} items will be moved to the Trash.`;
  const deleteAction = isBrowser() ? "Delete" : "Move to Trash";

  const confirmed = await showConfirmDialog(
    isBrowser()
      ? `Permanently delete ${entries.length} items?`
      : `Move ${entries.length} items to Trash?`,
    deleteMessage,
    deleteAction
  );
  if (!confirmed) return;

  for (const entry of entries) {
    try {
      await fs.deleteEntry(entry.path);
    } catch (e) {
      alert(`Failed to delete "${entry.name}": ${e}`);
    }
  }

  const pane = paneMap.get(paneId);
  if (pane) {
    await refreshPanesShowingPaths(pane.currentPath);
  }
}

async function handleDrop(
  targetPaneId: string,
  entries: FileEntry[],
  sourcePaneId: string,
  isCopy: boolean
) {
  const targetPane = paneMap.get(targetPaneId);
  if (!targetPane) return;
  const destDir = targetPane.currentPath;

  for (const entry of entries) {
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
        continue;
      }

      // choice === "replace": delete existing then proceed
      const existingPath = destDir + "/" + entry.name;
      try {
        await fs.deleteEntry(existingPath);
      } catch (e) {
        alert(`Failed to replace: ${e}`);
        continue;
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
  }

  await refreshPanesShowingPaths(
    paneMap.get(sourcePaneId)?.currentPath ?? "",
    paneMap.get(targetPaneId)?.currentPath ?? ""
  );
}

async function handleSameDirPaste(paneId: string, entries: FileEntry[]) {
  const pane = paneMap.get(paneId);
  if (!pane) return;
  const destDir = pane.currentPath;
  const parentDir = await fs.getParentDir(destDir);

  for (const entry of entries) {
    const choice = await showConflictDialog(entry.name);
    if (choice === "cancel") return;

    if (choice === "keep-both") {
      const newName = generateUniqueName(entry.name, pane.entries);
      try {
        // Stage through parent directory to create a real copy
        await fs.copyEntry(entry.path, parentDir);
        await fs.renameEntry(parentDir + "/" + entry.name, newName);
        await fs.moveEntry(parentDir + "/" + newName, destDir);
      } catch (e) {
        alert(`Copy failed: ${e}`);
      }
    }
    // "replace" is a no-op for same-dir copy (file already is itself)
  }

  await refreshPanesShowingPaths(destDir);
}

async function refreshPanesShowingPaths(...paths: string[]) {
  const pathSet = new Set(paths);
  const reloads: Promise<void>[] = [];
  for (const [id, pane] of paneMap) {
    if (pathSet.has(pane.currentPath)) {
      reloads.push(
        loadDirectory(pane).then((updated) => { paneMap.set(id, filterHidden(updated)); })
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

function makeDialogDraggable(dialog: HTMLElement, handle: HTMLElement) {
  let startX = 0, startY = 0, dx = 0, dy = 0;
  handle.style.cursor = "grab";

  function onMouseMove(e: MouseEvent) {
    dx = e.clientX - startX;
    dy = e.clientY - startY;
    dialog.style.transform = `translate(${dx}px, ${dy}px)`;
  }

  function onMouseUp() {
    handle.style.cursor = "grab";
    document.removeEventListener("mousemove", onMouseMove);
    document.removeEventListener("mouseup", onMouseUp);
  }

  handle.addEventListener("mousedown", (e) => {
    e.preventDefault();
    startX = e.clientX - dx;
    startY = e.clientY - dy;
    handle.style.cursor = "grabbing";
    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  });
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
    keepBothBtn.textContent = "Add Copy";

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
    makeDialogDraggable(dialog, titleEl);

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
    makeDialogDraggable(dialog, titleEl);

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
