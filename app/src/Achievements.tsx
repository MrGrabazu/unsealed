import { useEffect, useState } from "react";
import { AchievementCard } from "./AchievementCard";
import { getAchievements, type AchievementsView } from "./api";
import { rewardText } from "./boosterKinds";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { useMoney } from "./money";

type Filter = "all" | "unlocked" | "locked" | "secret";

/** The collection of achievement cards: what is unlocked, what remains, what is secret. */
export function Achievements({ refreshKey }: { refreshKey: number }) {
  const i18n = useI18n();
  const { t, lang, formatNumber } = i18n;
  const money = useMoney();
  const [view, setView] = useState<AchievementsView | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAchievements(lang)
      .then(setView)
      .catch((e) => setError(String(e)));
  }, [refreshKey, lang]);

  if (error) return <p className="error">{error}</p>;
  if (!view) return <Loading label={t("achievements.loading")} />;

  const shown = view.items.filter((a) => {
    switch (filter) {
      case "unlocked":
        return a.unlocked;
      case "locked":
        return !a.unlocked;
      case "secret":
        return a.secret;
      default:
        return true;
    }
  });
  const percent = view.total === 0 ? 0 : (view.unlockedCount * 100) / view.total;

  return (
    <section>
      <div className="controls">
        <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
          <option value="all">{t("achievements.all")}</option>
          <option value="unlocked">{t("achievements.unlocked")}</option>
          <option value="locked">{t("achievements.locked")}</option>
          <option value="secret">{t("achievements.secret")}</option>
        </select>
      </div>
      <div className="progress">
        <div className="progress-bar" style={{ width: `${percent}%` }} />
      </div>
      <p className="progress-label">
        {t("achievements.progress", {
          unlocked: view.unlockedCount,
          total: view.total,
          left: view.total - view.unlockedCount,
        })}
        {view.secretRemaining > 0 && t("achievements.secretLeft", { count: view.secretRemaining })}
      </p>
      <div className="ach-grid">
        {shown.map((a) => {
          const reward = a.reward && t("achievement.reward", { reward: rewardText(i18n, money, a.reward) });
          return (
            <div key={a.id} className="ach-item">
              <AchievementCard item={a} />
              <div className="ach-caption">
                {a.unlocked && reward && <span className="gain">{reward}</span>}
                {!a.unlocked && !a.secret && reward && <span className="hint">{reward}</span>}
                {!a.unlocked && a.progress && (
                  <span className="hint">
                    {formatNumber(a.progress.current)} / {formatNumber(a.progress.goal)}
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
