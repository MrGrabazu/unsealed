import { motion } from "framer-motion";
import type { OpenedCard } from "./api";
import { useMoney } from "./money";

interface Props {
  card: OpenedCard;
  faceUp: boolean;
  /** Whether a click on this card reveals it. */
  clickable: boolean;
  onReveal: () => void;
}

export function Card({ card, faceUp, clickable, onReveal }: Props) {
  const money = useMoney();
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
          <img src={card.imageUrl} alt={card.name} draggable={false} />
          {card.foil && <div className="foil-sheen" />}
          {card.isNew && <span className="badge-new">NEW</span>}
        </div>
      </motion.div>
      <div className={`card-caption ${faceUp ? "visible" : ""}`}>
        <span className="name">{card.name}</span>
        <span className="price">{card.priceCents == null ? "—" : money(card.priceCents)}</span>
      </div>
    </div>
  );
}
