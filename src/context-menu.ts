export interface MenuItem {
  label: string;
  shortcut?: string;
  action: () => void;
  divider?: boolean;
}

let activeMenu: HTMLElement | null = null;

function removeActiveMenu() {
  if (activeMenu) {
    activeMenu.remove();
    activeMenu = null;
  }
}

export function showContextMenu(x: number, y: number, items: MenuItem[]) {
  removeActiveMenu();

  const menu = document.createElement("div");
  menu.className = "context-menu";

  for (const item of items) {
    if (item.divider) {
      const hr = document.createElement("div");
      hr.className = "context-menu-divider";
      menu.appendChild(hr);
    }
    const menuItem = document.createElement("div");
    menuItem.className = "context-menu-item";

    const label = document.createElement("span");
    label.textContent = item.label;
    menuItem.appendChild(label);

    if (item.shortcut) {
      const shortcut = document.createElement("span");
      shortcut.className = "context-menu-shortcut";
      shortcut.textContent = item.shortcut;
      menuItem.appendChild(shortcut);
    }

    menuItem.addEventListener("click", (e) => {
      e.stopPropagation();
      removeActiveMenu();
      item.action();
    });
    menu.appendChild(menuItem);
  }

  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;
  document.body.appendChild(menu);
  activeMenu = menu;

  // Adjust if menu goes off-screen
  const rect = menu.getBoundingClientRect();
  if (rect.right > window.innerWidth) {
    menu.style.left = `${window.innerWidth - rect.width - 4}px`;
  }
  if (rect.bottom > window.innerHeight) {
    menu.style.top = `${window.innerHeight - rect.height - 4}px`;
  }

  // Close on click outside or Escape
  function onDismiss(e: Event) {
    // Don't dismiss if the click is inside the menu
    if (e.type === "mousedown" && menu.contains(e.target as Node)) return;
    removeActiveMenu();
    document.removeEventListener("mousedown", onDismiss);
    document.removeEventListener("contextmenu", onDismiss);
    document.removeEventListener("keydown", onKeyDown);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") onDismiss(e);
  }

  // Use setTimeout to skip the full click cycle from Ctrl+click on Mac
  setTimeout(() => {
    document.addEventListener("mousedown", onDismiss);
    document.addEventListener("contextmenu", onDismiss);
    document.addEventListener("keydown", onKeyDown);
  }, 100);
}
