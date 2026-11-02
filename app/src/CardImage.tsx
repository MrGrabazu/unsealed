import { useLayoutEffect, useRef, useState, type CSSProperties, type ImgHTMLAttributes, type RefObject } from "react";
import { useI18n } from "./i18n";
import { fittedImage } from "./images";
import { useNearScreen } from "./nearScreen";

/** `url`, or its thumbnail when that is enough for the box the image is in (the parent of
 * `marker`, measured before the first paint, and again when it grows: see `fittedImage`).
 * Undefined until measured, so that no image is fetched before the right one is known. */
function useFitted(url: string | null | undefined, marker: RefObject<HTMLElement | null>): string | undefined {
  const [box, setBox] = useState<[number, number] | null>(null);
  useLayoutEffect(() => {
    const parent = marker.current?.parentElement;
    if (!parent) return setBox([Infinity, Infinity]);
    // Only ever larger: a box that shrinks keeps the image it has.
    const measure = () =>
      setBox((b) => {
        const [w, h] = [parent.clientWidth, parent.clientHeight];
        return b && b[0] >= w && b[1] >= h ? b : [Math.max(w, b?.[0] ?? 0), Math.max(h, b?.[1] ?? 0)];
      });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(parent);
    return () => observer.disconnect();
  }, [marker]);
  if (url == null || box == null) return undefined;
  return fittedImage(url, box[0], box[1]);
}

interface Props {
  front: string;
  /** Back face of a double-faced card: a button turns the card over. */
  back: string | null;
  alt: string;
  className?: string;
  lazy?: boolean;
  /** Told the face shown after each turn over (true: the back). */
  onTurned?: (back: boolean) => void;
}

/** The marker telling where a lazy image goes: never displayed, whatever the CSS around it. */
const HIDDEN: CSSProperties = { display: "none" };

/** An image from the cache, as its thumbnail when that is enough for its box (its parent,
 * which keeps its size: see `fittedImage`). */
export function FittedImage(props: ImgHTMLAttributes<HTMLImageElement>) {
  const marker = useRef<HTMLSpanElement>(null);
  const src = useFitted(props.src, marker);
  return (
    <>
      <span ref={marker} style={HIDDEN} />
      {src && <img decoding="async" draggable={false} {...props} src={src} />}
    </>
  );
}

/** An image of a long list (a set's art in the Boosterium…), only in the page near the screen
 * (see `CardImage`'s `lazy`): its parent keeps its place. */
export function NearImage(props: ImgHTMLAttributes<HTMLImageElement>) {
  const marker = useRef<HTMLSpanElement>(null);
  const near = useNearScreen(marker, true, true);
  const src = useFitted(props.src, marker);
  return (
    <>
      <span ref={marker} style={HIDDEN} />
      {near && src && <img decoding="async" draggable={false} {...props} src={src} />}
    </>
  );
}

/** Where a turn is: the face shown going edge-on, then the other one coming round. */
type Turn = "idle" | "out" | "in";

/** A card image that double-faced cards can turn over. The image is never altered.
 * `lazy` (long lists): the image is only in the page while its place (the parent element,
 * which keeps the card's size) is on screen or close to it, so that its memory is given back
 * once scrolled far away. */
export function CardImage({ front, back, alt, className, lazy = false, onTurned }: Props) {
  const { t } = useI18n();
  const [showBack, setShowBack] = useState(false);
  const [turn, setTurn] = useState<Turn>("idle");
  const marker = useRef<HTMLSpanElement>(null);
  const near = useNearScreen(marker, lazy, true);
  const src = useFitted(showBack && back ? back : front, marker);
  const turnClass = turn === "out" ? "turning-out" : turn === "in" ? "turning-in" : "";
  if (!near) return <span ref={marker} style={HIDDEN} />;
  return (
    <>
      <span ref={marker} style={HIDDEN} />
      <img
        className={`${className ?? ""} ${turnClass}`}
        src={src}
        alt={alt}
        decoding="async"
        draggable={false}
        onAnimationEnd={() => {
          // Edge-on: the other face takes its place and finishes the turn.
          if (turn === "out") {
            setShowBack(!showBack);
            onTurned?.(!showBack);
            setTurn("in");
          } else if (turn === "in") {
            setTurn("idle");
          }
        }}
      />
      {back && (
        <button
          type="button"
          className="flip-button"
          title={t("card.flip")}
          aria-label={t("card.flip")}
          onClick={(e) => {
            // The card itself may be clickable (reveal, details): turning it over is not a click on it.
            e.stopPropagation();
            if (turn === "idle") setTurn("out");
          }}
        >
          ⟲
        </button>
      )}
    </>
  );
}
