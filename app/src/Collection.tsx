import { useCallback, useEffect, useState } from "react";
import {
  getCollection,
  sellCards,
  sellDuplicates,
  type CollectionCard,
  type CollectionView,
  type SetSummary,
} from "./api";
import { CardDetails } from "./CardDetails";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { useMoney, useMoneyText } from "./money";

type Filter = "all" | "owned" | "missing";

interface Props {
  sets: SetSummary[];
  /** Changes whenever the collection may have changed elsewhere (a booster was opened). */
  refreshKey: number;
  /** Called after a sale so the wallet can refresh. */
  onChanged: () => void;
}

/** Collection of one set: completion progress, missing cards greyed out, selling. */
export function Collection({ sets, refreshKey, onChanged }: Props) {
  const money = useMoney();
  const moneyText = useMoneyText();
  const { t, tx, plural } = useI18n();
  const [setCode, setSetCode] = useState("");
  const [view, setView] = useState<CollectionView | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // "uuid:foil" of a last copy waiting for a second click to confirm the sacrifice.
  const [pending, setPending] = useState<string | null>(null);
  // The card whose details are open.
  const [detailsOf, setDetailsOf] = useState<string | null>(null);

  useEffect(() => {
    if (!setCode && sets.length > 0) setSetCode(sets[0].code);
  }, [sets, setCode]);

  const reload = useCallback(async () => {
    if (!setCode) return;
    try {
      setView(await getCollection(setCode));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [setCode]);

  useEffect(() => {
    void reload();
  }, [reload, refreshKey]);

  async function sell(card: CollectionCard, foil: boolean) {
    const key = `${card.uuid}:${foil}`;
    const lastCopy = card.quantity + card.foilQuantity === 1;
    // Selling the last copy removes the card from the collection: ask twice.
    if (lastCopy && pending !== key) {
      setPending(key);
      return;
    }
    setPending(null);
    try {
      const result = await sellCards([{ uuid: card.uuid, foil, quantity: 1 }]);
      setNotice(t("collection.sold", { card: card.name, amount: moneyText(result.totalCents) }));
      await reload();
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  async function sellExtras() {
    try {
      const result = await sellDuplicates(setCode);
      setNotice(
        result.soldCount === 0
          ? t("collection.noDuplicates")
          : plural("collection.soldDuplicates", result.soldCount, { amount: moneyText(result.totalCents) }),
      );
      await reload();
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  const matches = (c: CollectionCard) => {
    const owned = c.quantity + c.foilQuantity > 0;
    return filter === "all" || (filter === "owned") === owned;
  };
  const shown = view?.cards.filter(matches);
  const shownExtras = view?.extras.filter(matches);

  function cardTile(c: CollectionCard) {
    const total = c.quantity + c.foilQuantity;
    return (
      <div key={c.uuid} className={`collection-card ${total > 0 ? "" : "missing"}`} data-rarity={c.rarity}>
        <div
          className="thumb clickable-thumb"
          role="button"
          tabIndex={0}
          title={t("details.open")}
          onClick={() => setDetailsOf(c.uuid)}
          onKeyDown={(e) => e.key === "Enter" && setDetailsOf(c.uuid)}
        >
          <CardImage front={c.imageUrl} back={c.backImageUrl} alt={c.name} lazy />
          {total > 1 && <span className="badge-qty">×{total}</span>}
          {c.foilQuantity > 0 && <span className="badge-foil">{t("collection.foilBadge")}</span>}
        </div>
        <div className="card-caption visible">
          <span className="name">
            #{c.number} {c.name}
          </span>
          <span className="price">{c.priceCents == null ? "—" : money(c.priceCents)}</span>
        </div>
        <div className="sell-row">
          {sellButton(c, false)}
          {sellButton(c, true)}
        </div>
      </div>
    );
  }

  function sellButton(card: CollectionCard, foil: boolean) {
    const value = foil ? card.foilSellValueCents : card.sellValueCents;
    const count = foil ? card.foilQuantity : card.quantity;
    if (count === 0 || value === 0) return null;
    const key = `${card.uuid}:${foil}`;
    const confirming = pending === key;
    return (
      <button
        key={key}
        className={`sell ${confirming ? "confirm" : ""}`}
        onClick={() => void sell(card, foil)}
        onMouseLeave={() => confirming && setPending(null)}
      >
        {confirming
          ? tx("collection.sacrifice", { amount: money(value) })
          : tx(foil ? "collection.sellFoil" : "collection.sell", { amount: money(value) })}
      </button>
    );
  }

  return (
    <section>
      <div className="controls">
        <select value={setCode} onChange={(e) => setSetCode(e.target.value)}>
          {sets.map((s) => (
            <option key={s.code} value={s.code}>
              {s.name} ({s.code})
            </option>
          ))}
        </select>
        <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
          <option value="all">{t("collection.allCards")}</option>
          <option value="owned">{t("collection.owned")}</option>
          <option value="missing">{t("collection.missing")}</option>
        </select>
        <button onClick={() => void sellExtras()}>{t("collection.sellDuplicates")}</button>
      </div>

      {error && <p className="error">{error}</p>}
      {notice && <p className="notice">{notice}</p>}
      {!view && !error && <Loading label={t("collection.loading")} />}
      {view && (
        <>
          <div className="progress">
            <div className="progress-bar" style={{ width: `${view.completionPercent}%` }} />
          </div>
          <p className="progress-label">
            {t("collection.progress", {
              owned: view.ownedCount,
              total: view.totalCount,
              percent: view.completionPercent.toFixed(1),
            })}
          </p>
          <div className="collection-grid">{shown?.map(cardTile)}</div>
          {view.extras.length > 0 && (
            <>
              <h3>{t("collection.extras")}</h3>
              <p className="progress-label">
                {t("collection.extrasProgress", { owned: view.extrasOwned, total: view.extras.length })}
              </p>
              <div className="collection-grid">{shownExtras?.map(cardTile)}</div>
            </>
          )}
        </>
      )}
      {view && detailsOf && (
        <CardDetails setCode={view.setCode} uuid={detailsOf} onClose={() => setDetailsOf(null)} />
      )}
    </section>
  );
}
