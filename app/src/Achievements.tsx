import { useEffect, useState } from "react";
import { AchievementCard } from "./AchievementCard";
import { getAchievements, type AchievementsView, type AchievementView } from "./api";
import { rewardText } from "./boosterKinds";
import { useI18n, type I18n } from "./i18n";
import { Loading } from "./Loading";
import { useAmountsInText, useMoneyText } from "./money";

type Filter = "all" | "unlocked" | "locked" | "secret";

/** The collection of achievement cards: what is unlocked, what remains, what is secret. */
export function Achievements({ refreshKey }: { refreshKey: number }) {
  const i18n = useI18n();
  const { t, lang } = i18n;
  const [view, setView] = useState<AchievementsView | null>(null);
  // The achievement whose details are open.
  const [open, setOpen] = useState<AchievementView | null>(null);
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
        {shown.map((a) => (
          <div
            key={a.id}
            className="ach-item"
            role="button"
            tabIndex={0}
            title={t("achievements.open")}
            onClick={() => setOpen(a)}
            onKeyDown={(e) => e.key === "Enter" && setOpen(a)}
          >
            <AchievementCard item={a} />
          </div>
        ))}
      </div>
      {open && <AchievementDetails item={open} onClose={() => setOpen(null)} />}
    </section>
  );
}

/** An achievement in detail: its card, large, and in plain text its goal, progress, reward
 * and when it was unlocked. A locked secret keeps everything hidden but its rarity. */
function AchievementDetails({ item, onClose }: { item: AchievementView; onClose: () => void }) {
  const i18n = useI18n();
  const { t, formatNumber, formatDate } = i18n;
  const moneyText = useMoneyText();
  const amountsInText = useAmountsInText();
  const hidden = item.secret && !item.unlocked;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const progress = item.progress;
  const percent = progress ? Math.min(100, (progress.current * 100) / Math.max(1, progress.goal)) : 0;
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal details ach-details" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <button className="modal-close" onClick={onClose} aria-label={t("details.close")}>
          ×
        </button>
        {/* Laid out like a card's details: the card on the left, its text on the right. */}
        <div className="details-layout">
        <div className="ach-details-card">
          <AchievementCard item={item} />
        </div>
        <div className="details-info ach-details-info">
          <p className="hint">
            {t("achievements.cardNumber", { number: item.number })} · {rarityLabel(t, item.rarity)}
            {item.secret && ` · ${t("achievements.secret")}`}
          </p>
          <h2>{hidden ? t("achievements.secretName") : item.name}</h2>

          <h3>{t("achievements.goal")}</h3>
          <p className="ach-goal">{hidden ? t("achievements.secretText") : amountsInText(item.description ?? "")}</p>

          {progress && !item.unlocked && (
            <>
              <h3>{t("achievements.progressTitle")}</h3>
              <div className="progress">
                <div className="progress-bar" style={{ width: `${percent}%` }} />
              </div>
              <p className="progress-label">
                {formatNumber(progress.current)} / {formatNumber(progress.goal)} ({Math.floor(percent)} %)
              </p>
            </>
          )}

          {!hidden && (
            <>
              <h3>{t("achievements.rewardTitle")}</h3>
              <p>{item.reward ? rewardText(i18n, moneyText, item.reward) : t("achievements.noReward")}</p>
            </>
          )}

          <p className={item.unlocked ? "gain" : "hint"}>
            {item.unlocked && item.unlockedAt != null
              ? t("achievements.unlockedOn", { date: formatDate(item.unlockedAt) })
              : t("achievements.notYet")}
          </p>
        </div>
        </div>
      </div>
    </div>
  );
}

function rarityLabel(t: I18n["t"], rarity: string) {
  switch (rarity) {
    case "common":
      return t("rarityName.common");
    case "uncommon":
      return t("rarityName.uncommon");
    case "rare":
      return t("rarityName.rare");
    case "mythic":
      return t("rarityName.mythic");
    default:
      return t("rarityName.special");
  }
}
