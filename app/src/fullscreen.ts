// The window in full screen, as a game: on by default (the window opens so, see
// `tauri.conf.json`), kept in the browser storage of the window. F11 switches it.
import { getCurrentWindow } from "@tauri-apps/api/window";

const FULLSCREEN = "unsealed.fullscreen";

export const getFullscreen = () => localStorage.getItem(FULLSCREEN) !== "off";

/** Puts the window in full screen or back in a window, and remembers it. */
export async function setFullscreen(on: boolean) {
  localStorage.setItem(FULLSCREEN, on ? "on" : "off");
  // Outside Tauri (the preview), there is no window to change.
  await getCurrentWindow()
    .setFullscreen(on)
    .catch(() => {});
}

/** Switches full screen; returns the new state. */
export async function toggleFullscreen(): Promise<boolean> {
  const on = !(await getCurrentWindow()
    .isFullscreen()
    .catch(() => getFullscreen()));
  await setFullscreen(on);
  return on;
}

/** Quits the game (in full screen, the window has no title bar to close it). */
export const quitApp = () =>
  getCurrentWindow()
    .close()
    .catch(() => {});
