import type { OpenedCard } from "./api";
import { isBigCard } from "./BatchOpening";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Money } from "./money";

const hitOf = (card: OpenedCard) =>
  card.rarity === "mythic" ? "hit-mythic" : isBigCard(card) ? "hit-rare" : card.foil ? "hit-foil" : "";

/** The cards of an opened booster, in a row, each with its price; a click shows its details.
 * `final`: the whole booster at once, the cards dealt one after the other. */
export function RevealedCards({ cards, final, onDetails }: { cards: OpenedCard[]; final: boolean; onDetails: (card: OpenedCard) => void }) {
  const { t } = useI18n();
  return (
    <div className={`revealed-row ${final ? "final" : ""}`}>
      {cards.map((card, index) => (
        <div
          key={index}
          role="button"
          tabIndex={0}
          className={`revealed-card ${hitOf(card)}`}
          style={{ "--i": index } as React.CSSProperties}
          onClick={() => onDetails(card)}
          onKeyDown={(e) => e.key === "Enter" && onDetails(card)}
          title={t("details.open")}
        >
          <div className="thumb">
            <CardImage front={card.imageUrl} back={card.backImageUrl} alt={card.name} />
            {card.foil && <div className="foil-sheen" />}
            {card.isNew && <span className="badge-new">{t("open.new")}</span>}
          </div>
          <span className="card-caption visible">
            <span className="name">{card.name}</span>
            <span className="price">
              {card.priceCents == null ? t("common.none") : <Money cents={card.priceCents} estimated={card.priceEstimated} />}
            </span>
          </span>
        </div>
      ))}
    </div>
  );
}
