import { useEffect, useState } from "react";
import { formatUsd, getCollection, type CollectionView, type SetSummary } from "./api";

type Filter = "all" | "owned" | "missing";

/** Collection of one set: completion progress, missing cards greyed out. */
export function Collection({ sets, refreshKey }: { sets: SetSummary[]; refreshKey: number }) {
  const [setCode, setSetCode] = useState("");
  const [view, setView] = useState<CollectionView | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!setCode && sets.length > 0) setSetCode(sets[0].code);
  }, [sets, setCode]);

  // Reload when the set changes or a booster was opened.
  useEffect(() => {
    if (!setCode) return;
    setError(null);
    getCollection(setCode).then(setView).catch((e) => setError(String(e)));
  }, [setCode, refreshKey]);

  const shown = view?.cards.filter((c) => {
    const owned = c.quantity + c.foilQuantity > 0;
    return filter === "all" || (filter === "owned") === owned;
  });

  return (
    <section>
      <div className="controls">
        <select value={setCode} onChange={(e) => setSetCode(e.target.value)}>
          {sets.map((s) => (
            <option key={s.code} value={s.code}>
              {s.name} ({s.code})
            </option>
          ))}
        </select>
        <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)}>
          <option value="all">All cards</option>
          <option value="owned">Owned</option>
          <option value="missing">Missing</option>
        </select>
      </div>

      {error && <p className="error">{error}</p>}
      {view && (
        <>
          <div className="progress">
            <div className="progress-bar" style={{ width: `${view.completionPercent}%` }} />
          </div>
          <p className="progress-label">
            {view.ownedCount} / {view.totalCount} cards ({view.completionPercent.toFixed(1)}%)
          </p>
          <div className="collection-grid">
            {shown?.map((c) => {
              const owned = c.quantity + c.foilQuantity > 0;
              return (
                <div key={c.uuid} className={`collection-card ${owned ? "" : "missing"}`} data-rarity={c.rarity}>
                  <div className="thumb">
                    <img src={c.imageUrl} alt={c.name} loading="lazy" draggable={false} />
                    {c.quantity + c.foilQuantity > 1 && <span className="badge-qty">×{c.quantity + c.foilQuantity}</span>}
                    {c.foilQuantity > 0 && <span className="badge-foil">FOIL</span>}
                  </div>
                  <div className="card-caption visible">
                    <span className="name">
                      #{c.number} {c.name}
                    </span>
                    <span className="price">{c.priceCents == null ? "—" : formatUsd(c.priceCents)}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </>
      )}
    </section>
  );
}
