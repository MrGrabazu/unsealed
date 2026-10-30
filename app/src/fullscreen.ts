// The window in full screen, as a game: on by default. Kept in the browser storage of the
// window, and given to the app (`set_display_fullscreen`), which opens the window hidden and
// shows it straight in the display chosen. F11 switches it, or ⌃⌘F on macOS, where the F keys
// drive the hardware (brightness, volume…) and F11 shows the desktop.
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const FULLSCREEN = "unsealed.fullscreen";

const IS_MAC = /Mac/i.test(navigator.userAgent);

/** The full screen key, as shown to the player. */
export const FULLSCREEN_KEY = IS_MAC ? "⌃⌘F" : "F11";

/** Whether a key press asks to switch full screen (F11 is kept on macOS too, with Fn). */
export const isFullscreenKey = (e: KeyboardEvent) =>
  e.key === "F11" || (IS_MAC && e.ctrlKey && e.metaKey && e.code === "KeyF");

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
