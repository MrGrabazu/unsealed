import { useEffect, useState } from "react";
import { getDuplicatesValue, sellCards, sellDuplicates, type OpenedCard, type SaleRequest } from "./api";
import { useI18n } from "./i18n";
import { useMoney } from "./money";
import { useSaleNotice } from "./notice";
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
  /** Copies sold elsewhere (the set's duplicates): as many of the booster's are marked sold,
   * so that selling what is left never asks for more copies than are owned. */
  markSold: (sold: SaleRequest[]) => void;
}

/** The sale of the booster `cards` (reset with each new booster); `onSold` refreshes the wallet. */
export function useBoosterSale(cards: OpenedCard[] | null, onSold: () => void): BoosterSale {
  const [sold, setSold] = useState<boolean[]>([]);
  const [soldCents, setSoldCents] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const saleNotice = useSaleNotice();

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
      saleNotice(result.soldCount, result.totalCents, indices.length === 1 ? cards[indices[0]].name : undefined);
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
    markSold: (requests) => {
      if (!cards) return;
      const left = new Map(requests.map((r) => [`${r.uuid}:${r.foil}`, r.quantity]));
      setSold((s) => {
        const next = [...s];
        cards.forEach((c, i) => {
          const key = `${c.uuid}:${c.foil}`;
          const n = left.get(key) ?? 0;
          if (next[i] || n === 0) return;
          next[i] = true;
          left.set(key, n - 1);
        });
        return next;
      });
    },
  };
}

/** Sells the duplicates of the opened boosters' set (one copy of each card kept, as in its
 * binder), after a second click to confirm. `opened`: what was opened (a new opening resets
 * the button); `sale`: the sale of the booster's own cards, if there is one; `large`: the
 * tally's main sale, in the room of two buttons. */
export function SellDuplicatesButton(props: { setCode: string; opened: unknown; sale?: BoosterSale; onSold: () => void; large?: boolean }) {
  const { setCode, opened, sale, onSold, large = false } = props;
  const { tx } = useI18n();
  const money = useMoney();
  const [value, setValue] = useState<number | null>(null);
  const [soldCents, setSoldCents] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const saleNotice = useSaleNotice();

  useEffect(() => setSoldCents(null), [opened]);

  // Read again after each sale of the booster's own cards.
  useEffect(() => {
    let live = true;
    getDuplicatesValue(setCode)
      .then((v) => live && setValue(v.totalCents))
      .catch(() => live && setValue(null));
    return () => {
      live = false;
    };
  }, [setCode, opened, sale?.sold, soldCents]);

  if (soldCents != null && !value) return <span className="hint">{tx("open.soldDuplicates", { amount: money(soldCents) })}</span>;
  if (!value) return null;

  async function sell() {
    setBusy(true);
    try {
      const result = await sellDuplicates(setCode);
      sounds.coins();
      saleNotice(result.soldCount, result.totalCents);
      sale?.markSold(result.sold);
      setSoldCents((n) => (n ?? 0) + result.totalCents);
      setError(null);
      onSold();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <button
        className={`sell${confirming ? " confirm" : ""}${large ? " large" : ""}`}
        disabled={busy || sale?.busy}
        onClick={() => {
          if (!confirming) return setConfirming(true);
          setConfirming(false);
          void sell();
        }}
        onMouseLeave={() => setConfirming(false)}
      >
        {tx(confirming ? "open.sellBoosterConfirm" : "open.sellDuplicates", { amount: money(value) })}
      </button>
      {error && <span className="error">{error}</span>}
    </>
  );
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
