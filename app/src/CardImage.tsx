import { useRef, useState, type CSSProperties, type ImgHTMLAttributes } from "react";
import { useI18n } from "./i18n";
import { useNearScreen } from "./nearScreen";

interface Props {
  front: string;
  /** Back face of a double-faced card: a button turns the card over. */
  back: string | null;
  alt: string;
  className?: string;
  lazy?: boolean;
}

/** The marker telling where a lazy image goes: never displayed, whatever the CSS around it. */
const HIDDEN: CSSProperties = { display: "none" };

/** An image of a long list (a set's art in the Boosterium…), only in the page near the screen
 * (see `CardImage`'s `lazy`): its parent keeps its place. */
export function NearImage(props: ImgHTMLAttributes<HTMLImageElement>) {
  const marker = useRef<HTMLSpanElement>(null);
  const near = useNearScreen(marker, true, true);
  return (
    <>
      <span ref={marker} style={HIDDEN} />
      {near && <img decoding="async" draggable={false} {...props} />}
    </>
  );
}

/** Where a turn is: the face shown going edge-on, then the other one coming round. */
type Turn = "idle" | "out" | "in";

/** A card image that double-faced cards can turn over. The image is never altered.
 * `lazy` (long lists): the image is only in the page while its place (the parent element,
 * which keeps the card's size) is on screen or close to it, so that its memory is given back
 * once scrolled far away. */
export function CardImage({ front, back, alt, className, lazy = false }: Props) {
  const { t } = useI18n();
  const [showBack, setShowBack] = useState(false);
  const [turn, setTurn] = useState<Turn>("idle");
  const marker = useRef<HTMLSpanElement>(null);
  const near = useNearScreen(marker, lazy, true);
  const turnClass = turn === "out" ? "turning-out" : turn === "in" ? "turning-in" : "";
  if (!near) return <span ref={marker} style={HIDDEN} />;
  return (
    <>
      {lazy && <span ref={marker} style={HIDDEN} />}
      <img
        className={`${className ?? ""} ${turnClass}`}
        src={showBack && back ? back : front}
        alt={alt}
        decoding="async"
        draggable={false}
        onAnimationEnd={() => {
          // Edge-on: the other face takes its place and finishes the turn.
          if (turn === "out") {
            setShowBack((b) => !b);
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
