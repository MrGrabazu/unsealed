import { Fragment, memo, useMemo, useState } from "react";
import type { CardPulls, HistoryOpening, HistoryPull, OpenedCard, SetHistoryView } from "./api";
import { FlatCard } from "./Card";
import { NearImage } from "./CardImage";
import { kindLabel, kindRank } from "./boosterKinds";
import { useI18n } from "./i18n";
import { smallImage } from "./images";
import { Money } from "./money";

/** Boosters shown at first, and added by each "show more". */
const PAGE = 20;

type Mode = "boosters" | "cards";
type CardFilter = "all" | "new" | "rare";
type Sort = "number" | "first" | "last" | "pulls" | "best";

const isRareOrAbove = (rarity: string) => rarity === "rare" || rarity === "mythic" || rarity === "special" || rarity === "bonus";

/** A pull as a card to draw: the card, in the finish and at the price it came with. */
function pulledCard(cards: SetHistoryView["cards"], pull: HistoryPull): OpenedCard | null {
  const card = cards[pull.uuid];
  if (!card) return null;
  return {
    ...card,
    foil: pull.foil,
    foilKind: pull.foil ? card.foilKind : undefined,
    priceCents: pull.priceCents,
    priceEstimated: false,
    isNew: pull.isNew,
    newFoil: pull.newFoil,
    // Small images: these cards are a little over 100 px wide.
    imageUrl: smallImage(card.imageUrl),
    backImageUrl: smallImage(card.backImageUrl),
  };
}

/** Day by day: today, yesterday, then the date. */
function useDates() {
  const { t, lang } = useI18n();
  return useMemo(() => {
    const dayOf = (at: number) => new Date(at * 1000).toDateString();
    const today = new Date().toDateString();
    const yesterday = new Date(Date.now() - 86_400_000).toDateString();
    return {
      dayOf,
      dayLabel: (at: number) => {
        const day = dayOf(at);
        if (day === today) return t("history.today");
        if (day === yesterday) return t("history.yesterday");
        return new Date(at * 1000).toLocaleDateString(lang, { dateStyle: "full" });
      },
      time: (at: number) => new Date(at * 1000).toLocaleTimeString(lang, { timeStyle: "short" }),
      date: (at: number) => new Date(at * 1000).toLocaleDateString(lang, { dateStyle: "medium" }),
    };
  }, [t, lang]);
}

/** What a booster brought against what it cost: its gain or loss, "Given" when it cost nothing,
 * nothing when its cost was not kept. */
function Balance({ opening }: { opening: HistoryOpening }) {
  const { t } = useI18n();
  if (opening.costCents == null) return null;
  if (opening.costCents === 0) return <span className="history-given">{t("setHistory.given")}</span>;
  const gap = opening.valueCents - opening.costCents;
  return (
    <span className={`history-gap ${gap >= 0 ? "history-up" : "history-down"}`}>
      {gap >= 0 ? "+" : "−"}
      <Money cents={Math.abs(gap)} />
    </span>
  );
}

/** One booster opened: when, its kind, its news, its value and balance, then its cards (each
 * opening its details). */
const HistoryBooster = memo(function HistoryBooster({
  opening,
  cards,
  filter,
  onOpen,
}: {
  opening: HistoryOpening;
  cards: SetHistoryView["cards"];
  filter: CardFilter;
  onOpen: (uuid: string) => void;
}) {
  const { t } = useI18n();
  const { time } = useDates();
  const shown = opening.pulls
    .filter((p) => filter === "all" || (filter === "new" ? p.isNew || p.newFoil : isRareOrAbove(cards[p.uuid]?.rarity ?? "") || p.foil))
    .map((p) => pulledCard(cards, p))
    .filter((c): c is OpenedCard => c != null);
  const news = opening.pulls.filter((p) => p.isNew).length;
  return (
    <div className="batch-booster history-booster">
      <div className="batch-booster-head">
        <b>{time(opening.openedAt)}</b>
        <span className="history-kind">{kindLabel(t, opening.kind)}</span>
        <span className="grow" />
        {news > 0 && <span className="hint">{t("setHistory.news", { count: news })}</span>}
        <Money cents={opening.valueCents} />
        <Balance opening={opening} />
      </div>
      <div className="cards batch-cards">
        {shown.map((card, i) => (
          <div key={`${card.uuid}-${i}`}>
            <FlatCard card={card} onClick={() => onOpen(card.uuid)} />
          </div>
        ))}
      </div>
    </div>
  );
});

/** The boosters opened, day by day, the latest first; filtered by kind and by the cards shown. */
function ByBooster({ view, onOpen }: { view: SetHistoryView; onOpen: (uuid: string) => void }) {
  const { t } = useI18n();
  const { dayOf, dayLabel } = useDates();
  const [kind, setKind] = useState("");
  const [filter, setFilter] = useState<CardFilter>("all");
  const [count, setCount] = useState(PAGE);
  const kinds = useMemo(() => [...new Set(view.openings.map((o) => o.kind))].sort((a, b) => kindRank(a) - kindRank(b)), [view]);
  const matching = view.openings.filter(
    (o) =>
      (!kind || o.kind === kind) &&
      (filter === "all" ||
        o.pulls.some((p) => (filter === "new" ? p.isNew || p.newFoil : isRareOrAbove(view.cards[p.uuid]?.rarity ?? "") || p.foil))),
  );
  const shown = matching.slice(0, count);
  return (
    <>
      <div className="cycles-tools">
        {kinds.length > 1 && (
          <div className="segmented" role="group">
            <button className={!kind ? "active" : ""} onClick={() => setKind("")}>
              {t("setHistory.allKinds")}
            </button>
            {kinds.map((k) => (
              <button key={k} className={kind === k ? "active" : ""} onClick={() => setKind(k)}>
                {kindLabel(t, k)}
              </button>
            ))}
          </div>
        )}
        <div className="segmented" role="group">
          {(["all", "new", "rare"] as CardFilter[]).map((f) => (
            <button key={f} className={filter === f ? "active" : ""} onClick={() => setFilter(f)}>
              {t(`setHistory.cards.${f}`)}
            </button>
          ))}
        </div>
      </div>
      <div className="history-boosters">
        {shown.map((o, i) => (
          <Fragment key={o.id}>
            {(i === 0 || dayOf(shown[i - 1].openedAt) !== dayOf(o.openedAt)) && <h3 className="history-day">{dayLabel(o.openedAt)}</h3>}
            <HistoryBooster opening={o} cards={view.cards} filter={filter} onOpen={onOpen} />
          </Fragment>
        ))}
      </div>
      {matching.length > count && (
        <button className="history-more" onClick={() => setCount((n) => n + PAGE)}>
          {t("setHistory.more", { count: Math.min(PAGE, matching.length - count) })}
        </button>
      )}
    </>
  );
}

/** The cards pulled, one line each: when first and last, how often, in foil, the best price;
 * searchable, sorted by a click on a column's head. */
function ByCard({ view, onOpen }: { view: SetHistoryView; onOpen: (uuid: string) => void }) {
  const { t } = useI18n();
  const { date } = useDates();
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<Sort>("number");
  const needle = search.trim().toLowerCase();
  const rows = useMemo(() => {
    const list = view.ledger.filter((e) => !needle || (view.cards[e.uuid]?.name ?? "").toLowerCase().includes(needle));
    const by: Record<Sort, ((a: CardPulls, b: CardPulls) => number) | null> = {
      number: null,
      first: (a, b) => b.firstAt - a.firstAt,
      last: (a, b) => b.lastAt - a.lastAt,
      pulls: (a, b) => b.pulls - a.pulls,
      best: (a, b) => (b.bestPriceCents ?? -1) - (a.bestPriceCents ?? -1),
    };
    const compare = by[sort];
    return compare ? [...list].sort(compare) : list;
  }, [view, needle, sort]);
  const head = (key: Sort | null, label: string) =>
    key ? (
      <th className={`sortable ${sort === key ? "sorted" : ""}`} onClick={() => setSort(key)}>
        {label}
      </th>
    ) : (
      <th>{label}</th>
    );
  return (
    <>
      <div className="cycles-tools">
        <input type="search" value={search} placeholder={t("setHistory.search")} onChange={(e) => setSearch(e.target.value)} />
      </div>
      {rows.length === 0 ? (
        <p className="hint">{t("setHistory.noMatch")}</p>
      ) : (
        <table className="history-ledger">
          <thead>
            <tr>
              <th />
              {head("number", t("setHistory.col.number"))}
              {head(null, t("setHistory.col.card"))}
              {head(null, t("setHistory.col.rarity"))}
              {head("first", t("setHistory.col.first"))}
              {head("last", t("setHistory.col.last"))}
              {head("pulls", t("setHistory.col.pulls"))}
              {head(null, t("setHistory.col.foils"))}
              {head("best", t("setHistory.col.best"))}
            </tr>
          </thead>
          <tbody>
            {rows.map((e) => {
              const card = view.cards[e.uuid];
              if (!card) return null;
              return (
                <tr key={e.uuid} onClick={() => onOpen(e.uuid)}>
                  <td className="history-thumb">
                    <NearImage src={smallImage(card.imageUrl) ?? undefined} alt={card.name} />
                  </td>
                  <td>{card.setCode === view.setCode ? card.number : `${card.setCode} ${card.number}`}</td>
                  <td className="history-name">{card.name}</td>
                  <td>
                    <span className={`history-rarity ${card.rarity}`}>{card.rarity.slice(0, 1).toUpperCase()}</span>
                  </td>
                  <td>{date(e.firstAt)}</td>
                  <td>{date(e.lastAt)}</td>
                  <td className="history-count">×{e.pulls}</td>
                  <td className="history-count">{e.foils > 0 ? `✦ ${e.foils}` : "—"}</td>
                  <td>{e.bestPriceCents == null ? "—" : <Money cents={e.bestPriceCents} />}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </>
  );
}

/** A set's history, in place of its binder: by booster (default) or by card. */
export function SetHistoryList({ view, onOpen }: { view: SetHistoryView; onOpen: (uuid: string) => void }) {
  const { t } = useI18n();
  const [mode, setMode] = useState<Mode>("boosters");
  return (
    <div className="set-history">
      <div className="segmented history-mode" role="group">
        <button className={mode === "boosters" ? "active" : ""} onClick={() => setMode("boosters")}>
          {t("setHistory.byBooster")}
        </button>
        <button className={mode === "cards" ? "active" : ""} onClick={() => setMode("cards")}>
          {t("setHistory.byCard")}
        </button>
      </div>
      {mode === "boosters" ? <ByBooster view={view} onOpen={onOpen} /> : <ByCard view={view} onOpen={onOpen} />}
    </div>
  );
}

/** Beside the history: what the set has given so far, its best card and best booster. */
export function SetHistoryPanel({ view, onOpen, onShop }: { view: SetHistoryView; onOpen: (uuid: string) => void; onShop?: () => void }) {
  const { t } = useI18n();
  const { date } = useDates();
  const s = view.summary;
  const best = s.bestPull ? pulledCard(view.cards, s.bestPull) : null;
  const bestPullOpening = view.openings.find((o) => o.id === s.bestPullOpening);
  const bestOpening = view.openings.find((o) => o.id === s.bestOpening);
  const returnPercent = s.spentCents > 0 ? Math.round((s.valueOfPaidCents * 100) / s.spentCents) : null;
  return (
    <aside className="frame set-side">
      <div className="cartouche">{t("setHistory.panel")}</div>
      <div className="scroll">
        <h3>{t("setHistory.story")}</h3>
        {s.firstOpenedAt != null && (
          <div className="ready-line">
            <span>{t("setHistory.firstBooster")}</span>
            <b>{date(s.firstOpenedAt)}</b>
          </div>
        )}
        <div className="ready-line">
          <span>{t("setHistory.boosters")}</span>
          <b>{s.boosters}</b>
        </div>
        <div className="ready-line">
          <span>{t("setHistory.cardsPulled")}</span>
          <b>
            {s.cardsPulled} <em>({t("setHistory.distinct", { count: s.distinctCards })})</em>
          </b>
        </div>
        <div className="ready-line">
          <span>{t("setHistory.pulled")}</span>
          <b>
            <Money cents={s.valueCents} />
          </b>
        </div>
        {s.spentCents > 0 && (
          <>
            <div className="ready-line">
              <span>{t("setHistory.spent")}</span>
              <b>
                <Money cents={s.spentCents} />
              </b>
            </div>
            <div className="ready-line">
              <span>{t("setHistory.return")}</span>
              <b className={returnPercent != null && returnPercent >= 100 ? "history-up" : "history-down"}>{returnPercent} %</b>
            </div>
          </>
        )}
        {best && (
          <>
            <h3>{t("setHistory.bestCard")}</h3>
            <div className="history-best">
              <div className="cards batch-cards">
                <div>
                  <FlatCard card={best} onClick={() => onOpen(best.uuid)} />
                </div>
              </div>
              {bestPullOpening && (
                <p className="hint">
                  {kindLabel(t, bestPullOpening.kind)} · {date(bestPullOpening.openedAt)}
                </p>
              )}
            </div>
          </>
        )}
        {bestOpening && (
          <>
            <h3>{t("setHistory.bestBooster")}</h3>
            <div className="ready-line">
              <span>
                {kindLabel(t, bestOpening.kind)} · {date(bestOpening.openedAt)}
              </span>
              <b>
                <Money cents={bestOpening.valueCents} /> <Balance opening={bestOpening} />
              </b>
            </div>
          </>
        )}
        <p className="hint cycles-note">{t("setHistory.hint")}</p>
      </div>
      {onShop && (
        <div className="buy-bar">
          <button className="big" onClick={onShop}>
            {t("collection.buyBoosters")}
          </button>
        </div>
      )}
    </aside>
  );
}
