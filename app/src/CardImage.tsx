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

/** A card image that double-faced cards can turn over. The image is never altered. */
export function CardImage({ front, back, alt, className, lazy }: Props) {
  const { t } = useI18n();
  const [showBack, setShowBack] = useState(false);
  return (
    <>
      <img
        // A new key restarts the turn animation on each flip.
        key={showBack ? "back" : "front"}
        className={`${className ?? ""} ${back ? "turnable" : ""}`}
        src={showBack && back ? back : front}
        alt={alt}
        loading={lazy ? "lazy" : undefined}
        draggable={false}
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
            setShowBack((b) => !b);
          }}
        >
          ⟲
        </button>
      )}
    </>
  );
}
