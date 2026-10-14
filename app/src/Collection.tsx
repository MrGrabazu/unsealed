import { useCallback, useEffect, useMemo, useState } from "react";
import {
  checkSetImages,
  getArtistCollection,
  getCollection,
  listArtists,
  sellCards,
  sellDuplicates,
  type ArtistCard,
  type ArtistSummary,
  type ArtistView,
  type CollectionCard,
  type CollectionView,
  type SetSummary,
} from "./api";

/** The collection shown: one set's, or one artist's across the sets. */
type Source = "sets" | "artists";

/** For a search: lower case, without accents ("Magali Villeneuve" ~ "magali villeneuve"). */
const fold = (text: string) => text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
import { Binder, type BinderSection } from "./Binder";
import { CardDetails } from "./CardDetails";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { duplicatesOf, SaleModal, type SoldLine } from "./SaleModal";
import { sheenClass } from "./RevealedCards";
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
  // Collections by artist.
  const [source, setSource] = useState<Source>("sets");
  const [artists, setArtists] = useState<ArtistSummary[] | null>(null);
  const [artist, setArtist] = useState("");
  const [artistSearch, setArtistSearch] = useState("");
  const [artistView, setArtistView] = useState<ArtistView | null>(null);

  useEffect(() => {
    if (!setCode && sets.length > 0) setSetCode(sets[0].code);
  }, [sets, setCode]);

  // The artists, most prolific first, loaded the first time they are asked for.
  useEffect(() => {
    if (source !== "artists") return;
    listArtists()
      .then((list) => {
        const sorted = [...list].sort((a, b) => b.cardCount - a.cardCount || a.name.localeCompare(b.name));
        setArtists(sorted);
        if (!artist && sorted.length > 0) setArtist(sorted[0].name);
      })
      .catch((e) => setError(String(e)));
  }, [source, refreshKey]);

  const reload = useCallback(async () => {
    try {
      if (source === "artists") {
        if (artist) setArtistView(await getArtistCollection(artist));
      } else if (setCode) {
        setView(await getCollection(setCode));
        // Translated images: shown once known as good scans (checked once per card).
        if ((await checkSetImages(setCode).catch(() => 0)) > 0) setView(await getCollection(setCode));
      }
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [source, setCode, artist]);

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
      const lines = view ? duplicatesOf([...view.cards, ...view.extras, ...view.tokens, ...view.arts]) : [];
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
  const shownArts = view?.arts.filter(matches);

  const byArtist = source === "artists";

  // The binder keeps every card in its place: no filter, sections one after the other (an
  // artist's binder is one section, oldest card first).
  const binderSections = useMemo<BinderSection[]>(
    () =>
      byArtist
        ? artistView
          ? [{ title: null, cards: artistView.cards }]
          : []
        : view
          ? [
              { title: null, cards: view.cards },
              { title: t("collection.extras"), cards: view.extras },
              { title: t("collection.tokens"), cards: view.tokens },
              { title: t("collection.arts"), cards: view.arts },
            ].filter((s) => s.cards.length > 0)
          : [],
    [byArtist, artistView, view, t],
  );

  // In the binder, the completion shown is that of the section open (cards, extras, tokens):
  // tokens and extras are collected too, and the main bar would say nothing of them.
  const section = layout === "binder" ? binderSections[Math.min(binderSection, binderSections.length - 1)] : undefined;
  const progress = byArtist
    ? artistView && {
        label: null,
        owned: artistView.ownedCount,
        total: artistView.totalCount,
        percent: artistView.completionPercent,
      }
    : section && section.title !== null
      ? (() => {
          const owned = section.cards.filter((c) => c.quantity + c.foilQuantity > 0).length;
          const total = section.cards.length;
          return { label: section.title, owned, total, percent: total === 0 ? 0 : (owned * 100) / total };
        })()
      : view && { label: null, owned: view.ownedCount, total: view.totalCount, percent: view.completionPercent };

  // The set file holding the card whose details are open (an artist's cards span sets).
  const detailsSet = byArtist ? artistView?.cards.find((c) => c.uuid === detailsOf)?.setCode : view?.setCode;
  const shownArtists = useMemo(() => {
    const query = fold(artistSearch.trim());
    return (artists ?? []).filter((a) => !query || fold(a.name).includes(query)).slice(0, 300);
  }, [artists, artistSearch]);

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
          {c.foilQuantity > 0 && <div className={sheenClass(c.rarity)} />}
          {total > 1 && <span className="badge-qty">×{total}</span>}
          {c.foilQuantity > 0 && (
            <span className={`badge-foil ${c.rarity === "art" ? "signed" : ""}`}>
              {t(c.rarity === "art" ? "collection.signedBadge" : "collection.foilBadge")}
            </span>
          )}
        </div>
        <div className="card-caption visible">
          <span className="name">
            #{c.number} {c.name}
          </span>
          <span className="price">{c.priceCents == null ? "—" : <Money cents={c.priceCents} estimated={c.priceEstimated} />}</span>
        </div>
        {"setName" in c && <div className="card-set hint">{(c as ArtistCard).setName}</div>}
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
        <div className="segmented" role="group" aria-label={t("collection.source")}>
          <button className={!byArtist ? "active" : ""} onClick={() => setSource("sets")}>
            {t("collection.bySet")}
          </button>
          <button className={byArtist ? "active" : ""} onClick={() => setSource("artists")}>
            {t("collection.byArtist")}
          </button>
        </div>
        {!byArtist && (
          <select value={setCode} onChange={(e) => setSetCode(e.target.value)}>
            {sets.map((s) => (
              <option key={s.code} value={s.code}>
                {s.name} ({s.code})
              </option>
            ))}
          </select>
        )}
        {byArtist && (
          <>
            <input
              type="search"
              value={artistSearch}
              placeholder={t("collection.artistSearch")}
              onChange={(e) => setArtistSearch(e.target.value)}
            />
            <select value={artist} onChange={(e) => setArtist(e.target.value)} aria-label={t("collection.byArtist")}>
              {shownArtists.map((a) => (
                <option key={a.name} value={a.name}>
                  {a.name} ({a.ownedCount} / {a.cardCount})
                </option>
              ))}
            </select>
          </>
        )}
        {layout === "grid" && (
          <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
            <option value="all">{t("collection.allCards")}</option>
            <option value="owned">{t("collection.owned")}</option>
            <option value="missing">{t("collection.missing")}</option>
          </select>
        )}
        {!byArtist && <button onClick={() => void sellExtras()}>{t("collection.sellDuplicates")}</button>}
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
      {!(byArtist ? artistView : view) && !error && <Loading label={t("collection.loading")} />}
      {byArtist && artistView && (
        <>
          {progress && (
            <>
              <div className="progress">
                <div className="progress-bar" style={{ width: `${progress.percent}%` }} />
              </div>
              <p className="progress-label">
                <strong className="progress-section">{artistView.name} · </strong>
                {t("collection.progress", {
                  owned: progress.owned,
                  total: progress.total,
                  percent: progress.percent.toFixed(1),
                })}
              </p>
            </>
          )}
          {layout === "binder" ? (
            <Binder
              key={`artist:${artistView.name}`}
              setCode={`artist:${artistView.name}`}
              sections={binderSections}
              onOpen={setDetailsOf}
              active={!detailsOf}
              onSection={setBinderSection}
            />
          ) : (
            <div className="collection-grid">{artistView.cards.filter(matches).map(cardTile)}</div>
          )}
        </>
      )}
      {!byArtist && view && (
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
              {view.arts.length > 0 && (
                <>
                  <h3>{t("collection.arts")}</h3>
                  <p className="progress-label">
                    {t("collection.artsProgress", { owned: view.artsOwned, total: view.arts.length })}
                  </p>
                  <div className="collection-grid">{shownArts?.map(cardTile)}</div>
                </>
              )}
            </>
          )}
        </>
      )}
      {sale && <SaleModal {...sale} onClose={() => setSale(null)} />}
      {detailsSet && detailsOf && (
        <CardDetails setCode={detailsSet} uuid={detailsOf} onClose={() => setDetailsOf(null)} />
      )}
    </section>
  );
}
