import { motion } from "framer-motion";
import { useEffect, useState } from "react";
import type { OpenedCard } from "./api";
import { cardBackUrl } from "./canvasArt";
import { CardImage } from "./CardImage";
import { cornersClass } from "./corners";
import { Money } from "./money";
import { FoilSheen } from "./foils";
import { NewBadge } from "./NewBadge";

interface Props {
  card: OpenedCard;
  faceUp: boolean;
  /** Whether a click on this card reveals it. */
  clickable: boolean;
  onReveal: () => void;
  /** In a long list: its image only in the page near the screen. */
  lazy?: boolean;
}

/** A card already face up that will not turn: the same, without the 3D faces nor the
 * animation (in a batch's list, hundreds of them: each 3D card took compositor layers of its
 * own). */
export function FlatCard({ card, onClick, lazy = true }: { card: OpenedCard; onClick: () => void; lazy?: boolean }) {
  return (
    <div className="card-slot">
      <div className={`card flat clickable ${cornersClass(card.setCode)}`} data-rarity={card.rarity} onClick={onClick}>
        <div className="face front">
          <CardImage front={card.imageUrl} back={card.backImageUrl} alt={card.name} lazy={lazy} />
          {card.foil && <FoilSheen rarity={card.rarity} kind={card.foilKind} />}
          <NewBadge card={card} />
        </div>
      </div>
      <div className="card-caption visible">
        <span className="name">{card.name}</span>
        <span className="price">{card.priceCents == null ? "—" : <Money cents={card.priceCents} estimated={card.priceEstimated} />}</span>
      </div>
    </div>
  );
}

/** The cards' back (drawn once, see `canvasArt.ts`), as an image URL once ready. */
export function useCardBack(): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void cardBackUrl().then((u) => live && setUrl(u));
    return () => {
      live = false;
    };
  }, []);
  return url;
}

export function Card({ card, faceUp, clickable, onReveal, lazy = false }: Props) {
  const backUrl = useCardBack();
  return (
    <div className="card-slot">
      <motion.div
        className={`card ${clickable ? "clickable" : ""} ${cornersClass(card.setCode)}`}
        data-rarity={card.rarity}
        animate={{ rotateY: faceUp ? 180 : 0 }}
        transition={{ duration: 0.55, ease: "easeInOut" }}
        onClick={clickable ? onReveal : undefined}
      >
        <div className="face back" style={backUrl ? { backgroundImage: `url(${backUrl})` } : undefined} />
        <div className="face front">
          {/* Only a revealed card can be turned over: before that, a click reveals it. */}
          <CardImage front={card.imageUrl} back={faceUp ? card.backImageUrl : null} alt={card.name} lazy={lazy} />
          {card.foil && <FoilSheen rarity={card.rarity} kind={card.foilKind} />}
          <NewBadge card={card} />
        </div>
      </motion.div>
      <div className={`card-caption ${faceUp ? "visible" : ""}`}>
        <span className="name">{card.name}</span>
        <span className="price">{card.priceCents == null ? "—" : <Money cents={card.priceCents} estimated={card.priceEstimated} />}</span>
      </div>
    </div>
  );
}
