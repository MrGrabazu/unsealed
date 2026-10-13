import { useEffect, useState, type ReactNode } from "react";
import { getStats, type OpeningView, type StatsView } from "./api";
import { kindLabel } from "./boosterKinds";
import { Card } from "./Card";
import { Loading } from "./Loading";
import { useMoney } from "./money";

const noop = () => {};

const formatDate = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

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
  return (
    <div className="stat-panel">
      <span className="stat-label">{title}</span>
      <strong className="stat-value">{money(opening.valueCents)}</strong>
      <span>
        {opening.setName} · {kindLabel(opening.kind)}
      </span>
      <span className="hint">
        {formatDate(opening.openedAt)}
        {opening.bestCard && ` · best card: ${opening.bestCard.name}${opening.bestCard.foil ? " (foil)" : ""}`}
      </span>
    </div>
  );
}

/** Personal statistics: what was opened, spent and earned, and the best pulls of all time. */
export function Stats({ refreshKey }: { refreshKey: string }) {
  const money = useMoney();
  const [stats, setStats] = useState<StatsView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getStats()
      .then(setStats)
      .catch((e) => setError(String(e)));
  }, [refreshKey]);

  if (error) return <p className="error">{error}</p>;
  if (!stats) return <Loading label="Loading statistics…" />;
  if (stats.boostersOpened === 0) {
    return <p className="hint">No booster opened yet. Your statistics will show up here.</p>;
  }

  const s = stats;
  // Pulled value against what the boosters cost: below 100 %, opening lost money (as it usually does).
  const returnPercent = s.spentOnBoostersCents === 0 ? null : (s.valuePulledCents * 100) / s.spentOnBoostersCents;
  const netWorth = s.balanceCents + s.collectionValueCents + s.sealedValueCents;

  return (
    <section className="stats">
      <div className="stat-grid">
        <Tile label="Boosters opened">{s.boostersOpened.toLocaleString()}</Tile>
        <Tile label="Cards pulled">
          {s.cardsPulled.toLocaleString()}
          <span className="hint stat-sub"> · {s.foilsPulled.toLocaleString()} foil</span>
        </Tile>
        <Tile label="Value pulled" hint="Market value of the cards when they were pulled">
          {money(s.valuePulledCents)}
        </Tile>
        <Tile label="Average booster">{money(Math.round(s.valuePulledCents / s.boostersOpened))}</Tile>
        <Tile label="Spent on boosters">{money(s.spentOnBoostersCents)}</Tile>
        <Tile label="Return on boosters" hint="Value pulled compared with what the boosters cost">
          {returnPercent === null ? (
            "—"
          ) : (
            <span className={returnPercent >= 100 ? "gain" : "loss"}>{returnPercent.toFixed(0)}%</span>
          )}
        </Tile>
        <Tile label="Earned from sales" hint="Cards and sealed boosters sold to the Boosterium">
          {money(s.earnedFromCardsCents + s.earnedFromBoostersCents)}
        </Tile>
        <Tile label="Rewards and help" hint="Achievement rewards and daily help">
          {money(s.rewardsCents + s.charityCents)}
        </Tile>
        <Tile label="Distinct cards owned">{s.distinctCards.toLocaleString()}</Tile>
        <Tile label="Collection value" hint="Today's market value of the cards you own">
          {money(s.collectionValueCents)}
        </Tile>
        <Tile label="Net worth" hint="Balance + collection at market value + sealed boosters if sold today">
          {money(netWorth)}
        </Tile>
      </div>

      <div className="stat-panels">
        {s.bestBooster && <OpeningPanel title="Best booster ever" opening={s.bestBooster} />}
        {s.worstBooster && s.boostersOpened > 1 && <OpeningPanel title="Worst booster ever" opening={s.worstBooster} />}
      </div>

      {s.bestPulls.length > 0 && (
        <>
          <h3>Best pulls of all time</h3>
          <p className="hint">Priced when they were pulled.</p>
          <div className="cards compact">
            {s.bestPulls.map((p, i) => (
              <div key={`${p.card.uuid}-${p.openedAt}-${i}`} title={formatDate(p.openedAt)}>
                <Card card={p.card} faceUp clickable={false} onReveal={noop} />
              </div>
            ))}
          </div>
        </>
      )}

      <h3>By booster</h3>
      <table className="stat-table">
        <thead>
          <tr>
            <th>Booster</th>
            <th>Opened</th>
            <th>Total value</th>
            <th>Average</th>
            <th>Best</th>
          </tr>
        </thead>
        <tbody>
          {s.boosters.map((b) => (
            <tr key={`${b.setCode}/${b.kind}`}>
              <td>
                {b.setName} · {kindLabel(b.kind)}
              </td>
              <td>{b.boosters.toLocaleString()}</td>
              <td>{money(b.valueCents)}</td>
              <td>{money(b.averageCents)}</td>
              <td>{money(b.bestValueCents)}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>Latest openings</h3>
      <table className="stat-table">
        <thead>
          <tr>
            <th>Date</th>
            <th>Booster</th>
            <th>Value</th>
            <th>Best card</th>
          </tr>
        </thead>
        <tbody>
          {s.recentOpenings.map((o) => (
            <tr key={o.id}>
              <td>{formatDate(o.openedAt)}</td>
              <td>
                {o.setName} · {kindLabel(o.kind)}
              </td>
              <td>{money(o.valueCents)}</td>
              <td>
                {o.bestCard
                  ? `${o.bestCard.name}${o.bestCard.foil ? " (foil)" : ""} · ${money(o.bestCard.priceCents ?? 0)}`
                  : "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
