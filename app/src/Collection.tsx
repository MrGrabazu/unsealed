import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import {
  checkSetImages,
  getArtistCollection,
  getCollection,
  getCycles,
  getOwnedCards,
  listArtists,
  listGuestSeries,
  sellCards,
  sellDuplicates,
  type ArtistCard,
  type ArtistSummary,
  type ArtistView,
  type CollectionCard,
  type CollectionView,
  type CyclesView,
  type SetSummary,
} from "./api";

/** The collection shown: the library of binders, one set's, or one artist's across the sets. */
type Source = "library" | "sets" | "artists";

/** In the set list: every card owned, across the sets (a grid to sort, to pick what to sell). */
const ALL_SETS = "*";

type Sort = "priceDesc" | "priceAsc" | "rarity" | "name" | "set" | "quantity";

/** What a card owned is worth: the market price of the finest finish owned. */
const ownedValue = (c: CollectionCard) =>
  Math.max(c.quantity > 0 ? (c.priceCents ?? 0) : 0, c.foilQuantity > 0 ? (c.foilPriceCents ?? 0) : 0);

const RARITY_ORDER = ["mythic", "rare", "special", "bonus", "uncommon", "common", "art", "token"];
const rarityRank = (rarity: string) => {
  const i = RARITY_ORDER.indexOf(rarity);
  return i < 0 ? RARITY_ORDER.length : i;
};

function sortCards(cards: ArtistCard[], sort: Sort): ArtistCard[] {
  const sorted = [...cards];
  const byName = (a: ArtistCard, b: ArtistCard) => a.name.localeCompare(b.name);
  switch (sort) {
    case "priceDesc":
      return sorted.sort((a, b) => ownedValue(b) - ownedValue(a) || byName(a, b));
    case "priceAsc":
      return sorted.sort((a, b) => ownedValue(a) - ownedValue(b) || byName(a, b));
    case "rarity":
      return sorted.sort((a, b) => rarityRank(a.rarity) - rarityRank(b.rarity) || ownedValue(b) - ownedValue(a));
    case "name":
      return sorted.sort(byName);
    case "set":
      return sorted.sort((a, b) => b.releaseDate.localeCompare(a.releaseDate) || a.setName.localeCompare(b.setName) || byName(a, b));
    case "quantity":
      return sorted.sort((a, b) => b.quantity + b.foilQuantity - (a.quantity + a.foilQuantity) || ownedValue(b) - ownedValue(a));
  }
}

/** Every card of a set's binder, its sections one after the other. */
const sectionsOf = (view: CollectionView) => [
  ...view.cards,
  ...view.extras,
  ...view.tokens,
  ...view.arts,
  ...view.guests,
  ...view.elsewhere,
];

/** For a search: lower case, without accents ("Magali Villeneuve" ~ "magali villeneuve"). */
const fold = (text: string) => text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
import { Binder, type BinderSection } from "./Binder";
import { CardDetails } from "./CardDetails";
import { CardImage } from "./CardImage";
import { CyclesList, CyclesPanel } from "./Cycles";
import { useI18n } from "./i18n";
import { Library } from "./Library";
import { Loading } from "./Loading";
import { duplicatesOf, SaleModal, type SoldLine } from "./SaleModal";
import { FoilSheen, foilLabel } from "./foils";
import { SellPicker } from "./SellPicker";
import { SetIcon } from "./SetIcon";
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
  /** Takes the player to the Boosterium, on a set's boosters. */
  onShop: (setCode: string) => void;
  /** Selling a binder's duplicates pays this much more, in percent (an artifact's). */
  duplicatesBonus: number;
  /** The best share of a card's market price a sale can reach, that bonus included. */
  maxSellRate: number;
}

/** Collection of one set: completion progress, missing cards greyed out, selling. */
export function Collection({ sets, refreshKey, onChanged, onShop, duplicatesBonus, maxSellRate }: Props) {
  // A binder's duplicates, at the price `sell_duplicates` pays (the artifact's bonus on top,
  // never above the best rate on the market price: `Bonuses::duplicate_value`).
  const binderDuplicates = (cards: CollectionCard[]): SoldLine[] =>
    duplicatesOf(cards).map((l) => {
      const boosted = l.unitCents + Math.floor((l.unitCents * duplicatesBonus) / 100);
      const market = l.foil ? l.card.foilPriceCents : l.card.priceCents;
      const best = market == null ? boosted : Math.floor((market * maxSellRate) / 100);
      return { ...l, unitCents: Math.max(l.unitCents, Math.min(boosted, best)) };
    });
  const money = useMoney();
  const moneyText = useMoneyText();
  const { t, tx, plural, lang } = useI18n();
  const [setCode, setSetCode] = useState("");
  const [view, setView] = useState<CollectionView | null>(null);
  // The set's cycles, and whether they are shown in place of its binder or grid.
  const [cycles, setCycles] = useState<CyclesView | null>(null);
  const [showCycles, setShowCycles] = useState(false);
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
  // The window to sell in bulk (every card, or one rarity).
  const [picking, setPicking] = useState(false);
  // The binder's section on show (index in `binderSections`).
  const [binderSection, setBinderSection] = useState(0);
  // Collections by artist.
  const [source, setSource] = useState<Source>("library");
  const [artists, setArtists] = useState<ArtistSummary[] | null>(null);
  const [artist, setArtist] = useState("");
  const [artistSearch, setArtistSearch] = useState("");
  const [artistView, setArtistView] = useState<ArtistView | null>(null);
  // "All sets": every card owned.
  const [ownedAll, setOwnedAll] = useState<ArtistCard[] | null>(null);
  const [sort, setSort] = useState<Sort>("priceDesc");
  // The guest series (Special Guests, The List…), with binders of their own.
  const [series, setSeries] = useState<SetSummary[]>([]);
  const known = useMemo(() => [...sets, ...series], [sets, series]);
  const isSeries = (code: string) => series.some((s) => s.code === code);

  useEffect(() => {
    if (!setCode && sets.length > 0) setSetCode(sets[0].code);
  }, [sets, setCode]);

  useEffect(() => {
    listGuestSeries()
      .then(setSeries)
      .catch(() => setSeries([]));
  }, [refreshKey]);

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
      if (source === "library") {
        // The library loads its own binders.
      } else if (source === "artists") {
        if (artist) setArtistView(await getArtistCollection(artist));
      } else if (setCode === ALL_SETS) {
        setOwnedAll(await getOwnedCards());
      } else if (setCode) {
        setView(await getCollection(setCode));
        setCycles(await getCycles(setCode, lang).catch(() => null));
        // Translated images: shown once known as good scans (checked once per card).
        if ((await checkSetImages(setCode).catch(() => 0)) > 0) {
          setView(await getCollection(setCode));
          setCycles(await getCycles(setCode, lang).catch(() => null));
        }
      }
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [source, setCode, artist, lang]);

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
      const lines = view ? binderDuplicates(sectionsOf(view)) : [];
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

  /** Sells the lines chosen in the bulk sale window, then shows what it brought in. */
  async function sellLines(lines: SoldLine[]) {
    try {
      const result = await sellCards(lines.map((l) => ({ uuid: l.card.uuid, foil: l.foil, quantity: l.quantity })));
      sounds.coins();
      setNotice(null);
      setPicking(false);
      setSale({ soldCount: result.soldCount, totalCents: result.totalCents, lines });
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
  const shownGuests = view?.guests.filter(matches);
  const shownElsewhere = view?.elsewhere.filter(matches);

  const library = source === "library";
  const byArtist = source === "artists";
  // Every card owned: a sortable grid only.
  const allSets = source === "sets" && setCode === ALL_SETS;
  const sortedOwned = useMemo(() => (ownedAll ? sortCards(ownedAll, sort) : []), [ownedAll, sort]);

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
              { title: t("collection.guests"), cards: view.guests },
              { title: t("collection.elsewhere"), cards: view.elsewhere },
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
  const detailsSet = byArtist
    ? artistView?.cards.find((c) => c.uuid === detailsOf)?.setCode
    : allSets
      ? ownedAll?.find((c) => c.uuid === detailsOf)?.setCode
      : // A guest's file is that of the first set whose boosters give it.
        view && (sectionsOf(view).find((c) => c.uuid === detailsOf)?.boosters?.[0] ?? view.setCode);
  const shownArtists = useMemo(() => {
    const query = fold(artistSearch.trim());
    return (artists ?? []).filter((a) => !query || fold(a.name).includes(query)).slice(0, 300);
  }, [artists, artistSearch]);

  function changeLayout(next: CollectionLayout) {
    setShowCycles(false);
    setLayout(next);
    setCollectionLayout(next);
  }

  /** A binder taken from the library's shelves: that set's collection, as a binder. */
  function openBinder(code: string) {
    sounds.page();
    setSetCode(code);
    setBinderSection(0);
    setLayout("binder");
    setSource("sets");
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
          {c.foilQuantity > 0 && <FoilSheen rarity={c.rarity} kind={c.foilKind} />}
          {total > 1 && <span className="badge-qty">×{total}</span>}
          {/* A special foil has its own badge (its name, from FoilSheen). */}
          {c.foilQuantity > 0 && (c.rarity === "art" || !foilLabel(c.foilKind)) && (
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
        {!("setName" in c) && c.boosters && c.boosters.length > 0 && (
          <div className="card-set hint">
            {t("collection.fromBoosters", {
              sets: c.boosters.map((code) => known.find((s) => s.code === code)?.name ?? code).join(", "),
            })}
          </div>
        )}
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

  // One set's collection: its binder (or grid) beside a panel with its completion and sales.
  const oneSet = source === "sets" && !allSets && !!view;
  // Its cycles instead, when it has some and they are asked for.
  const hasCycles = oneSet && !!cycles && cycles.setCode === view?.setCode && cycles.total > 0;
  const cyclesShown = hasCycles && showCycles;

  const bar = (
    <>
      {/* One row: what to show (left), the filters of the view shown (middle), the layout
          (right). */}
      <div className="collection-bar compact">
        <div className="segmented" role="group" aria-label={t("collection.source")}>
          <button className={library ? "active" : ""} onClick={() => setSource("library")}>
            {t("collection.library")}
          </button>
          <button className={source === "sets" ? "active" : ""} onClick={() => setSource("sets")}>
            {t("collection.bySet")}
          </button>
          <button className={byArtist ? "active" : ""} onClick={() => setSource("artists")}>
            {t("collection.byArtist")}
          </button>
        </div>
        <div className="collection-bar-middle">
        {source === "sets" && (
          <select value={setCode} onChange={(e) => setSetCode(e.target.value)}>
            <option value={ALL_SETS}>{t("collection.allSets")}</option>
            {sets.map((s) => (
              <option key={s.code} value={s.code}>
                {s.name} ({s.code})
              </option>
            ))}
            {series.length > 0 && (
              <optgroup label={t("collection.guestSeries")}>
                {series.map((s) => (
                  <option key={s.code} value={s.code}>
                    {s.name} ({s.code})
                  </option>
                ))}
              </optgroup>
            )}
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
        {allSets && (
          <select value={sort} onChange={(e) => setSort(e.target.value as Sort)} aria-label={t("collection.sort")}>
            <option value="priceDesc">{t("collection.sortPriceDesc")}</option>
            <option value="priceAsc">{t("collection.sortPriceAsc")}</option>
            <option value="rarity">{t("collection.sortRarity")}</option>
            <option value="name">{t("collection.sortName")}</option>
            <option value="set">{t("collection.sortSet")}</option>
            <option value="quantity">{t("collection.sortQuantity")}</option>
          </select>
        )}
        {layout === "grid" && !allSets && !library && !cyclesShown && (
          <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
            <option value="all">{t("collection.allCards")}</option>
            <option value="owned">{t("collection.owned")}</option>
            <option value="missing">{t("collection.missing")}</option>
          </select>
        )}
        </div>
        <div className="collection-bar-right">
          {!allSets && !library && (
            <div className="segmented" role="group" aria-label={t("collection.layout")}>
              <button className={layout === "binder" && !cyclesShown ? "active" : ""} onClick={() => changeLayout("binder")}>
                {t("collection.layoutBinder")}
              </button>
              <button className={layout === "grid" && !cyclesShown ? "active" : ""} onClick={() => changeLayout("grid")}>
                {t("collection.layoutGrid")}
              </button>
              {hasCycles && (
                <button className={cyclesShown ? "active" : ""} onClick={() => setShowCycles(true)}>
                  {t("collection.layoutCycles")}
                </button>
              )}
            </div>
          )}
        </div>
      </div>
      {error && <p className="error">{error}</p>}
      {notice && <p className="notice">{notice}</p>}
    </>
  );

  const content = (
    <>
      {library && <Library sets={known} refreshKey={refreshKey} onOpen={openBinder} />}
      {!library && !(byArtist ? artistView : allSets ? ownedAll : view) && !error && <Loading label={t("collection.loading")} />}
      {byArtist && artistView && (
        <>
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
      {allSets && ownedAll && (
        <>
          <p className="progress-label">
            {t("collection.ownedSummary", {
              cards: ownedAll.length,
              copies: ownedAll.reduce((n, c) => n + c.quantity + c.foilQuantity, 0),
            })}
          </p>
          {ownedAll.length === 0 ? (
            <p className="hint">{t("collection.noneOwned")}</p>
          ) : (
            <div className="collection-grid">{sortedOwned.map(cardTile)}</div>
          )}
        </>
      )}
      {cyclesShown && cycles && <CyclesList view={cycles} onOpen={setDetailsOf} />}
      {source === "sets" && !allSets && view && !cyclesShown && (
        <>
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
              {view.guests.length > 0 && (
                <>
                  <h3>{t("collection.guests")}</h3>
                  <p className="progress-label">
                    {t("collection.guestsProgress", { owned: view.guestsOwned, total: view.guests.length })}
                  </p>
                  <div className="collection-grid">{shownGuests?.map(cardTile)}</div>
                </>
              )}
              {view.elsewhere.length > 0 && (
                <>
                  <h3>{t("collection.elsewhere")}</h3>
                  <p className="progress-label">
                    {t("collection.elsewhereProgress", { owned: view.elsewhereOwned, total: view.elsewhere.length })}
                  </p>
                  <div className="collection-grid">{shownElsewhere?.map(cardTile)}</div>
                </>
              )}
            </>
          )}
        </>
      )}
    </>
  );

  // The cards of the completion panel: one set's, or one artist's across the sets.
  const panelCards: CollectionCard[] | null = byArtist
    ? (artistView?.cards ?? null)
    : oneSet && view
      ? sectionsOf(view)
      : null;

  const dialogs = (
    <>
      {picking && panelCards && (
        <SellPicker
          cards={panelCards}
          onSell={sellLines}
          onClose={() => setPicking(false)}
        />
      )}
      {sale && <SaleModal {...sale} onClose={() => setSale(null)} />}
      {detailsSet && detailsOf && (
        <CardDetails setCode={detailsSet} uuid={detailsOf} onClose={() => setDetailsOf(null)} />
      )}
    </>
  );

  const panelName = byArtist ? artistView?.name : oneSet ? view?.setName : undefined;
  if (!panelCards || panelName === undefined) {
    return (
      <section className="frame">
        <div className="cartouche">{t(library ? "collection.library" : byArtist ? "collection.byArtist" : "tab.collection")}</div>
        {bar}
        {content}
        {dialogs}
      </section>
    );
  }

  const set = view && !byArtist ? known.find((s) => s.code === view.setCode) : undefined;
  const ofSeries = !byArtist && !!view && isSeries(view.setCode);
  // A guest series: the sets whose boosters give its cards, those giving the most first.
  const givenBy = ofSeries && view
    ? Object.entries(
        view.cards.reduce<Record<string, number>>((n, c) => {
          for (const code of c.boosters ?? []) n[code] = (n[code] ?? 0) + 1;
          return n;
        }, {}),
      ).sort((a, b) => b[1] - a[1])
    : [];
  const ownedValueCents = panelCards.reduce((n, c) => n + c.quantity * (c.priceCents ?? 0) + c.foilQuantity * (c.foilPriceCents ?? 0), 0);
  const resaleCents = panelCards.reduce((n, c) => n + c.quantity * c.sellValueCents + c.foilQuantity * c.foilSellValueCents, 0);
  // An artist's duplicates sell card by card (`sell_cards`), without the binder's bonus.
  const duplicates = byArtist ? duplicatesOf(panelCards) : binderDuplicates(panelCards);
  const duplicatesCents = duplicates.reduce((n, l) => n + l.quantity * l.unitCents, 0);
  // A set's rows count its main cards (its tokens and arts have their own binder sections); an
  // artist's count every card, tokens and arts included, as its completion does.
  const rarityCards = byArtist ? panelCards : (view?.cards ?? []);
  const byRarity = (rarities: string[]) => {
    const cards = rarityCards.filter((c) => rarities.includes(c.rarity));
    return { owned: cards.filter((c) => c.quantity + c.foilQuantity > 0).length, total: cards.length };
  };
  const rows: [string, { owned: number; total: number }][] = (
    [
      [t("sell.common"), byRarity(["common"])],
      [t("sell.uncommon"), byRarity(["uncommon"])],
      [t("sell.rare"), byRarity(["rare"])],
      [t("sell.mythic"), byRarity(["mythic"])],
      [t("sell.other"), byRarity(["special", "bonus"])],
      [t("collection.tokens"), byArtist ? byRarity(["token"]) : { owned: 0, total: 0 }],
      [t("collection.arts"), byArtist ? byRarity(["art"]) : { owned: 0, total: 0 }],
    ] as [string, { owned: number; total: number }][]
  ).filter(([, r]) => r.total > 0);
  const percent = progress?.percent ?? 0;
  // An artist's years: from their oldest card to their latest.
  const years =
    byArtist && artistView && artistView.cards.length > 0
      ? [artistView.cards[0].releaseDate.slice(0, 4), artistView.cards[artistView.cards.length - 1].releaseDate.slice(0, 4)]
      : null;
  const initials = panelName
    .split(/\s+/)
    .filter((w) => /^\p{L}/u.test(w))
    .slice(0, 2)
    .map((w) => w[0].toUpperCase())
    .join("");

  async function sellPanelDuplicates() {
    if (!byArtist) return sellExtras();
    if (duplicates.length === 0) setNotice(t("collection.noDuplicates"));
    else await sellLines(duplicates);
  }

  return (
    // The whole screen scrolls (not the binder in its frame): the completion panel stays in
    // view beside it.
    <div className="split flow collection-split">
      <section className="frame">
        <div className="cartouche">{byArtist ? t("collection.byArtist") : view?.setName}</div>
        {bar}
        {content}
      </section>
      {cyclesShown && cycles ? (
        <CyclesPanel view={cycles} onShop={view && !ofSeries ? () => onShop(view.setCode) : undefined} />
      ) : (
      <aside className="frame set-side">
        <div className="cartouche">{t("collection.completion")}</div>
        <div className="scroll">
          <h2>
            <span className="medallion lg">
              {byArtist ? <span className="initials">{initials}</span> : <SetIcon url={set?.iconUrl ?? null} name={panelName} />}
            </span>
            {panelName}
          </h2>
          <div className="meta">
            {years
              ? years[0] === years[1]
                ? years[0]
                : `${years[0]} – ${years[1]}`
              : view && (
                  <>
                    {view.setCode}
                    {set && ` · ${set.releaseDate.slice(0, 4)}`}
                  </>
                )}
          </div>
          <div className="ring" style={{ "--p": percent } as CSSProperties}>
            <b>{Math.floor(percent)} %</b>
            <small>
              {progress?.owned ?? 0} / {progress?.total ?? 0}
            </small>
          </div>
          {progress?.label && <div className="ring-label">{progress.label}</div>}
          {rows.map(([label, r]) => (
            <div key={label} className="ready-line">
              <span>{label}</span>
              <b>
                {r.owned} / {r.total}
              </b>
            </div>
          ))}
          <div className="ready-line">
            <span>{t("collection.valueResale")}</span>
            <b>
              <Money cents={ownedValueCents} /> · <Money cents={resaleCents} />
            </b>
          </div>
          {/* The guest series its boosters give, each in a binder of its own. */}
          {!byArtist && view && view.guestLinks.length > 0 && (
            <>
              <div className="side-title">{t("collection.guests")}</div>
              {view.guestLinks.map((l) => (
                <button key={l.code} className="ready-line link" onClick={() => openBinder(l.code)}>
                  <span title={l.name}>{l.name}</span>
                  <b>
                    {l.owned} / {l.total} ›
                  </b>
                </button>
              ))}
            </>
          )}
          {givenBy.length > 0 && (
            <>
              <div className="side-title">{t("collection.givenBy")}</div>
              {givenBy.map(([code, count]) => (
                <button key={code} className="ready-line link" onClick={() => onShop(code)}>
                  <span>{known.find((s) => s.code === code)?.name ?? code}</span>
                  <b>
                    {plural("collection.cardCount", count)} ›
                  </b>
                </button>
              ))}
            </>
          )}
        </div>
        <div className="buy-bar">
          <button className="big" onClick={() => void sellPanelDuplicates()} disabled={duplicatesCents === 0}>
            {tx("collection.sellDuplicatesFor", { amount: money(duplicatesCents) })}
          </button>
          <div className="ready-more">
            <button onClick={() => setPicking(true)}>{t("collection.sellMore")}</button>
            {view && !byArtist && !ofSeries && <button onClick={() => onShop(view.setCode)}>{t("collection.buyBoosters")}</button>}
          </div>
        </div>
      </aside>
      )}
      {dialogs}
    </div>
  );
}
