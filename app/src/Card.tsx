import { motion } from "framer-motion";
import type { OpenedCard } from "./api";
import { CardImage } from "./CardImage";
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

export function Card({ card, faceUp, clickable, onReveal, lazy = false }: Props) {
  return (
    <div className="card-slot">
      <motion.div
        className={`card ${clickable ? "clickable" : ""}`}
        data-rarity={card.rarity}
        animate={{ rotateY: faceUp ? 180 : 0 }}
        transition={{ duration: 0.55, ease: "easeInOut" }}
        onClick={clickable ? onReveal : undefined}
      >
        <div className="face back">
          <span>U</span>
        </div>
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
