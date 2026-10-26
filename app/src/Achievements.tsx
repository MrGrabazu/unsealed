import { useEffect, useRef, useState } from "react";
import { AchievementCard } from "./AchievementCard";
import { BackToTop } from "./BackToTop";
import { getAchievements, type AchievementsView, type AchievementView } from "./api";
import { rewardText } from "./boosterKinds";
import { useI18n, type I18n } from "./i18n";
import { Loading } from "./Loading";
import { useNearScreen } from "./nearScreen";
import { useAmountsInText, useMoneyText } from "./money";

type Filter = "all" | "unlocked" | "locked" | "secret";
const FILTERS: Filter[] = ["all", "unlocked", "locked", "secret"];

/** The cabinet of achievement cards (what is unlocked, what remains, what is secret), and
 * the one chosen beside it, in detail. */
export function Achievements({ refreshKey }: { refreshKey: number }) {
  const i18n = useI18n();
  const { t, lang } = i18n;
  const [view, setView] = useState<AchievementsView | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);

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
  // The one chosen, else the latest unlocked, else the first shown.
  const latest = [...view.items].filter((a) => a.unlocked).sort((a, b) => (b.unlockedAt ?? 0) - (a.unlockedAt ?? 0))[0];
  const current = view.items.find((a) => a.id === chosen) ?? latest ?? shown[0] ?? null;

  return (
    <div className="split ach-split">
      <section className="frame">
        <div className="cartouche">
          {t("achievements.cabinet", { unlocked: view.unlockedCount, total: view.total })}
        </div>
        <div className="ach-tools">
          <div className="segmented" role="group">
            {FILTERS.map((f) => (
              <button key={f} className={filter === f ? "active" : ""} onClick={() => setFilter(f)}>
                {t(`achievements.${f}`)}
              </button>
            ))}
          </div>
          <div className="ach-progress" title={t("achievements.progress", { unlocked: view.unlockedCount, total: view.total, left: view.total - view.unlockedCount })}>
            <div className="bar">
              <i style={{ width: `${percent}%` }} />
            </div>
            <span>
              {view.unlockedCount} / {view.total}
              {view.secretRemaining > 0 && t("achievements.secretLeft", { count: view.secretRemaining })}
            </span>
          </div>
        </div>
        <div className="scroll" ref={list}>
          <div className="ach-grid">
            {shown.map((a) => (
              <AchievementTile key={a.id} item={a} selected={current?.id === a.id} onChoose={() => setChosen(a.id)} />
            ))}
          </div>
          <BackToTop target={list} />
        </div>
      </section>
      {current && <AchievementDetails item={current} />}
    </div>
  );
}

/** A card of the cabinet, drawn only near the screen; its shadow is the tile's, not a filter. */
function AchievementTile({ item, selected, onChoose }: { item: AchievementView; selected: boolean; onChoose: () => void }) {
  const { t } = useI18n();
  const ref = useRef<HTMLButtonElement>(null);
  const near = useNearScreen(ref, true);
  const glow = item.unlocked && (item.rarity === "rare" || item.rarity === "mythic") ? item.rarity : undefined;
  return (
    <button ref={ref} className={`ach-item ${selected ? "sel" : ""}`} data-glow={glow} title={t("achievements.open")} onClick={onChoose}>
      {near ? <AchievementCard item={item} lite /> : <span className="ach-card-space" />}
    </button>
  );
}

/** An achievement in detail: its card, large, and in plain text its goal, progress, reward
 * and when it was unlocked. A locked secret keeps everything hidden but its rarity. */
function AchievementDetails({ item }: { item: AchievementView }) {
  const i18n = useI18n();
  const { t, formatNumber, formatDate } = i18n;
  const moneyText = useMoneyText();
  const amountsInText = useAmountsInText();
  const hidden = item.secret && !item.unlocked;
  const progress = item.progress;
  const percent = progress ? Math.min(100, (progress.current * 100) / Math.max(1, progress.goal)) : 0;
  return (
    <aside className="frame ach-side">
      <div className="cartouche">
        {item.unlocked && item.unlockedAt != null
          ? t("achievements.unlockedShort", { date: formatDate(item.unlockedAt) })
          : rarityLabel(t, item.rarity)}
      </div>
      <div className="scroll">
        <div className="ach-big">
          <AchievementCard item={item} />
        </div>
        <p className="meta" style={{ textAlign: "center" }}>
          {t("achievements.cardNumber", { number: item.number })} · {rarityLabel(t, item.rarity)}
          {item.secret && ` · ${t("achievements.secret")}`}
        </p>
        <h2 className="ach-name">{hidden ? t("achievements.secretName") : item.name}</h2>
        <p className="ach-goal">{hidden ? t("achievements.secretText") : amountsInText(item.description ?? "")}</p>
        <div className="gild-divider" />
        {progress && !item.unlocked && (
          <div className="ready-line">
            <span>{t("achievements.progressTitle")}</span>
            <b>
              {formatNumber(progress.current)} / {formatNumber(progress.goal)} ({Math.floor(percent)} %)
            </b>
          </div>
        )}
        {progress && !item.unlocked && (
          <div className="bar" style={{ margin: "6px 0 4px" }}>
            <i style={{ width: `${percent}%` }} />
          </div>
        )}
        {!hidden && (
          <div className="ready-line">
            <span>{t("achievements.rewardTitle")}</span>
            <b>{item.reward ? rewardText(i18n, moneyText, item.reward) : t("achievements.noReward")}</b>
          </div>
        )}
        <div className="ready-line" style={{ borderBottom: "none" }}>
          <span className={item.unlocked ? "gain" : "hint"}>
            {item.unlocked && item.unlockedAt != null
              ? t("achievements.unlockedOn", { date: formatDate(item.unlockedAt) })
              : t("achievements.notYet")}
          </span>
        </div>
      </div>
    </aside>
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
