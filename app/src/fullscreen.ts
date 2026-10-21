// The window in full screen, as a game: on by default. Kept in the browser storage of the
// window, and given to the app (`set_display_fullscreen`), which opens the window hidden and
// shows it straight in the display chosen. F11 switches it.
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const FULLSCREEN = "unsealed.fullscreen";

export const getFullscreen = () => localStorage.getItem(FULLSCREEN) !== "off";

/** Puts the window in full screen or back in a window, and remembers it. */
export async function setFullscreen(on: boolean) {
  localStorage.setItem(FULLSCREEN, on ? "on" : "off");
  // Outside Tauri (the preview), there is no window to change.
  await invoke("set_display_fullscreen", { on }).catch(() => {});
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
