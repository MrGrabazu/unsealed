import { memo, useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { OpenedCard } from "./api";
import { FlatCard } from "./Card";
import { useI18n } from "./i18n";
import { Money, useMoney } from "./money";
import { fittedImage, smallImage } from "./images";
import { getBatchDelay, setBatchDelay } from "./settings";

/** Time a booster takes to appear: its cards dealt one after the other, like a quick opening's
 * hand (each 0.35 s, 30 ms after the one before: `.batch-deal` of comptoir.css), before the
 * chosen pause. */
const dealMs = (cards: number) => 350 + Math.max(0, cards - 1) * 30;
/** The longest a booster waits for its images before showing anyway. */
const FACE_WAIT_MS = 1000;
/** The pause between two boosters, chosen with the slider: 0 to 5 s, by half-seconds. */
const DELAY_MAX_S = 5;
const DELAY_STEP_S = 0.5;
/** Cards at or above this market price count as big. */
const BIG_PRICE_CENTS = 500;
const BEST_PULLS_SHOWN = 8;

export const isBigCard = (c: OpenedCard) =>
  c.rarity === "mythic" || (c.priceCents ?? 0) >= BIG_PRICE_CENTS || (c.foil && c.rarity === "rare");

/** A batch being opened: how many boosters were asked, and how to open the next one (its
 * cards, or null when none is left or the opening failed). */
export interface BatchRun {
  id: number;
  total: number;
  next: () => Promise<OpenedCard[] | null>;
}

/** Loads and decodes a booster's images (both sizes a card may show), `FACE_WAIT_MS` at most. */
function preload(cards: OpenedCard[]): Promise<void> {
  const urls = [...new Set(cards.flatMap((c) => (c.imageUrl ? [c.imageUrl, fittedImage(c.imageUrl, 1, 1)] : [])))];
  const decoded = Promise.all(
    urls.map((src) => {
      const image = new Image();
      image.src = src;
      return image.decode().catch(() => {});
    }),
  );
  return Promise.race([decoded.then(() => {}), new Promise<void>((resolve) => setTimeout(resolve, FACE_WAIT_MS))]);
}

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** Plural texts counting the cards of each rarity. */
const RARITY_COUNTS: Record<string, "rarity.common" | "rarity.uncommon" | "rarity.rare" | "rarity.mythic"> = {
  common: "rarity.common",
  uncommon: "rarity.uncommon",
  rare: "rarity.rare",
  mythic: "rarity.mythic",
};

/** A booster of the batch already seen, in the list under the one opening (the latest first):
 * its number, value, news and cards, small, each opening its details. */
const SeenBooster = memo(function SeenBooster({
  n,
  cards,
  onDetails,
}: {
  n: number;
  cards: OpenedCard[];
  onDetails: (card: OpenedCard) => void;
}) {
  const { t } = useI18n();
  const value = cards.reduce((sum, c) => sum + (c.priceCents ?? 0), 0);
  const news = cards.filter((c) => c.isNew).length;
  // Small images: these cards are a little over 100 px wide.
  const small = useMemo(
    () => cards.map((card) => ({ ...card, imageUrl: smallImage(card.imageUrl), backImageUrl: smallImage(card.backImageUrl) })),
    [cards],
  );
  return (
    <div className="batch-booster">
      <div className="batch-booster-head">
        <b>{t("batch.booster", { n })}</b>
        <Money cents={value} />
        {news > 0 && <span className="hint">{t("batch.newCards", { count: news })}</span>}
      </div>
      <div className="cards batch-cards">
        {small.map((card, i) => (
          <div key={`${card.uuid}-${i}`} className={isBigCard(card) ? "big" : ""}>
            <FlatCard card={card} onClick={() => onDetails(cards[i])} />
          </div>
        ))}
      </div>
    </div>
  );
});

/** The boosters of the batch seen so far (up to `count`), the latest first. */
function SeenBoosters({ boosters, count, onDetails }: { boosters: OpenedCard[][]; count: number; onDetails: (card: OpenedCard) => void }) {
  const { t } = useI18n();
  // The same function from one render to the next, so each booster seen is drawn only once.
  const latest = useRef(onDetails);
  latest.current = onDetails;
  const details = useCallback((card: OpenedCard) => latest.current(card), []);
  if (count <= 0) return null;
  const seen = boosters.slice(0, count).map((cards, i) => ({ n: i + 1, cards })).reverse();
  return (
    <>
      <h3>{t("batch.openings")}</h3>
      <div className="batch-boosters">
        {seen.map(({ n, cards }) => (
          <SeenBooster key={n} n={n} cards={cards} onDetails={details} />
        ))}
      </div>
    </>
  );
}

/** Opens a batch the way a quick opening does, one booster after the other: each booster is
 * drawn (the next one already while this one shows, its images loaded ahead), dealt face up,
 * then stays the chosen pause (slider, 0 to 5 s); the player can pause between two boosters, or
 * open the rest at once. The boosters already seen pile up under the one showing (the latest
 * first), and stay under the summary at the end, to look at each draw. */
export function BatchOpening({
  run,
  boosters,
  held = false,
  onShown,
  onFinished,
  onDetails,
}: {
  run: BatchRun;
  /** Something is shown over the batch (a card's details): it waits, like a pause. */
  held?: boolean;
  /** The boosters shown so far, in order (kept by the parent: a sale needs them). */
  boosters: OpenedCard[][];
  onShown: (cards: OpenedCard[]) => void;
  /** Once, when the batch is over: how many boosters were opened. */
  onFinished: (opened: number) => void;
  onDetails: (card: OpenedCard) => void;
}) {
  const money = useMoney();
  const { t, tx, plural } = useI18n();
  const [playing, setPlaying] = useState(true);
  const [paused, setPaused] = useState(false);
  const [rushing, setRushing] = useState(false);
  const [delay, setDelay] = useState(getBatchDelay);
  // Read by the loop at each step, so the controls act at once.
  const control = useRef({ paused, rushing, delay, held });
  control.current = { paused, rushing, delay, held };
  const latest = useRef({ onShown, onFinished });
  latest.current = { onShown, onFinished };

  // The loop opens boosters: it must run once per batch, even when React mounts the component
  // twice in development (StrictMode), and stop when the player leaves the batch.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const started = useRef<number | null>(null);
  useEffect(() => {
    if (started.current === run.id) return;
    started.current = run.id;
    const live = () => mounted.current;
    void (async () => {
      let opened = 0;
      let ahead: Promise<OpenedCard[] | null> = run.next();
      for (let i = 0; i < run.total; i++) {
        const cards = await ahead;
        if (!live() || !cards) break;
        // The next booster is drawn while this one shows.
        ahead = i + 1 < run.total ? run.next() : Promise.resolve(null);
        if (!control.current.rushing) await preload(cards);
        if (!live()) return;
        latest.current.onShown(cards);
        opened++;
        if (control.current.rushing) continue;
        await sleep(dealMs(cards.length) + control.current.delay * 1000);
        // Paused, or a card's details open over the batch: the next booster waits.
        while (live() && (control.current.paused || control.current.held) && !control.current.rushing) await sleep(100);
        if (!live()) return;
      }
      // Leaving the batch midway: a booster already drawn ahead stays opened (its cards are in
      // the collection, like any opening), only not shown.
      if (live()) {
        setPlaying(false);
        latest.current.onFinished(opened);
      }
    })();
  }, [run]);

  const changeDelay = (seconds: number) => {
    setDelay(seconds);
    setBatchDelay(seconds);
  };

  const summary = useMemo(() => {
    const all = boosters.flat();
    const rarities: Record<string, number> = {};
    for (const c of all) rarities[c.rarity] = (rarities[c.rarity] ?? 0) + 1;
    const priced = [...all].sort((a, b) => (b.priceCents ?? 0) - (a.priceCents ?? 0));
    // Each new card once (two copies in a booster are both new), the most valuable first, in
    // small images (they are a little over 100 px wide).
    const once = (cards: OpenedCard[]) => {
      const seen = new Set<string>();
      return cards
        .filter((c) => {
          const key = `${c.uuid}:${c.foil}`;
          if (seen.has(key)) return false;
          seen.add(key);
          return true;
        })
        .map((card) => ({ card, shown: { ...card, imageUrl: smallImage(card.imageUrl), backImageUrl: smallImage(card.backImageUrl) } }));
    };
    const newCards = once(priced.filter((c) => c.isNew));
    const newFoils = once(priced.filter((c) => c.newFoil));
    return {
      cardCount: all.length,
      value: all.reduce((sum, c) => sum + (c.priceCents ?? 0), 0),
      newCount: newCards.length,
      newFoilCount: newFoils.length,
      newCards,
      newFoils,
      rarities,
      best: priced.slice(0, BEST_PULLS_SHOWN),
    };
  }, [boosters]);

  if (playing) {
    const shown = boosters.length;
    const cards = shown > 0 ? boosters[shown - 1] : null;
    return (
      <section className="opening">
        <div className="opening-bar batch-bar">
          <span>{t("batch.progress", { current: Math.max(shown, 1), total: run.total })}</span>
          <span className="batch-progress" role="progressbar" aria-valuemin={0} aria-valuemax={run.total} aria-valuenow={shown}>
            <span style={{ width: `${(100 * shown) / Math.max(run.total, 1)}%` }} />
          </span>
          <label className="batch-delay">
            <span>{t("batch.delay", { seconds: delay.toLocaleString(undefined, { minimumFractionDigits: 1 }) })}</span>
            <input
              type="range"
              min={0}
              max={DELAY_MAX_S}
              step={DELAY_STEP_S}
              value={delay}
              onChange={(e) => changeDelay(Number(e.target.value))}
            />
          </label>
          <button disabled={rushing} onClick={() => setPaused((p) => !p)}>
            {paused ? t("batch.resume") : t("batch.pause")}
          </button>
          <button disabled={rushing} onClick={() => setRushing(true)}>{t("batch.skip")}</button>
        </div>
        {/* Dealt face up at once, like a quick opening (a click opens a card's details: handy
            while paused). */}
        {cards && (
          <div className="cards compact batch-deal" key={shown}>
            {cards.map((card, i) => (
              <div key={`${card.uuid}-${i}`} className={isBigCard(card) ? "big" : ""} style={{ "--i": i } as CSSProperties}>
                <FlatCard card={card} lazy={false} onClick={() => onDetails(card)} />
              </div>
            ))}
          </div>
        )}
        <SeenBoosters boosters={boosters} count={Math.max(0, shown - 1)} onDetails={onDetails} />
      </section>
    );
  }

  return (
    <section className="opening">
      <div className="opening-bar">
        <strong>{t("batch.summary", { boosters: boosters.length, cards: summary.cardCount })}</strong>
        <strong className="total">{tx("batch.total", { amount: money(summary.value) })}</strong>
        <span>{t("batch.newCards", { count: summary.newCount })}</span>
        {summary.newFoilCount > 0 && <span>{t("batch.newFoils", { count: summary.newFoilCount })}</span>}
      </div>
      <p className="hint">
        {Object.entries(summary.rarities)
          .map(([rarity, n]) => plural(RARITY_COUNTS[rarity] ?? "rarity.other", n))
          .join(" · ")}
      </p>
      <h3>{t("batch.bestPulls")}</h3>
      <div className="cards compact">
        {summary.best.map((card, i) => (
          <FlatCard key={`${card.uuid}-${i}`} card={card} lazy={false} onClick={() => onDetails(card)} />
        ))}
      </div>
      {/* Everything new in the batch at a glance, without looking through each booster. */}
      {/* Folded at first (a batch of 100 can hold hundreds): unfolded with a click. */}
      {summary.newCards.length > 0 && (
        <details className="batch-news">
          <summary>{t("batch.newCards", { count: summary.newCount })}</summary>
          <div className="cards batch-cards">
            {summary.newCards.map(({ card, shown }) => (
              <FlatCard key={`${card.uuid}:${card.foil}`} card={shown} onClick={() => onDetails(card)} />
            ))}
          </div>
        </details>
      )}
      {summary.newFoils.length > 0 && (
        <details className="batch-news">
          <summary>{t("batch.newFoils", { count: summary.newFoilCount })}</summary>
          <div className="cards batch-cards">
            {summary.newFoils.map(({ card, shown }) => (
              <FlatCard key={`${card.uuid}:${card.foil}`} card={shown} onClick={() => onDetails(card)} />
            ))}
          </div>
        </details>
      )}
      <SeenBoosters boosters={boosters} count={boosters.length} onDetails={onDetails} />
    </section>
  );
}
