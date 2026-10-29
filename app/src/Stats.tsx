import { useEffect, useState, type ReactNode } from "react";
import { getStats, type OpenedCard, type OpeningView, type StatsView } from "./api";
import { kindLabel } from "./boosterKinds";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { Money, useMoney, useMoneyText } from "./money";
import { FoilSheen, foilLabel } from "./foils";
import { ShareButton } from "./Share";
import { drawStatsShare, type StatsShare } from "./sharePicture";

/** A big number in its own medallion. */
function Tile({ label, children, sub, hint }: { label: string; children: ReactNode; sub?: ReactNode; hint?: string }) {
  return (
    <div className="frame stat-medal" title={hint}>
      <small>{label}</small>
      <b>{children}</b>
      {sub && <em>{sub}</em>}
    </div>
  );
}

/** A card's name, marked when foil. */
const cardName = (t: ReturnType<typeof useI18n>["t"], card: OpenedCard) =>
  card.foil ? `${card.name}${t("stats.foilSuffix")}` : card.name;

const CHART_W = 1000;
const CHART_H = 150;

/** The latest openings' values, oldest first, as gilded bars; the best one marked. */
function OpeningsChart({ openings }: { openings: OpeningView[] }) {
  const moneyText = useMoneyText();
  const { t, formatDate } = useI18n();
  const shown = [...openings].reverse();
  const max = Math.max(1, ...shown.map((o) => o.valueCents));
  const average = shown.reduce((n, o) => n + o.valueCents, 0) / Math.max(1, shown.length);
  const step = CHART_W / Math.max(1, shown.length);
  const barW = Math.max(4, step - 6);
  const best = shown.reduce((b, o, i) => (o.valueCents > shown[b].valueCents ? i : b), 0);
  const y = (cents: number) => CHART_H - 4 - (cents * (CHART_H - 26)) / max;
  return (
    <svg className="stat-chart" viewBox={`0 0 ${CHART_W} ${CHART_H}`} preserveAspectRatio="none" role="img" aria-label={t("stats.latest")}>
      {[0.25, 0.5, 0.75].map((f) => (
        <line key={f} x1="0" x2={CHART_W} y1={y(max * f)} y2={y(max * f)} className="grid" />
      ))}
      {shown.map((o, i) => {
        const top = y(o.valueCents);
        const x = i * step + (step - barW) / 2;
        return (
          <g key={o.id}>
            <path
              className={`bar-mark ${i === best ? "best" : ""}`}
              d={`M${x},${CHART_H - 4} V${top + 4} q0,-4 4,-4 h${barW - 8} q4,0 4,4 V${CHART_H - 4} Z`}
            />
            {/* The hit target is the whole column, larger than the bar. */}
            <rect x={i * step} y="0" width={step} height={CHART_H} fill="transparent">
              <title>
                {`${formatDate(o.openedAt)} · ${o.setName} · ${kindLabel(t, o.kind)} · ${moneyText(o.valueCents)}`}
                {o.bestCard ? ` · ${cardName(t, o.bestCard)}` : ""}
              </title>
            </rect>
          </g>
        );
      })}
      <line x1="0" x2={CHART_W} y1={y(average)} y2={y(average)} className="average" />
      <text x={best * step + step / 2} y={Math.max(12, y(shown[best]?.valueCents ?? 0) - 8)} className="best-label" textAnchor="middle">
        {moneyText(shown[best]?.valueCents ?? 0)}
      </text>
    </svg>
  );
}

/** Personal statistics: the big numbers, what the boosters gave on average, the finest pulls of
 * all time, and the latest openings. */
export function Stats({ refreshKey }: { refreshKey: string }) {
  const money = useMoney();
  const moneyText = useMoneyText();
  const { t, tx, formatDate, formatNumber } = useI18n();
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
    return (
      <section className="frame empty-state">
        <div className="cartouche">{t("tab.stats")}</div>
        <p className="hint">{t("stats.empty")}</p>
      </section>
    );
  }

  const s = stats;
  // Pulled value against what the boosters cost: below 100 %, opening lost money (as it usually does).
  const returnPercent = s.spentOnBoostersCents === 0 ? null : (s.valuePulledCents * 100) / s.spentOnBoostersCents;
  const netWorth = s.balanceCents + s.collectionValueCents + s.sealedValueCents;
  const byAverage = [...s.boosters].sort((a, b) => b.averageCents - a.averageCents).slice(0, 6);
  const topAverage = Math.max(1, ...byAverage.map((b) => b.averageCents));
  const betsNet = s.betsWonCents - s.betsStakedCents;

  /** The picture to share: the big numbers and the finest pulls. */
  const statsShare = (): StatsShare => ({
    title: t("share.statsTitle"),
    tiles: [
      {
        label: t("stats.boostersOpened"),
        value: formatNumber(s.boostersOpened),
        sub: `${t("stats.cardsPulledShort", { count: formatNumber(s.cardsPulled) })}${t("stats.foils", { count: formatNumber(s.foilsPulled) })}`,
      },
      {
        label: t("stats.valuePulled"),
        value: moneyText(s.valuePulledCents),
        sub: returnPercent === null ? null : t("stats.returnShort", { percent: returnPercent.toFixed(0) }),
      },
      {
        label: t("stats.collectionValue"),
        value: moneyText(s.collectionValueCents),
        sub: t("share.distinctCards", { count: formatNumber(s.distinctCards) }),
      },
      ...(s.bestBooster
        ? [{ label: t("stats.best"), value: moneyText(s.bestBooster.valueCents), sub: `${s.bestBooster.setName} · ${kindLabel(t, s.bestBooster.kind)}` }]
        : []),
    ],
    pullsLabel: t("stats.bestPulls"),
    pulls: s.bestPulls.slice(0, 5).map((p) => ({
      card: {
        imageUrl: p.card.imageUrl,
        name: p.card.name,
        foil: p.card.foil,
        foilLabel: p.card.foil ? (foilLabel(p.card.foilKind) ?? t(p.card.rarity === "art" ? "collection.signedBadge" : "collection.foilBadge")) : null,
        newLabel: null,
      },
      priceText: p.card.priceCents == null ? "—" : moneyText(p.card.priceCents),
    })),
    tagline: t("share.taglineStats"),
  });

  return (
    <div className="stats-screen">
      <ShareButton make={() => drawStatsShare(statsShare())} fileName={t("share.statsTitle")} round className="stats-share" />
      <div className="stat-medals">
        <Tile
          label={t("stats.boostersOpened")}
          sub={`${t("stats.cardsPulledShort", { count: formatNumber(s.cardsPulled) })}${t("stats.foils", { count: formatNumber(s.foilsPulled) })}`}
        >
          {formatNumber(s.boostersOpened)}
        </Tile>
        <Tile
          label={t("stats.spent")}
          hint={t("stats.returnHint")}
          sub={
            returnPercent === null ? undefined : (
              <span className={returnPercent >= 100 ? "gain" : "loss"}>{t("stats.returnShort", { percent: returnPercent.toFixed(0) })}</span>
            )
          }
        >
          <Money cents={s.spentOnBoostersCents} />
        </Tile>
        <Tile
          label={t("stats.collectionValue")}
          hint={t("stats.collectionValueHint")}
          sub={
            <>
              {tx("stats.collectionSellShort", { amount: money(s.collectionSellValueCents) })}
              <br />
              {tx("stats.netWorthShort", { amount: money(netWorth) })}
            </>
          }
        >
          <Money cents={s.collectionValueCents} />
        </Tile>
        {s.bestBooster && (
          <Tile label={t("stats.best")} sub={`${s.bestBooster.setName} · ${kindLabel(t, s.bestBooster.kind)}`}>
            <Money cents={s.bestBooster.valueCents} />
          </Tile>
        )}
      </div>

      <div className="stat-twocol">
        <section className="frame">
          <div className="cartouche">{t("stats.averageByBooster")}</div>
          <div className="stat-bars">
            {byAverage.map((b) => (
              <div key={`${b.setCode}/${b.kind}`} className="stat-bar-row" title={t("stats.boostersCount", { count: formatNumber(b.boosters) })}>
                <span className="stat-bar-name">
                  {b.setName}
                  <small>
                    {kindLabel(t, b.kind)} · {t("stats.boostersCount", { count: formatNumber(b.boosters) })}
                  </small>
                </span>
                <div className="bar">
                  <i style={{ width: `${(b.averageCents * 100) / topAverage}%` }} />
                </div>
                <b>
                  <Money cents={b.averageCents} />
                </b>
              </div>
            ))}
          </div>
        </section>
        {s.bestPulls.length > 0 && (
          <section className="frame">
            <div className="cartouche">{t("stats.bestPulls")}</div>
            <div className="stat-pulls">
              {s.bestPulls.slice(0, 5).map((p, i) => (
                <div key={`${p.card.uuid}-${p.openedAt}-${i}`} title={`${cardName(t, p.card)} · ${formatDate(p.openedAt)}`}>
                  <div className={`thumb pull-thumb ${p.card.rarity}`}>
                    <CardImage front={p.card.imageUrl} back={p.card.backImageUrl} alt={p.card.name} lazy />
                    {p.card.foil && <FoilSheen rarity={p.card.rarity} kind={p.card.foilKind} />}
                  </div>
                  <span className="pull-value">{p.card.priceCents == null ? "—" : <Money cents={p.card.priceCents} />}</span>
                </div>
              ))}
            </div>
          </section>
        )}
      </div>

      {s.recentOpenings.length > 1 && (
        <section className="frame">
          <div className="cartouche">{t("stats.latestChart", { count: s.recentOpenings.length })}</div>
          <p className="meta stat-chart-legend">{t("stats.latestChartHint")}</p>
          <OpeningsChart openings={s.recentOpenings} />
        </section>
      )}

      <div className="stat-twocol">
        <section className="frame">
          <div className="cartouche">{t("stats.accounts")}</div>
          <div className="ready-line">
            <span>{t("stats.valuePulled")}</span>
            <b>
              <Money cents={s.valuePulledCents} />
            </b>
          </div>
          <div className="ready-line">
            <span>{t("stats.average")}</span>
            <b>
              <Money cents={Math.round(s.valuePulledCents / s.boostersOpened)} />
            </b>
          </div>
          <div className="ready-line">
            <span>{t("stats.earned")}</span>
            <b>
              <Money cents={s.earnedFromCardsCents + s.earnedFromBoostersCents} />
            </b>
          </div>
          <div className="ready-line">
            <span>{t("stats.rewards")}</span>
            <b>
              <Money cents={s.rewardsCents + s.charityCents} />
            </b>
          </div>
          {s.betsStakedCents > 0 && (
            <div className="ready-line" title={t("stats.betsHint")}>
              <span>{t("stats.bets")}</span>
              <b className={betsNet >= 0 ? "gain" : "loss"}>
                {betsNet >= 0 ? "+" : "−"}
                <Money cents={Math.abs(betsNet)} />
              </b>
            </div>
          )}
          <div className="ready-line">
            <span>{t("stats.distinct")}</span>
            <b>{formatNumber(s.distinctCards)}</b>
          </div>
          {s.worstBooster && s.boostersOpened > 1 && (
            <div className="ready-line">
              <span>{t("stats.worst")}</span>
              <b>
                {s.worstBooster.setName} · <Money cents={s.worstBooster.valueCents} />
              </b>
            </div>
          )}
        </section>
        <section className="frame">
          <div className="cartouche">{t("stats.byBooster")}</div>
          <table className="ledger stat-ledger">
            <thead>
              <tr>
                <th>{t("stats.colBooster")}</th>
                <th>{t("stats.colOpened")}</th>
                <th>{t("stats.colTotal")}</th>
                <th>{t("stats.colBest")}</th>
              </tr>
            </thead>
            <tbody>
              {s.boosters.map((b) => (
                <tr key={`${b.setCode}/${b.kind}`}>
                  <td>
                    {b.setName}
                    <span className="ledger-sub" style={{ marginLeft: 0 }}>
                      {kindLabel(t, b.kind)}
                    </span>
                  </td>
                  <td>{formatNumber(b.boosters)}</td>
                  <td className="ledger-amount">
                    <Money cents={b.valueCents} />
                  </td>
                  <td className="ledger-amount">
                    <Money cents={b.bestValueCents} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </div>
    </div>
  );
}
