import { useEffect, useState, type ReactNode } from "react";
import { getStats, type OpenedCard, type OpeningView, type StatsView } from "./api";
import { kindLabel } from "./boosterKinds";
import { Card } from "./Card";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { useMoney } from "./money";

const noop = () => {};

function Tile({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <div className="stat-tile" title={hint}>
      <span className="stat-label">{label}</span>
      <strong className="stat-value">{children}</strong>
    </div>
  );
}

function OpeningPanel({ title, opening }: { title: string; opening: OpeningView }) {
  const money = useMoney();
  const { t, formatDate } = useI18n();
  return (
    <div className="stat-panel">
      <span className="stat-label">{title}</span>
      <strong className="stat-value">{money(opening.valueCents)}</strong>
      <span>
        {opening.setName} · {kindLabel(t, opening.kind)}
      </span>
      <span className="hint">
        {formatDate(opening.openedAt)}
        {opening.bestCard && t("stats.bestCard", { card: cardName(t, opening.bestCard) })}
      </span>
    </div>
  );
}

/** A card's name, marked when foil. */
const cardName = (t: ReturnType<typeof useI18n>["t"], card: OpenedCard) =>
  card.foil ? `${card.name}${t("stats.foilSuffix")}` : card.name;

/** Personal statistics: what was opened, spent and earned, and the best pulls of all time. */
export function Stats({ refreshKey }: { refreshKey: string }) {
  const money = useMoney();
  const { t, formatDate, formatNumber } = useI18n();
  const [stats, setStats] = useState<StatsView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getStats()
      .then(setStats)
      .catch((e) => setError(String(e)));
  }, [refreshKey]);

  if (error) return <p className="error">{error}</p>;
  if (!stats) return <Loading label={t("stats.loading")} />;
  if (stats.boostersOpened === 0) {
    return <p className="hint">{t("stats.empty")}</p>;
  }

  const s = stats;
  // Pulled value against what the boosters cost: below 100 %, opening lost money (as it usually does).
  const returnPercent = s.spentOnBoostersCents === 0 ? null : (s.valuePulledCents * 100) / s.spentOnBoostersCents;
  const netWorth = s.balanceCents + s.collectionValueCents + s.sealedValueCents;

  return (
    <section className="stats">
      <div className="stat-grid">
        <Tile label={t("stats.boostersOpened")}>{formatNumber(s.boostersOpened)}</Tile>
        <Tile label={t("stats.cardsPulled")}>
          {formatNumber(s.cardsPulled)}
          <span className="hint stat-sub">{t("stats.foils", { count: formatNumber(s.foilsPulled) })}</span>
        </Tile>
        <Tile label={t("stats.valuePulled")} hint={t("stats.valuePulledHint")}>
          {money(s.valuePulledCents)}
        </Tile>
        <Tile label={t("stats.average")}>{money(Math.round(s.valuePulledCents / s.boostersOpened))}</Tile>
        <Tile label={t("stats.spent")}>{money(s.spentOnBoostersCents)}</Tile>
        <Tile label={t("stats.return")} hint={t("stats.returnHint")}>
          {returnPercent === null ? (
            t("common.none")
          ) : (
            <span className={returnPercent >= 100 ? "gain" : "loss"}>{returnPercent.toFixed(0)}%</span>
          )}
        </Tile>
        <Tile label={t("stats.earned")} hint={t("stats.earnedHint")}>
          {money(s.earnedFromCardsCents + s.earnedFromBoostersCents)}
        </Tile>
        <Tile label={t("stats.rewards")} hint={t("stats.rewardsHint")}>
          {money(s.rewardsCents + s.charityCents)}
        </Tile>
        <Tile label={t("stats.distinct")}>{formatNumber(s.distinctCards)}</Tile>
        <Tile label={t("stats.collectionValue")} hint={t("stats.collectionValueHint")}>
          {money(s.collectionValueCents)}
        </Tile>
        <Tile label={t("stats.netWorth")} hint={t("stats.netWorthHint")}>
          {money(netWorth)}
        </Tile>
      </div>

      <div className="stat-panels">
        {s.bestBooster && <OpeningPanel title={t("stats.best")} opening={s.bestBooster} />}
        {s.worstBooster && s.boostersOpened > 1 && <OpeningPanel title={t("stats.worst")} opening={s.worstBooster} />}
      </div>

      {s.bestPulls.length > 0 && (
        <>
          <h3>{t("stats.bestPulls")}</h3>
          <p className="hint">{t("stats.bestPullsHint")}</p>
          <div className="cards compact">
            {s.bestPulls.map((p, i) => (
              <div key={`${p.card.uuid}-${p.openedAt}-${i}`} title={formatDate(p.openedAt)}>
                <Card card={p.card} faceUp clickable={false} onReveal={noop} />
              </div>
            ))}
          </div>
        </>
      )}

      <h3>{t("stats.byBooster")}</h3>
      <table className="stat-table">
        <thead>
          <tr>
            <th>{t("stats.colBooster")}</th>
            <th>{t("stats.colOpened")}</th>
            <th>{t("stats.colTotal")}</th>
            <th>{t("stats.colAverage")}</th>
            <th>{t("stats.colBest")}</th>
          </tr>
        </thead>
        <tbody>
          {s.boosters.map((b) => (
            <tr key={`${b.setCode}/${b.kind}`}>
              <td>
                {b.setName} · {kindLabel(t, b.kind)}
              </td>
              <td>{formatNumber(b.boosters)}</td>
              <td>{money(b.valueCents)}</td>
              <td>{money(b.averageCents)}</td>
              <td>{money(b.bestValueCents)}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>{t("stats.latest")}</h3>
      <table className="stat-table">
        <thead>
          <tr>
            <th>{t("stats.colDate")}</th>
            <th>{t("stats.colBooster")}</th>
            <th>{t("stats.colValue")}</th>
            <th>{t("stats.colBestCard")}</th>
          </tr>
        </thead>
        <tbody>
          {s.recentOpenings.map((o) => (
            <tr key={o.id}>
              <td>{formatDate(o.openedAt)}</td>
              <td>
                {o.setName} · {kindLabel(t, o.kind)}
              </td>
              <td>{money(o.valueCents)}</td>
              <td>
                {o.bestCard ? (
                  <>
                    {cardName(t, o.bestCard)} · {money(o.bestCard.priceCents ?? 0)}
                  </>
                ) : (
                  t("common.none")
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
