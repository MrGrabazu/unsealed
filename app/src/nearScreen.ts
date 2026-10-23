import { useEffect, useState, type RefObject } from "react";

/** How far outside its scrolling area an element counts as near: loaded ahead of the scroll,
 * and let go only well after it is out of sight (no flicker scrolling back and forth). */
const MARGIN = "900px";

type Listener = (near: boolean) => void;

/** One observer per scrolling area, shared by all the elements in it: a collection of
 * thousands of cards would otherwise make thousands of observers. */
const observers = new WeakMap<Element | Document, { observer: IntersectionObserver; listeners: Map<Element, Listener> }>();

/** The nearest ancestor that scrolls: the margin has to be counted from it, not from the window,
 * since a panel clips what lies beyond its edge. */
function scrollingArea(el: Element): Element | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const { overflowY, overflowX } = getComputedStyle(p);
    if (/(auto|scroll)/.test(overflowY + overflowX)) return p;
  }
  return null;
}

function observe(el: Element, listener: Listener): () => void {
  const root = scrollingArea(el);
  const key = root ?? document;
  let entry = observers.get(key);
  if (!entry) {
    const listeners = new Map<Element, Listener>();
    const observer = new IntersectionObserver(
      (changes) => {
        for (const c of changes) listeners.get(c.target)?.(c.isIntersecting);
      },
      { root, rootMargin: MARGIN },
    );
    entry = { observer, listeners };
    observers.set(key, entry);
  }
  entry.listeners.set(el, listener);
  entry.observer.observe(el);
  const { observer, listeners } = entry;
  return () => {
    observer.unobserve(el);
    listeners.delete(el);
  };
}

/** Whether the element (or, given `parent`, its parent) is on screen or close to it. Lets
 * long lists keep only the images near the eye: a decoded card image weighs over 1 MB, and
 * a browser keeps it as long as its element is in the page. */
export function useNearScreen(ref: RefObject<Element | null>, enabled: boolean, parent = false): boolean {
  const [near, setNear] = useState(!enabled);
  useEffect(() => {
    if (!enabled) return;
    const el = parent ? ref.current?.parentElement : ref.current;
    if (!el || typeof IntersectionObserver === "undefined") {
      setNear(true);
      return;
    }
    return observe(el, setNear);
  }, [ref, enabled, parent]);
  return near || !enabled;
}
