import { useEffect, useState } from "react";
import type { OpenedCard } from "./api";
import { foilLabel } from "./foils";
import { useI18n } from "./i18n";

/** On a card just opened: "New" for a card never owned, "New foil" for the foil of a card
 * only owned in non-foil until now. A special foil (galaxy, surge…) is a printing of its own:
 * never owned, it is new. */
export function NewBadge({ card }: { card: OpenedCard }) {
  const { t } = useI18n();
  if (card.isNew) return <span className="badge-new">{t("open.new")}</span>;
  if (card.newFoil) return <span className="badge-new new-foil">{t("open.newFoil")}</span>;
  return null;
}

/** Said as a card is revealed, if it is new to the collection: over the 3D scene or the pile.
 * `card` is the card just shown (null between two); keyed by `index`, so that each card comes
 * with its own mention. */
export function RevealMention({ card, index }: { card: OpenedCard | null; index: number }) {
  const { t } = useI18n();
  // Gone a few seconds after the card shows: a mention, not a label.
  const [shown, setShown] = useState(true);
  useEffect(() => {
    setShown(true);
    const timer = setTimeout(() => setShown(false), 2600);
    return () => clearTimeout(timer);
  }, [card, index]);
  if (!card || !shown || !(card.isNew || card.newFoil)) return null;
  const special = card.foil ? foilLabel(card.foilKind) : null;
  const text = card.isNew ? t("open.mentionNew") : t("open.mentionNewFoil");
  return (
    <div key={index} className={`reveal-mention${card.isNew ? "" : " new-foil"}`} role="status">
      <span className="star">✦</span> {text}
      {card.isNew && special && <em> · {special}</em>}
    </div>
  );
}
