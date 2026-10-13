import { useEffect, useState } from "react";
import { AchievementCard } from "./AchievementCard";
import { getAchievements, type AchievementsView } from "./api";

type Filter = "all" | "unlocked" | "locked" | "secret";

/** The collection of achievement cards: what is unlocked, what remains, what is secret. */
export function Achievements({ refreshKey }: { refreshKey: number }) {
  const [view, setView] = useState<AchievementsView | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getAchievements()
      .then(setView)
      .catch((e) => setError(String(e)));
  }, [refreshKey]);

  if (error) return <p className="error">{error}</p>;
  if (!view) return null;

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
          <option value="all">All achievements</option>
          <option value="unlocked">Unlocked</option>
          <option value="locked">Still to unlock</option>
          <option value="secret">Secret</option>
        </select>
      </div>
      <div className="progress">
        <div className="progress-bar" style={{ width: `${percent}%` }} />
      </div>
      <p className="progress-label">
        {view.unlockedCount} / {view.total} unlocked · {view.total - view.unlockedCount} to go
        {view.secretRemaining > 0 && ` (${view.secretRemaining} secret)`}
      </p>
      <div className="ach-grid">
        {shown.map((a) => (
          <div key={a.id} className="ach-item">
            <AchievementCard item={a} />
            <div className="ach-caption">
              {a.unlocked && a.reward && <span className="gain">Reward: {a.reward}</span>}
              {!a.unlocked && !a.secret && a.reward && <span className="hint">Reward: {a.reward}</span>}
              {!a.unlocked && a.progress && (
                <span className="hint">
                  {a.progress.current.toLocaleString()} / {a.progress.goal.toLocaleString()}
                </span>
              )}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
