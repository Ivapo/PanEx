export type ThemeName = "dark" | "light" | "3.1" | "tui";

const STORAGE_KEY = "paneexplorer_theme";
const THEME_ORDER: ThemeName[] = ["dark", "light", "3.1", "tui"];

export function getTheme(): ThemeName {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "dark" || stored === "light" || stored === "3.1" || stored === "tui") {
    return stored;
  }
  return "dark";
}

export function setTheme(theme: ThemeName): void {
  localStorage.setItem(STORAGE_KEY, theme);
  document.documentElement.setAttribute("data-theme", theme);
}

export function cycleTheme(): void {
  const current = getTheme();
  const idx = THEME_ORDER.indexOf(current);
  const next = THEME_ORDER[(idx + 1) % THEME_ORDER.length] ?? "dark";
  setTheme(next);
}

export function initTheme(): void {
  setTheme(getTheme());
}
