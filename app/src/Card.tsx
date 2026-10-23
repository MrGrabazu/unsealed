import { motion } from "framer-motion";
import type { OpenedCard } from "./api";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Money } from "./money";
import { FoilSheen } from "./foils";

interface Props {
  card: OpenedCard;
  faceUp: boolean;
  /** Whether a click on this card reveals it. */
  clickable: boolean;
  onReveal: () => void;
}

export function Card({ card, faceUp, clickable, onReveal }: Props) {
  const { t } = useI18n();
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
          <CardImage front={card.imageUrl} back={faceUp ? card.backImageUrl : null} alt={card.name} />
          {card.foil && <FoilSheen rarity={card.rarity} kind={card.foilKind} />}
          {card.isNew && <span className="badge-new">{t("open.new")}</span>}
        </div>
      </motion.div>
      <div className={`card-caption ${faceUp ? "visible" : ""}`}>
        <span className="name">{card.name}</span>
        <span className="price">{card.priceCents == null ? "—" : <Money cents={card.priceCents} estimated={card.priceEstimated} />}</span>
      </div>
    </div>
  );
}
