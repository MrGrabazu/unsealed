import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getCollection,
  sellCards,
  sellDuplicates,
  type CollectionCard,
  type CollectionView,
  type SetSummary,
} from "./api";
import { Binder, type BinderSection } from "./Binder";
import { CardDetails } from "./CardDetails";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { duplicatesOf, SaleModal, type SoldLine } from "./SaleModal";
import { Money, useMoney, useMoneyText } from "./money";
import { getCollectionLayout, setCollectionLayout, type CollectionLayout } from "./settings";
import { sounds } from "./sound";

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
  const { t, tx } = useI18n();
  const [setCode, setSetCode] = useState("");
  const [view, setView] = useState<CollectionView | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // "uuid:foil" of a last copy waiting for a second click to confirm the sacrifice.
  const [pending, setPending] = useState<string | null>(null);
  // The card whose details are open.
  const [detailsOf, setDetailsOf] = useState<string | null>(null);
  const [layout, setLayout] = useState<CollectionLayout>(getCollectionLayout);
  // The last bulk sale, shown in its own window.
  const [sale, setSale] = useState<{ soldCount: number; totalCents: number; lines: SoldLine[] } | null>(null);
  // The binder's section on show (index in `binderSections`).
  const [binderSection, setBinderSection] = useState(0);

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
      sounds.coins();
      setNotice(t("collection.sold", { card: card.name, amount: moneyText(result.totalCents) }));
      await reload();
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  async function sellExtras() {
    try {
      // What is about to go, read before the collection changes (shown in the sale's window).
      const lines = view ? duplicatesOf([...view.cards, ...view.extras, ...view.tokens]) : [];
      const result = await sellDuplicates(setCode);
      if (result.soldCount > 0) {
        sounds.coins();
        setNotice(null);
        setSale({ soldCount: result.soldCount, totalCents: result.totalCents, lines });
      } else {
        setNotice(t("collection.noDuplicates"));
      }
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
  const shownTokens = view?.tokens.filter(matches);

  // The binder keeps every card in its place: no filter, sections one after the other.
  const binderSections = useMemo<BinderSection[]>(
    () =>
      view
        ? [
            { title: null, cards: view.cards },
            { title: t("collection.extras"), cards: view.extras },
            { title: t("collection.tokens"), cards: view.tokens },
          ].filter((s) => s.cards.length > 0)
        : [],
    [view, t],
  );

  // In the binder, the completion shown is that of the section open (cards, extras, tokens):
  // tokens and extras are collected too, and the main bar would say nothing of them.
  const section = layout === "binder" ? binderSections[Math.min(binderSection, binderSections.length - 1)] : undefined;
  const progress =
    section && section.title !== null
      ? (() => {
          const owned = section.cards.filter((c) => c.quantity + c.foilQuantity > 0).length;
          const total = section.cards.length;
          return { label: section.title, owned, total, percent: total === 0 ? 0 : (owned * 100) / total };
        })()
      : view && { label: null, owned: view.ownedCount, total: view.totalCount, percent: view.completionPercent };

  function changeLayout(next: CollectionLayout) {
    setLayout(next);
    setCollectionLayout(next);
  }

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
          {c.foilQuantity > 0 && <div className="foil-sheen" />}
          {total > 1 && <span className="badge-qty">×{total}</span>}
          {c.foilQuantity > 0 && <span className="badge-foil">{t("collection.foilBadge")}</span>}
        </div>
        <div className="card-caption visible">
          <span className="name">
            #{c.number} {c.name}
          </span>
          <span className="price">{c.priceCents == null ? "—" : <Money cents={c.priceCents} estimated={c.priceEstimated} />}</span>
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
        {layout === "grid" && (
          <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
            <option value="all">{t("collection.allCards")}</option>
            <option value="owned">{t("collection.owned")}</option>
            <option value="missing">{t("collection.missing")}</option>
          </select>
        )}
        <button onClick={() => void sellExtras()}>{t("collection.sellDuplicates")}</button>
        <div className="segmented" role="group" aria-label={t("collection.layout")}>
          <button className={layout === "binder" ? "active" : ""} onClick={() => changeLayout("binder")}>
            {t("collection.layoutBinder")}
          </button>
          <button className={layout === "grid" ? "active" : ""} onClick={() => changeLayout("grid")}>
            {t("collection.layoutGrid")}
          </button>
        </div>
      </div>

      {error && <p className="error">{error}</p>}
      {notice && <p className="notice">{notice}</p>}
      {!view && !error && <Loading label={t("collection.loading")} />}
      {view && (
        <>
          {progress && (
            <>
              <div className="progress">
                <div className="progress-bar" style={{ width: `${progress.percent}%` }} />
              </div>
              <p className="progress-label">
                {progress.label && <strong className="progress-section">{progress.label} · </strong>}
                {t(progress.label ? "collection.sectionProgress" : "collection.progress", {
                  owned: progress.owned,
                  total: progress.total,
                  percent: progress.percent.toFixed(1),
                })}
              </p>
            </>
          )}
          {layout === "binder" ? (
            <Binder
              key={view.setCode}
              setCode={view.setCode}
              sections={binderSections}
              onOpen={setDetailsOf}
              active={!detailsOf}
              onSection={setBinderSection}
            />
          ) : (
            <>
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
              {view.tokens.length > 0 && (
                <>
                  <h3>{t("collection.tokens")}</h3>
                  <p className="progress-label">
                    {t("collection.tokensProgress", { owned: view.tokensOwned, total: view.tokens.length })}
                  </p>
                  <div className="collection-grid">{shownTokens?.map(cardTile)}</div>
                </>
              )}
            </>
          )}
        </>
      )}
      {sale && <SaleModal {...sale} onClose={() => setSale(null)} />}
      {view && detailsOf && (
        <CardDetails setCode={view.setCode} uuid={detailsOf} onClose={() => setDetailsOf(null)} />
      )}
    </section>
  );
}
