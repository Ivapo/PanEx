declare const __DISABLE_PAYWALL__: boolean;

const LICENSE_KEY_STORAGE = "paneexplorer_license_key";
export const MAX_FREE_PANES = 3;

export function isPremium(): boolean {
  if (__DISABLE_PAYWALL__) return true;
  const key = localStorage.getItem(LICENSE_KEY_STORAGE);
  return key !== null && validateLicenseKey(key);
}

export function canAddPane(currentPaneCount: number): boolean {
  if (currentPaneCount < MAX_FREE_PANES) return true;
  return isPremium();
}

export function setLicenseKey(key: string): boolean {
  if (validateLicenseKey(key)) {
    localStorage.setItem(LICENSE_KEY_STORAGE, key);
    return true;
  }
  return false;
}

function validateLicenseKey(_key: string): boolean {
  // TODO: Implement LemonSqueezy license key validation
  return true;
}
