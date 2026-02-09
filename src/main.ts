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
    const el = renderPane(
      pane,
      (entry: FileEntry) => handleNavigate(i, entry),
      () => handleNavigateUp(i)
    );
    app.appendChild(el);
  }
}

async function handleNavigate(paneIndex: number, entry: FileEntry) {
  if (entry.is_dir) {
    panes[paneIndex] = await navigateInto(panes[paneIndex]!, entry);
    renderAllPanes();
  } else {
    const { open } = await import("@tauri-apps/plugin-shell");
    await open(entry.path);
  }
}

async function handleNavigateUp(paneIndex: number) {
  panes[paneIndex] = await navigateUp(panes[paneIndex]!);
  renderAllPanes();
}

document.addEventListener("DOMContentLoaded", init);
