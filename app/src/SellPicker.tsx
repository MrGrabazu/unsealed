import { useEffect, useMemo, useRef, useState } from "react";
import type { CollectionCard } from "./api";
import { BackToTop } from "./BackToTop";
import { useI18n } from "./i18n";
import type { MessageKey } from "./i18n/en";
import { Money } from "./money";
import { beyondOneCopy, type SoldLine } from "./SaleModal";

/** What can be sold at once: every card, or those of one rarity (tokens and art cards apart). */
const GROUPS: { key: string; label: MessageKey; rarities: string[] | null }[] = [
  { key: "all", label: "sell.all", rarities: null },
  { key: "common", label: "sell.common", rarities: ["common"] },
  { key: "uncommon", label: "sell.uncommon", rarities: ["uncommon"] },
  { key: "rare", label: "sell.rare", rarities: ["rare"] },
  { key: "mythic", label: "sell.mythic", rarities: ["mythic"] },
  { key: "other", label: "sell.other", rarities: ["special", "bonus"] },
  { key: "token", label: "collection.tokens", rarities: ["token"] },
  { key: "art", label: "collection.arts", rarities: ["art"] },
];

/** The copies a group sells: every copy the Boosterium buys, or all but one of each card
 * (the foil one kept if there is one, as for duplicates). */
function linesOf(cards: CollectionCard[], rarities: string[] | null, keepOne: boolean): SoldLine[] {
  const lines: SoldLine[] = [];
  for (const card of cards) {
    if (rarities && !rarities.includes(card.rarity)) continue;
    const { normal, foil } = keepOne ? beyondOneCopy(card) : { normal: card.quantity, foil: card.foilQuantity };
    if (normal > 0 && card.sellValueCents > 0) lines.push({ card, foil: false, quantity: normal, unitCents: card.sellValueCents });
    if (foil > 0 && card.foilSellValueCents > 0) lines.push({ card, foil: true, quantity: foil, unitCents: card.foilSellValueCents });
  }
  return lines.sort((a, b) => b.quantity * b.unitCents - a.quantity * a.unitCents);
}

interface Props {
  /** Every card of the collection shown (its sections together). */
  cards: CollectionCard[];
  onSell: (lines: SoldLine[]) => Promise<void>;
  onClose: () => void;
}

/** Selling in bulk: every card, or one rarity, keeping one copy of each card or not. Each
 * choice says what it sells and for how much, and asks a second click to confirm. */
export function SellPicker({ cards, onSell, onClose }: Props) {
  const { t, plural } = useI18n();
  const [keepOne, setKeepOne] = useState(true);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const groups = useMemo(
    () =>
      GROUPS.map((g) => {
        const lines = linesOf(cards, g.rarities, keepOne);
        return {
          ...g,
          lines,
          copies: lines.reduce((n, l) => n + l.quantity, 0),
          totalCents: lines.reduce((n, l) => n + l.quantity * l.unitCents, 0),
        };
      }).filter((g) => g.key === "all" || g.copies > 0),
    [cards, keepOne],
  );

  async function sell(key: string, lines: SoldLine[]) {
    if (confirming !== key) {
      setConfirming(key);
      return;
    }
    setBusy(true);
    try {
      await onSell(lines);
    } finally {
      setBusy(false);
      setConfirming(null);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div ref={box} className="modal sell-picker" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h2>{t("sell.title")}</h2>
        <p className="hint">{t("sell.intro")}</p>
        <label className="sell-keep">
          <input
            type="checkbox"
            checked={keepOne}
            onChange={(e) => {
              setKeepOne(e.target.checked);
              setConfirming(null);
            }}
          />
          {t("sell.keepOne")}
        </label>
        <ul className="sell-groups">
          {groups.map((g) => (
            <li key={g.key}>
              <span className="sell-group-name">{t(g.label)}</span>
              <span className="hint">{plural("sell.copies", g.copies)}</span>
              <strong>
                <Money cents={g.totalCents} />
              </strong>
              <button
                className={confirming === g.key ? "sell confirm" : "sell"}
                disabled={busy || g.copies === 0}
                onClick={() => void sell(g.key, g.lines)}
                onMouseLeave={() => confirming === g.key && setConfirming(null)}
              >
                {confirming === g.key ? t("sell.confirm") : t("sell.sell")}
              </button>
            </li>
          ))}
        </ul>
        <button onClick={onClose}>{t("common.cancel")}</button>
        <BackToTop target={box} />
      </div>
    </div>
  );
}
