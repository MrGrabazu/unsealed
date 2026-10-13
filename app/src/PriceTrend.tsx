import { useI18n } from "./i18n";
import { useMoney } from "./money";

const WIDTH = 90;
const HEIGHT = 26;

/** Sparkline of a booster's recent prices with the change since the previous price update. */
export function PriceTrend({ history }: { history: number[] }) {
  const money = useMoney();
  const { t } = useI18n();
  if (history.length < 2) {
    return <span className="hint trend">{t("trend.tracking")}</span>;
  }
  const last = history[history.length - 1];
  const previous = history[history.length - 2];
  const change = last - previous;
  const min = Math.min(...history);
  const range = Math.max(...history) - min || 1;
  const points = history
    .map((p, i) => {
      const x = (i * WIDTH) / (history.length - 1);
      const y = HEIGHT - 2 - ((p - min) * (HEIGHT - 4)) / range;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  const percent = previous === 0 ? 0 : (change * 100) / previous;
  const cls = change > 0 ? "gain" : change < 0 ? "loss" : "hint";
  return (
    <span
      className="trend"
      title={t("trend.title", { count: history.length, first: money(history[0]), last: money(last) })}
    >
      <svg width={WIDTH} height={HEIGHT} viewBox={`0 0 ${WIDTH} ${HEIGHT}`} aria-hidden="true">
        <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" className={cls} />
      </svg>
      <span className={cls}>
        {change > 0 ? "▲" : change < 0 ? "▼" : "•"} {percent >= 0 ? "+" : "−"}
        {Math.abs(percent).toFixed(1)}%
      </span>
    </span>
  );
}
