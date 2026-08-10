/**
 * Convert a local filesystem path into a URL the Tauri webview can load.
 *
 * Tauri serves files under the `asset:` protocol when `assetProtocol.enable`
 * is on. The scope in tauri.conf.json grants access to user media folders.
 */

export function assetUrl(path: string): string {
  if (!path) return "";
  const normalized = path.replace(/\\/g, "/");
  // Windows drive letters become `C:` which the protocol treats as a host.
  return `asset://localhost/${encodeURI(normalized)}`;
}

/** True when running inside the Tauri shell (window.__TAURI_INTERNALS__). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
