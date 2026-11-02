// Whether anyone is looking at the game. While the window is minimized or hidden, or left
// alone (no focus, or no mouse nor key for a minute), every CSS animation is paused at once
// (`html[data-idle]`, comptoir.css): one still running is enough for the WebView to redraw
// the whole window at each frame. The income follows `isAway`, the music `isElsewhere`.
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** No mouse nor key for this long: the animations rest until the next move. */
const IDLE_AFTER_MS = 60_000;

let minimized = false;
let focused = typeof document === "undefined" ? true : document.hasFocus();
let lastInput = Date.now();
let idleTimer: ReturnType<typeof setTimeout> | undefined;
let resting = false;
const listeners = new Set<() => void>();

/** The window can't be seen: minimized, or hidden. */
export function isAway(): boolean {
  return minimized || document.visibilityState === "hidden";
}

function update() {
  const idle = isAway() || !focused || Date.now() - lastInput >= IDLE_AFTER_MS;
  if (idle !== resting) {
    resting = idle;
    if (idle) document.documentElement.dataset.idle = "";
    else delete document.documentElement.dataset.idle;
  }
  for (const listener of listeners) listener();
}

function onInput() {
  lastInput = Date.now();
  clearTimeout(idleTimer);
  idleTimer = setTimeout(update, IDLE_AFTER_MS);
  if (resting) update();
}

let installed = false;

/** Starts watching (once, for the whole app). */
export function watchIdle() {
  if (installed) return;
  installed = true;
  for (const type of ["pointermove", "pointerdown", "keydown", "wheel"]) {
    window.addEventListener(type, onInput, { passive: true, capture: true });
  }
  window.addEventListener("focus", () => {
    focused = true;
    onInput();
  });
  window.addEventListener("blur", () => {
    focused = false;
    update();
  });
  document.addEventListener("visibilitychange", update);
  // A minimized window is not always "hidden" for the WebView: asked to the window itself.
  try {
    const win = getCurrentWindow();
    const check = () => {
      win
        .isMinimized()
        .then((value) => {
          if (value !== minimized) {
            minimized = value;
            update();
          }
        })
        .catch(() => {});
    };
    void win.onResized(check);
    // Out of sight, WebView2 is asked to give back what memory it can (Windows; lib.rs).
    onAwayChange(() => {
      invoke("set_webview_memory_low", { low: isAway() }).catch(() => {});
    });
    void win.onFocusChanged(({ payload }) => {
      focused = payload;
      check();
      if (payload) onInput();
      else update();
    });
  } catch {
    // Outside Tauri (the preview): the browser's own events are enough.
  }
  onInput();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Whether the window can't be seen (see `isAway`), as React state. */
export function useAway(): boolean {
  return useSyncExternalStore(subscribe, isAway);
}

/** Calls `listener` each time the window may have been shown or hidden. */
export function onAwayChange(listener: () => void): () => void {
  return onChangeOf(isAway, listener);
}

/** The player is elsewhere: the window can't be seen (`isAway`), or another one has the focus.
 * The music stops then. */
export function isElsewhere(): boolean {
  return isAway() || !focused;
}

/** Calls `listener` each time the player may have left or come back (see `isElsewhere`). */
export function onElsewhereChange(listener: () => void): () => void {
  return onChangeOf(isElsewhere, listener);
}

function onChangeOf(state: () => boolean, listener: () => void): () => void {
  let last = state();
  const wrapped = () => {
    const now = state();
    if (now !== last) {
      last = now;
      listener();
    }
  };
  listeners.add(wrapped);
  return () => listeners.delete(wrapped);
}
