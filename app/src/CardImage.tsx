import { useState } from "react";
import { useI18n } from "./i18n";

interface Props {
  front: string;
  /** Back face of a double-faced card: a button turns the card over. */
  back: string | null;
  alt: string;
  className?: string;
  lazy?: boolean;
}

/** Where a turn is: the face shown going edge-on, then the other one coming round. */
type Turn = "idle" | "out" | "in";

/** A card image that double-faced cards can turn over. The image is never altered. */
export function CardImage({ front, back, alt, className, lazy }: Props) {
  const { t } = useI18n();
  const [showBack, setShowBack] = useState(false);
  const [turn, setTurn] = useState<Turn>("idle");
  const turnClass = turn === "out" ? "turning-out" : turn === "in" ? "turning-in" : "";
  return (
    <>
      <img
        className={`${className ?? ""} ${turnClass}`}
        src={showBack && back ? back : front}
        alt={alt}
        loading={lazy ? "lazy" : undefined}
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
