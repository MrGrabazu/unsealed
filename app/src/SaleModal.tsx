import { useEffect } from "react";
import type { CollectionCard } from "./api";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Money } from "./money";

/** One line of a sale: copies of a card in one finish. */
export interface SoldLine {
  card: CollectionCard;
  foil: boolean;
  quantity: number;
  unitCents: number;
}

/** The copies "sell duplicates" sells: every copy beyond the first of each card and finish
 * the Boosterium buys (as the game does it, see `Game::sell_duplicates`). */
export function duplicatesOf(cards: CollectionCard[]): SoldLine[] {
  const lines: SoldLine[] = [];
  for (const card of cards) {
    if (card.quantity > 1 && card.sellValueCents > 0) {
      lines.push({ card, foil: false, quantity: card.quantity - 1, unitCents: card.sellValueCents });
    }
    if (card.foilQuantity > 1 && card.foilSellValueCents > 0) {
      lines.push({ card, foil: true, quantity: card.foilQuantity - 1, unitCents: card.foilSellValueCents });
    }
  }
  return lines.sort((a, b) => b.quantity * b.unitCents - a.quantity * a.unitCents);
}

interface Props {
  soldCount: number;
  totalCents: number;
  lines: SoldLine[];
  onClose: () => void;
}

/** What a sale brought in, in plain view: the total, then each card sold. */
export function SaleModal({ soldCount, totalCents, lines, onClose }: Props) {
  const { t, plural } = useI18n();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => (e.key === "Escape" || e.key === "Enter") && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal sale-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h2>{t("sale.title")}</h2>
        <p className="sale-total">
          <Money cents={totalCents} />
        </p>
        <p className="hint sale-count">{plural("sale.count", soldCount)}</p>
        {lines.length > 0 && (
          <ul className="sale-lines">
            {lines.map((line) => (
              <li key={`${line.card.uuid}:${line.foil}`}>
                <div className="sale-thumb thumb">
                  <CardImage front={line.card.imageUrl} back={null} alt={line.card.name} lazy />
                  {line.foil && <div className="foil-sheen" />}
                </div>
                <span className="sale-name">
                  {line.card.name}
                  {line.foil && <span className="tag">{t("collection.foilBadge")}</span>}
                </span>
                <span className="hint">
                  ×{line.quantity} · <Money cents={line.unitCents} />
                </span>
                <strong>
                  <Money cents={line.quantity * line.unitCents} />
                </strong>
              </li>
            ))}
          </ul>
        )}
        <button className="primary" onClick={onClose} autoFocus>
          {t("common.ok")}
        </button>
      </div>
    </div>
  );
}
