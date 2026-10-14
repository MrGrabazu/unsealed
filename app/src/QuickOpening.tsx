import { useEffect } from "react";
import type { OpenedCard, SetSummary } from "./api";
import { kindLabel } from "./boosterKinds";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { useMoney } from "./money";
import { RevealedCards } from "./RevealedCards";
import { bestCard, revealSound, sounds } from "./sound";

/** Delay between two cards dealt (ms), as in `.revealed-row.final`. */
const DEAL_MS = 30;

/** The quick opening, for the impatient: no tearing, no pile, the cards dealt at once (in a
 * real booster's order), with the booster's value. */
export function QuickOpening({
  set,
  kind,
  cards,
  onDetails,
}: {
  set: SetSummary | undefined;
  kind: string;
  cards: OpenedCard[] | null;
  onDetails: (card: OpenedCard) => void;
}) {
  const { t, tx } = useI18n();
  const money = useMoney();

  // The rip, then the best card's sound once every card has landed.
  useEffect(() => {
    if (!cards) return;
    sounds.tear();
    const timer = setTimeout(() => revealSound(bestCard(cards)), cards.length * DEAL_MS + 350);
    return () => clearTimeout(timer);
  }, [cards]);

  if (!cards) return <Loading />;
  const value = cards.reduce((sum, c) => sum + (c.priceCents ?? 0), 0);
  return (
    <section className="pack-opening">
      <div className="opening-bar">
        <span>
          {set?.name} · {kindLabel(t, kind)}
        </span>
        <strong className="total">{tx("open.boosterValue", { amount: money(value) })}</strong>
      </div>
      <RevealedCards cards={cards} final onDetails={onDetails} />
      <p className="hint scene-hint">{t("open.details")}</p>
    </section>
  );
}
