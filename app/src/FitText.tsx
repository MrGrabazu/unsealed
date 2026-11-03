import { useLayoutEffect, useRef, type ReactNode } from "react";

/** Text whose box is as wide as its longest line. Wrapped on several lines, a text's box takes
 * all the width it is given, whatever its lines (CSS has no way to shrink it to them): beside
 * an icon, in a centred title, the icon went to the left with a gap before the text. */
export function FitText({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const span = ref.current;
    const parent = span?.parentElement;
    if (!span || !parent) return;
    const fit = () => {
      span.style.width = "";
      const range = document.createRange();
      range.selectNodeContents(span);
      const widest = Math.max(0, ...Array.from(range.getClientRects(), (r) => r.width));
      if (widest > 0 && widest < span.getBoundingClientRect().width) span.style.width = `${Math.ceil(widest)}px`;
    };
    fit();
    // Measured again once the title font is there (its lines are not the fallback font's).
    void document.fonts.ready.then(fit);
    const observer = new ResizeObserver(fit);
    observer.observe(parent);
    return () => observer.disconnect();
  }, [children]);
  return <span ref={ref}>{children}</span>;
}
