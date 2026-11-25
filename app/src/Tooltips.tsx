import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/** Space kept between the tooltip and what it explains, and from the window's edges. */
const GAP = 8;

/**
 * The game's tooltips: every `title` of the page (and every `data-tip`) is shown at once, in a
 * small leather-and-gold frame of the game, instead of the system's grey label that came after
 * a second or two. Mounted once (App.tsx): nothing to do in each screen but give an element a
 * `title` as before. On the first hover, the `title` becomes `data-tip`, so the system's own
 * label never shows.
 */
export function Tooltips() {
  const [tip, setTip] = useState<{ text: string; anchor: DOMRect } | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let target: Element | null = null;
    const hide = () => {
      target = null;
      setTip(null);
    };
    const show = (el: Element) => {
      const native = el.getAttribute("title");
      if (native) {
        el.setAttribute("data-tip", native);
        el.removeAttribute("title");
        // Still named for screen readers when it has no text of its own.
        if (!el.getAttribute("aria-label") && !el.textContent?.trim()) el.setAttribute("aria-label", native);
      }
      const text = el.getAttribute("data-tip");
      if (!text) return;
      target = el;
      setTip({ text, anchor: el.getBoundingClientRect() });
    };
    const holder = (node: EventTarget | null) =>
      node instanceof Element ? node.closest("[title]:not(svg *), [data-tip]") : null;
    const over = (e: Event) => {
      const el = holder(e.target);
      if (el && el !== target) show(el);
      else if (!el && target) hide();
    };
    const out = (e: Event) => {
      const next = (e as PointerEvent | FocusEvent).relatedTarget;
      if (target && !(next instanceof Node && target.contains(next))) {
        const el = holder(next);
        if (el) show(el);
        else hide();
      }
    };
    document.addEventListener("pointerover", over);
    document.addEventListener("pointerout", out);
    document.addEventListener("focusin", over);
    document.addEventListener("focusout", out);
    // Whatever moves the page or acts on it takes the tooltip away.
    document.addEventListener("pointerdown", hide);
    document.addEventListener("keydown", hide);
    window.addEventListener("scroll", hide, true);
    return () => {
      document.removeEventListener("pointerover", over);
      document.removeEventListener("pointerout", out);
      document.removeEventListener("focusin", over);
      document.removeEventListener("focusout", out);
      document.removeEventListener("pointerdown", hide);
      document.removeEventListener("keydown", hide);
      window.removeEventListener("scroll", hide, true);
    };
  }, []);

  // Above what it explains, centred, kept inside the window; below when there is no room above.
  useLayoutEffect(() => {
    const el = box.current;
    if (!el || !tip) return;
    const { anchor } = tip;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const above = anchor.top - GAP - h >= GAP;
    const top = above ? anchor.top - GAP - h : Math.min(window.innerHeight - GAP - h, anchor.bottom + GAP);
    const left = Math.max(GAP, Math.min(window.innerWidth - GAP - w, anchor.left + anchor.width / 2 - w / 2));
    el.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
    el.dataset.side = above ? "above" : "below";
    el.style.setProperty("--arrow-x", `${Math.round(anchor.left + anchor.width / 2 - left)}px`);
    el.style.visibility = "visible";
  }, [tip]);

  if (!tip) return null;
  return createPortal(
    <div ref={box} className="game-tip" role="tooltip" style={{ visibility: "hidden" }}>
      {tip.text}
    </div>,
    document.body,
  );
}
