import { useEffect, useState } from "react";
import { sellCards, type OpenedCard, type SaleRequest } from "./api";
import { useI18n } from "./i18n";
import { useMoney } from "./money";
import { sounds } from "./sound";

/** Selling the cards of the booster just opened: one by one, or all those left at once. */
export interface BoosterSale {
  /** By card index: sold already. */
  sold: boolean[];
  sellOne: (index: number) => void;
  sellAll: () => void;
  /** What selling the cards left would bring in. */
  leftCents: number;
  /** What the cards sold brought in. */
  soldCents: number;
  busy: boolean;
  error: string | null;
}

/** The sale of the booster `cards` (reset with each new booster); `onSold` refreshes the wallet. */
export function useBoosterSale(cards: OpenedCard[] | null, onSold: () => void): BoosterSale {
  const [sold, setSold] = useState<boolean[]>([]);
  const [soldCents, setSoldCents] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setSold([]);
    setSoldCents(0);
    setError(null);
  }, [cards]);

  const sellable = (i: number) => cards != null && !sold[i] && cards[i].sellValueCents > 0;

  async function sell(indices: number[]) {
    if (!cards || busy || indices.length === 0) return;
    // Copies of the same card and finish go in one request.
    const requests = new Map<string, SaleRequest>();
    for (const i of indices) {
      const { uuid, foil } = cards[i];
      const key = `${uuid}:${foil}`;
      const request = requests.get(key) ?? { uuid, foil, quantity: 0 };
      request.quantity += 1;
      requests.set(key, request);
    }
    setBusy(true);
    try {
      const result = await sellCards([...requests.values()]);
      sounds.coins();
      setSold((s) => {
        const next = [...s];
        indices.forEach((i) => (next[i] = true));
        return next;
      });
      setSoldCents((n) => n + result.totalCents);
      setError(null);
      onSold();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const left = cards ? cards.map((_, i) => i).filter(sellable) : [];
  return {
    sold,
    sellOne: (i) => sellable(i) && void sell([i]),
    sellAll: () => void sell(left),
    leftCents: left.reduce((sum, i) => sum + cards![i].sellValueCents, 0),
    soldCents,
    busy,
    error,
  };
}

/** Sells every card of the booster still there, after a second click to confirm. */
export function SellBoosterButton({ sale }: { sale: BoosterSale }) {
  const { tx } = useI18n();
  const money = useMoney();
  const [confirming, setConfirming] = useState(false);
  if (sale.leftCents === 0) {
    return sale.soldCents > 0 ? <span className="hint">{tx("open.soldCards", { amount: money(sale.soldCents) })}</span> : null;
  }
  return (
    <>
      <button
        className={confirming ? "sell confirm" : "sell"}
        disabled={sale.busy}
        onClick={() => {
          if (!confirming) return setConfirming(true);
          setConfirming(false);
          sale.sellAll();
        }}
        onMouseLeave={() => setConfirming(false)}
      >
        {tx(confirming ? "open.sellBoosterConfirm" : "open.sellBooster", { amount: money(sale.leftCents) })}
      </button>
      {sale.error && <span className="error">{sale.error}</span>}
    </>
  );
}

/** Under a card of the booster: sells it, or says it was sold. */
export function SellCardButton({ sale, index, card }: { sale: BoosterSale; index: number; card: OpenedCard }) {
  const { t, tx } = useI18n();
  const money = useMoney();
  if (sale.sold[index]) return <span className="sold-tag">{t("open.cardSold")}</span>;
  if (card.sellValueCents === 0) return null;
  return (
    <button
      className="sell"
      disabled={sale.busy}
      onClick={(e) => {
        // The card itself opens its details.
        e.stopPropagation();
        sale.sellOne(index);
      }}
    >
      {tx("collection.sell", { amount: money(card.sellValueCents) })}
    </button>
  );
}
