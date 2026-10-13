import { useEffect, useMemo, useState } from "react";
import type { OpenedCard } from "./api";
import { Card } from "./Card";
import { useMoney } from "./money";

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

/** Plays an accelerated opening of many boosters, then shows a summary of the haul. */
export function BatchOpening({ boosters }: { boosters: OpenedCard[][] }) {
  const money = useMoney();
  const [index, setIndex] = useState(0);
  const [faceUp, setFaceUp] = useState(false);
  const [skipped, setSkipped] = useState(false);
  const playing = !skipped && index < boosters.length;

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
      rarities,
      best: priced.slice(0, BEST_PULLS_SHOWN),
    };
  }, [boosters]);

  if (playing) {
    const cards = boosters[index];
    return (
      <section className="opening">
        <div className="opening-bar">
          <span>
            Booster {index + 1} / {boosters.length}
          </span>
          <button onClick={() => setSkipped(true)}>Skip to summary</button>
        </div>
        <div className="cards compact" key={index}>
          {cards.map((card, i) => (
            <div key={`${card.uuid}-${i}`} className={isBigCard(card) ? "big" : ""}>
              <Card card={card} faceUp={faceUp} clickable={false} onReveal={noop} />
            </div>
          ))}
        </div>
      </section>
    );
  }

  return (
    <section className="opening">
      <div className="opening-bar">
        <strong>
          {boosters.length} boosters opened · {summary.cardCount} cards
        </strong>
        <strong className="total">Total value: {money(summary.value)}</strong>
        <span>{summary.newCount} new</span>
      </div>
      <p className="hint">
        {Object.entries(summary.rarities)
          .map(([rarity, n]) => `${n} ${rarity}`)
          .join(" · ")}
      </p>
      <h3>Best pulls</h3>
      <div className="cards compact">
        {summary.best.map((card, i) => (
          <Card key={`${card.uuid}-${i}`} card={card} faceUp clickable={false} onReveal={noop} />
        ))}
      </div>
    </section>
  );
}
