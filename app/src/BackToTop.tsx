import { useEffect, useState, type RefObject } from "react";
import { useI18n } from "./i18n";

/** Scrolled further than this, the button shows up. */
const SHOWN_FROM_PX = 400;

/** A button that scrolls back to the very top, shown once scrolled down: the page's (fixed in
 * the corner) or, with `target`, a box's of its own such as a dialog (kept in view at its
 * bottom, so it must be the box's last child). */
export function BackToTop({ target }: { target?: RefObject<HTMLElement | null> }) {
  const { t } = useI18n();
  const [shown, setShown] = useState(false);

  useEffect(() => {
    const box = target?.current ?? null;
    const scrolled = () => (box ? box.scrollTop : window.scrollY);
    const update = () => setShown(scrolled() > SHOWN_FROM_PX);
    const source: HTMLElement | Window = box ?? window;
    update();
    source.addEventListener("scroll", update, { passive: true });
    return () => source.removeEventListener("scroll", update);
  }, [target]);

  if (!shown) return null;
  return (
    <button
      className={`back-to-top ${target ? "in-box" : ""}`}
      onClick={() => (target?.current ?? window).scrollTo({ top: 0, behavior: "smooth" })}
      title={t("scroll.top")}
      aria-label={t("scroll.top")}
    >
      ↑
    </button>
  );
}
