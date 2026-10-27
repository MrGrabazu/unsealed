import type { OpenedCard } from "./api";
import { isBigCard } from "./BatchOpening";
import { SellCardButton, type BoosterSale } from "./BoosterSale";
import { CardImage } from "./CardImage";
import { FoilSheen } from "./foils";
import { useI18n } from "./i18n";
import { Money } from "./money";
import { NewBadge } from "./NewBadge";

const hitOf = (card: OpenedCard) =>
  card.rarity === "mythic" ? "hit-mythic" : isBigCard(card) ? "hit-rare" : card.foil ? "hit-foil" : "";

/** The cards of an opened booster, in a row, each with its price; a click shows its details.
 * `final`: the whole booster at once, the cards dealt one after the other. With `sale`, each
 * card can be sold from here. */
export function RevealedCards({
  cards,
  final,
  onDetails,
  sale,
}: {
  cards: OpenedCard[];
  final: boolean;
  onDetails: (card: OpenedCard) => void;
  sale?: BoosterSale;
}) {
  const { t } = useI18n();
  return (
    <div className={`revealed-row ${final ? "final" : ""}`}>
      {cards.map((card, index) => (
        <div
          key={index}
          role="button"
          tabIndex={0}
          className={`revealed-card ${hitOf(card)} ${sale?.sold[index] ? "sold" : ""}`}
          style={{ "--i": index } as React.CSSProperties}
          onClick={() => onDetails(card)}
          onKeyDown={(e) => e.key === "Enter" && onDetails(card)}
          title={t("details.open")}
        >
          <div className="thumb">
            <CardImage front={card.imageUrl} back={card.backImageUrl} alt={card.name} />
            {card.foil && <FoilSheen rarity={card.rarity} kind={card.foilKind} />}
            <NewBadge card={card} />
          </div>
          <span className="card-caption visible">
            <span className="name">{card.name}</span>
            <span className="price">
              {card.priceCents == null ? t("common.none") : <Money cents={card.priceCents} estimated={card.priceEstimated} />}
            </span>
          </span>
          {sale && <SellCardButton sale={sale} index={index} card={card} />}
        </div>
      ))}
    </div>
  );
}
