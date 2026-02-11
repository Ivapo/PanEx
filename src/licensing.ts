declare const __DISABLE_PAYWALL__: boolean;

const SUPPORT_PROMPT_DISMISSED = "paneexplorer_support_dismissed";
export const MAX_FREE_PANES = 3;

export function shouldShowSupportPrompt(currentPaneCount: number): boolean {
  if (__DISABLE_PAYWALL__) return false;
  if (localStorage.getItem(SUPPORT_PROMPT_DISMISSED)) return false;
  return currentPaneCount >= MAX_FREE_PANES;
}

export function dismissSupportPrompt(): void {
  localStorage.setItem(SUPPORT_PROMPT_DISMISSED, "1");
}
