const SUPPORT_PROMPT_DISMISSED = "paneexplorer_support_dismissed";
const SUPPORT_PANE_THRESHOLD = 3;

export function shouldShowSupportPrompt(currentPaneCount: number): boolean {
  if (localStorage.getItem(SUPPORT_PROMPT_DISMISSED)) return false;
  return currentPaneCount >= SUPPORT_PANE_THRESHOLD;
}

export function dismissSupportPrompt(): void {
  localStorage.setItem(SUPPORT_PROMPT_DISMISSED, "1");
}
