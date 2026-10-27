import { useEffect, useMemo, useState } from "react";
import type { OpenedCard } from "./api";
import { Card } from "./Card";
import { useI18n } from "./i18n";
import { Money, useMoney } from "./money";
import { smallImage } from "./images";

/** Time each ordinary booster stays on screen. */
const FAST_MS = 450;
/** Time a booster holding a big card stays on screen: the only place the animation slows down. */
const SLOW_MS = 2400;
/** Cards at or above this market price count as big. */
const BIG_PRICE_CENTS = 500;
/** Delay before the cards of a new booster flip, so the flip is visible. */
const FLIP_DELAY_MS = 80;
const BEST_PULLS_SHOWN = 8;

export const isBigCard = (c: OpenedCard) =>
  c.rarity === "mythic" || (c.priceCents ?? 0) >= BIG_PRICE_CENTS || (c.foil && c.rarity === "rare");

const noop = () => {};

/** Plural texts counting the cards of each rarity. */
const RARITY_COUNTS: Record<string, "rarity.common" | "rarity.uncommon" | "rarity.rare" | "rarity.mythic"> = {
  common: "rarity.common",
  uncommon: "rarity.uncommon",
  rare: "rarity.rare",
  mythic: "rarity.mythic",
};

/** A booster of the batch already seen, in the list under the one opening (the latest first):
 * its number, value, news and cards, small, each opening its details. */
function SeenBooster({ n, cards, onDetails }: { n: number; cards: OpenedCard[]; onDetails: (card: OpenedCard) => void }) {
  const { t } = useI18n();
  const value = cards.reduce((sum, c) => sum + (c.priceCents ?? 0), 0);
  const news = cards.filter((c) => c.isNew).length;
  return (
    <div className="batch-booster">
      <div className="batch-booster-head">
        <b>{t("batch.booster", { n })}</b>
        <Money cents={value} />
        {news > 0 && <span className="hint">{t("batch.newCards", { count: news })}</span>}
      </div>
      <div className="cards batch-cards">
        {cards.map((card, i) => (
          <div key={`${card.uuid}-${i}`} className={isBigCard(card) ? "big" : ""}>
            <Card
              card={{ ...card, imageUrl: smallImage(card.imageUrl), backImageUrl: smallImage(card.backImageUrl) }}
              faceUp
              clickable
              onReveal={() => onDetails(card)}
              lazy
            />
          </div>
        ))}
      </div>
    </div>
  );
}

/** The boosters of the batch seen so far (up to `count`), the latest first. */
function SeenBoosters({ boosters, count, onDetails }: { boosters: OpenedCard[][]; count: number; onDetails: (card: OpenedCard) => void }) {
  const { t } = useI18n();
  if (count <= 0) return null;
  const seen = boosters.slice(0, count).map((cards, i) => ({ n: i + 1, cards })).reverse();
  return (
    <>
      <h3>{t("batch.openings")}</h3>
      <div className="batch-boosters">
        {seen.map(({ n, cards }) => (
          <SeenBooster key={n} n={n} cards={cards} onDetails={onDetails} />
        ))}
      </div>
    </>
  );
}

/** Plays an accelerated opening of many boosters, then shows a summary of the haul. The
 * boosters already seen pile up under the one opening (the latest first), and stay under the
 * summary, to look at each draw. */
export function BatchOpening({
  boosters,
  onFinished,
  onDetails,
}: {
  boosters: OpenedCard[][];
  onFinished: () => void;
  onDetails: (card: OpenedCard) => void;
}) {
  const money = useMoney();
  const { t, tx, plural } = useI18n();
  const [index, setIndex] = useState(0);
  const [faceUp, setFaceUp] = useState(false);
  const [skipped, setSkipped] = useState(false);
  const playing = !skipped && index < boosters.length;

  // The summary is on screen: the achievements unlocked by the batch can show up.
  useEffect(() => {
    if (!playing) onFinished();
  }, [playing, onFinished]);

  // A new batch starts from the beginning.
  useEffect(() => {
    setIndex(0);
    setSkipped(false);
  }, [boosters]);

  useEffect(() => {
    if (!playing) return;
    setFaceUp(false);
    const flip = setTimeout(() => setFaceUp(true), FLIP_DELAY_MS);
    const next = setTimeout(
      () => setIndex((i) => i + 1),
      boosters[index].some(isBigCard) ? SLOW_MS : FAST_MS,
    );
    return () => {
      clearTimeout(flip);
      clearTimeout(next);
    };
  }, [index, playing, boosters]);

  const summary = useMemo(() => {
    const all = boosters.flat();
    const rarities: Record<string, number> = {};
    for (const c of all) rarities[c.rarity] = (rarities[c.rarity] ?? 0) + 1;
    const priced = [...all].sort((a, b) => (b.priceCents ?? 0) - (a.priceCents ?? 0));
    return {
      cardCount: all.length,
      value: all.reduce((sum, c) => sum + (c.priceCents ?? 0), 0),
      newCount: all.filter((c) => c.isNew).length,
      newFoilCount: all.filter((c) => c.newFoil).length,
      rarities,
      best: priced.slice(0, BEST_PULLS_SHOWN),
    };
  }, [boosters]);

  if (playing) {
    const cards = boosters[index];
    return (
      <section className="opening">
        <div className="opening-bar">
          <span>{t("batch.progress", { current: index + 1, total: boosters.length })}</span>
          <button onClick={() => setSkipped(true)}>{t("batch.skip")}</button>
        </div>
        <div className="cards compact" key={index}>
          {cards.map((card, i) => (
            <div key={`${card.uuid}-${i}`} className={isBigCard(card) ? "big" : ""}>
              <Card card={card} faceUp={faceUp} clickable={false} onReveal={noop} />
            </div>
          ))}
        </div>
        <SeenBoosters boosters={boosters} count={index} onDetails={onDetails} />
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
          <Card key={`${card.uuid}-${i}`} card={card} faceUp clickable onReveal={() => onDetails(card)} />
        ))}
      </div>
      <SeenBoosters boosters={boosters} count={boosters.length} onDetails={onDetails} />
    </section>
  );
}
