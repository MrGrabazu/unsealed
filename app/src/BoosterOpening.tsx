import { useEffect, useState } from "react";
import type { OpenedCard } from "./api";
import { Card } from "./Card";
import { useI18n } from "./i18n";
import { useMoney } from "./money";

/** Reveals the cards of an opened booster one by one. */
export function BoosterOpening({ cards }: { cards: OpenedCard[] }) {
  const money = useMoney();
  const { t } = useI18n();
  const [revealed, setRevealed] = useState(0);

  // A new booster starts face down.
  useEffect(() => setRevealed(0), [cards]);

  const done = revealed >= cards.length;
  const total = cards.reduce((sum, c) => sum + (c.priceCents ?? 0), 0);

  return (
    <section className="opening">
      <div className="opening-bar">
        <span>{t("open.revealed", { revealed, total: cards.length })}</span>
        {!done && <button onClick={() => setRevealed(cards.length)}>{t("open.revealAll")}</button>}
        {done && <strong className="total">{t("open.boosterValue", { amount: money(total) })}</strong>}
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
      {!done && <p className="hint">{t("open.clickToReveal")}</p>}
    </section>
  );
}
