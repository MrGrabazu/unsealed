/// <reference types="vite/client" />
// New versions of the app: published on GitHub (see `plugins.updater` of tauri.conf.json),
// signed, and offered on the home screen.
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type { Update };

/** A newer version, if there is one; never during development, nor offline. */
export async function findUpdate(): Promise<Update | null> {
  if (import.meta.env.DEV) return null;
  try {
    return await check();
  } catch {
    return null;
  }
}

/** Downloads and installs the update (`onProgress`: 0 to 1, `null` while the size is
 * unknown), then restarts the app on it. */
export async function installUpdate(update: Update, onProgress: (fraction: number | null) => void) {
  let total = 0;
  let received = 0;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") {
      total = event.data.contentLength ?? 0;
      onProgress(total ? 0 : null);
    } else if (event.event === "Progress") {
      received += event.data.chunkLength;
      onProgress(total ? Math.min(1, received / total) : null);
    }
  });
  // On Windows the installer closes the app by itself; elsewhere, restart it.
  await relaunch();
}
