import { useEffect, useRef } from "react";
import type { CollectionCard } from "./api";
import { BackToTop } from "./BackToTop";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Money } from "./money";
import { sheenClass } from "./RevealedCards";

/** One line of a sale: copies of a card in one finish. */
export interface SoldLine {
  card: CollectionCard;
  foil: boolean;
  quantity: number;
  unitCents: number;
}

/** What is left to sell of a card when one copy is kept, whatever its finish: the foil one
 * if there is one (as `Game::sell_duplicates` does). */
export function beyondOneCopy(card: CollectionCard): { normal: number; foil: number } {
  const keptFoil = card.foilQuantity > 0 ? 1 : 0;
  return { normal: Math.max(0, card.quantity - (1 - keptFoil)), foil: card.foilQuantity - keptFoil };
}

/** The copies "sell duplicates" sells: every copy beyond the one kept of each card, that
 * the Boosterium buys (as the game does it, see `Game::sell_duplicates`). */
export function duplicatesOf(cards: CollectionCard[]): SoldLine[] {
  const lines: SoldLine[] = [];
  for (const card of cards) {
    const { normal, foil } = beyondOneCopy(card);
    if (normal > 0 && card.sellValueCents > 0) lines.push({ card, foil: false, quantity: normal, unitCents: card.sellValueCents });
    if (foil > 0 && card.foilSellValueCents > 0) lines.push({ card, foil: true, quantity: foil, unitCents: card.foilSellValueCents });
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
  const box = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => (e.key === "Escape" || e.key === "Enter") && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div ref={box} className="modal sale-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h2>{t("sale.title")}</h2>
        <p className="sale-total">
          <Money cents={totalCents} />
        </p>
        <p className="hint sale-count">{plural("sale.count", soldCount)}</p>
        {lines.length > 0 && (
          <ul ref={list} className="sale-lines">
            {lines.map((line) => (
              <li key={`${line.card.uuid}:${line.foil}`}>
                <div className="sale-thumb thumb">
                  <CardImage front={line.card.imageUrl} back={null} alt={line.card.name} lazy />
                  {line.foil && <div className={sheenClass(line.card.rarity)} />}
                </div>
                <span className="sale-name">
                  {line.card.name}
                  {line.foil && (
                    <span className="tag">{t(line.card.rarity === "art" ? "collection.signedBadge" : "collection.foilBadge")}</span>
                  )}
                </span>
                <span className="hint">
                  ×{line.quantity} · <Money cents={line.unitCents} />
                </span>
                <strong>
                  <Money cents={line.quantity * line.unitCents} />
                </strong>
              </li>
            ))}
            <li className="back-to-top-row">
              <BackToTop target={list} />
            </li>
          </ul>
        )}
        <button className="primary" onClick={onClose} autoFocus>
          {t("common.ok")}
        </button>
        <BackToTop target={box} />
      </div>
    </div>
  );
}
