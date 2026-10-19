import type { SettledBet, SkippedBet } from "./api";
import { betKey, describeBet } from "./bets";
import { useI18n } from "./i18n";
import { Money, useMoneyText } from "./money";

/** One bet, over the boosters it was placed on (a batch opening repeats the same bets). */
interface Line {
  bet: SettledBet;
  placed: number;
  won: number;
  paidCents: number;
}

/** What the bets on the boosters just opened came to: each bet won or lost (how many times,
 * in a batch), and the balance of it all. Shown once every card is seen. */
export function BetResults({
  bets,
  skipped,
  cardName,
}: {
  bets: SettledBet[];
  /** Bets left out, and why: no stake was taken. */
  skipped: SkippedBet[];
  cardName: (uuid: string) => string;
}) {
  const { t, plural, tx } = useI18n();
  const moneyText = useMoneyText();
  if (bets.length === 0 && skipped.length === 0) return null;

  const lines = new Map<string, Line>();
  for (const bet of bets) {
    const key = betKey(bet.bet);
    const line = lines.get(key) ?? { bet, placed: 0, won: 0, paidCents: 0 };
    line.placed += 1;
    line.won += bet.won ? 1 : 0;
    line.paidCents += bet.payoutCents;
    lines.set(key, line);
  }
  const staked = bets.reduce((n, b) => n + b.stakeCents, 0);
  const paid = bets.reduce((n, b) => n + b.payoutCents, 0);

  return (
    <section className="bet-results">
      <h3>🎲 {t("bet.results")}</h3>
      <ul>
        {[...lines.entries()].map(([key, line]) => (
          <li key={key} className={line.won > 0 ? "won" : "lost"}>
            <span className="bet-line-name">{describeBet(line.bet.bet, { t, plural }, moneyText, cardName)}</span>
            <span className="hint">
              {line.placed > 1 ? t("bet.wonOf", { won: line.won, placed: line.placed }) : line.won > 0 ? t("bet.won") : t("bet.lost")}
            </span>
            <strong>{line.paidCents > 0 ? <Money cents={line.paidCents} /> : <span className="hint">—</span>}</strong>
          </li>
        ))}
      </ul>
      {bets.length > 0 && (
        <p className={`bet-net ${paid >= staked ? "gain" : "loss"}`}>
          {tx("bet.net", { staked: <Money cents={staked} />, paid: <Money cents={paid} /> })}
        </p>
      )}
      {skipped.length > 0 && (
        <div className="bet-skipped">
          <p className="hint">{t("bet.skipped")}</p>
          <ul>
            {skipped.map(({ bet, reason }) => (
              <li key={`${betKey(bet)}:${reason}`} className="hint">
                {describeBet(bet, { t, plural }, moneyText, cardName)} — {t(`bet.skip.${reason}`)}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
