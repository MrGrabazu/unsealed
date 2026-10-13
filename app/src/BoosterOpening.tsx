import { useEffect, useState } from "react";
import type { OpenedCard } from "./api";
import { Card } from "./Card";
import { useMoney } from "./money";

/** Reveals the cards of an opened booster one by one. */
export function BoosterOpening({ cards }: { cards: OpenedCard[] }) {
  const money = useMoney();
  const [revealed, setRevealed] = useState(0);

  // A new booster starts face down.
  useEffect(() => setRevealed(0), [cards]);

  const done = revealed >= cards.length;
  const total = cards.reduce((sum, c) => sum + (c.priceCents ?? 0), 0);

  return (
    <section className="opening">
      <div className="opening-bar">
        <span>
          {revealed} / {cards.length} revealed
        </span>
        {!done && <button onClick={() => setRevealed(cards.length)}>Reveal all</button>}
        {done && <strong className="total">Booster value: {money(total)}</strong>}
      </div>
      <div className="cards">
        {cards.map((card, i) => (
          <Card
            key={`${card.uuid}-${i}`}
            card={card}
            faceUp={i < revealed}
            clickable={i === revealed}
            onReveal={() => setRevealed(i + 1)}
          />
        ))}
      </div>
      {!done && <p className="hint">Click the highlighted card to reveal it.</p>}
    </section>
  );
}
