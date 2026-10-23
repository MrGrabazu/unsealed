import { useState, type CSSProperties } from "react";
import type { CycleView, CyclesView } from "./api";
import { Placeholder } from "./Binder";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { FoilSheen } from "./foils";

type Filter = "all" | "progress" | "complete" | "todo";
const FILTERS: Filter[] = ["all", "progress", "complete", "todo"];
/** A group's sort, as the player tells them apart: the wiki's cycles, its stories, ours. */
type Kind = "cycle" | "story" | "ours";
const KINDS: Kind[] = ["cycle", "story", "ours"];
const kindOf = (c: CycleView): Kind => (c.custom ? "ours" : c.kind);

const matches = (c: CycleView, filter: Filter) =>
  filter === "all" ||
  (filter === "complete" && c.complete) ||
  (filter === "progress" && !c.complete && c.owned > 0) ||
  (filter === "todo" && c.owned === 0);

/** A group's badge: a gold diamond for the wiki's cycles, a pilcrow for its stories, our
 * red seal for ours. */
function Badge({ group }: { group: CycleView }) {
  const { t } = useI18n();
  const kind = kindOf(group);
  const [mark, title] = kind === "ours" ? ["U", t("cycles.ours")] : kind === "story" ? ["¶", t("cycles.story")] : ["◆", t("cycles.official")];
  return (
    <span className={`cycle-badge ${kind === "ours" ? "ours" : kind === "story" ? "story" : "official"}`} title={title}>
      {mark}
    </span>
  );
}

/** A set's groups (cycles, stories, ours): each one a plaque with its name and progress, its
 * cards side by side (the best copy owned of each, else a scrap of paper as in the binder). */
export function CyclesList({ view, onOpen }: { view: CyclesView; onOpen: (uuid: string) => void }) {
  const { t, plural } = useI18n();
  const [filter, setFilter] = useState<Filter>("all");
  const [kind, setKind] = useState<Kind | "all">("all");
  // The sorts the set has: the choice is only offered when there are several.
  const kinds = KINDS.filter((k) => view.cycles.some((c) => kindOf(c) === k));
  const shown = view.cycles.filter((c) => matches(c, filter) && (kind === "all" || kindOf(c) === kind));
  return (
    <div className="cycles">
      <div className="cycles-tools">
        {kinds.length > 1 && (
          <div className="segmented" role="group">
            {(["all", ...kinds] as const).map((k) => (
              <button key={k} className={kind === k ? "active" : ""} onClick={() => setKind(k)}>
                {t(`cycles.kind.${k}`)}
              </button>
            ))}
          </div>
        )}
        <div className="segmented" role="group">
          {FILTERS.map((f) => (
            <button key={f} className={filter === f ? "active" : ""} onClick={() => setFilter(f)}>
              {t(`cycles.filter.${f}`)}
            </button>
          ))}
        </div>
        {/* The explanation on demand: hovering (or focusing) the "?" shows it. */}
        <span className="help-tip" tabIndex={0} aria-label={t("cycles.hint")}>
          ?<span className="help-tip-text" role="tooltip">{t("cycles.hint")}</span>
        </span>
      </div>
      {shown.length === 0 && <p className="hint">{t("cycles.none")}</p>}
      {shown.map((c) => (
        <article key={c.id} className={`cycle ${c.complete ? "done" : ""} ${c.prestige ? "prestige" : ""}`}>
          <header>
            <Badge group={c} />
            <b>{c.name}</b>
            <span className="cycle-count">
              {c.owned} / {c.total}
            </span>
            <span className="bar cycle-bar">
              <i style={{ width: `${(c.owned * 100) / c.total}%` }} />
            </span>
            {c.prestige ? (
              <span className="cycle-tag prestige">✦ {t("cycles.prestige")}</span>
            ) : (
              c.complete && <span className="cycle-tag">✦ {t("cycles.complete")}</span>
            )}
          </header>
          <div className="cycle-strip">
            {c.slots.map((s, i) => (
              <div
                key={i}
                className={`pocket cycle-slot ${s.owned ? "" : "missing"} ${s.prestige ? "best" : ""}`}
                role="button"
                tabIndex={0}
                title={`#${s.card.number} ${s.card.name}${s.printings > 1 ? ` · ${plural("cycles.printings", s.printings)}` : ""}`}
                onClick={() => onOpen(s.card.uuid)}
                onKeyDown={(e) => e.key === "Enter" && onOpen(s.card.uuid)}
              >
                <div className="thumb">
                  {s.owned ? <CardImage front={s.card.imageUrl} back={s.card.backImageUrl} alt={s.card.name} lazy /> : <Placeholder card={s.card} />}
                  {s.foil && <FoilSheen rarity={s.card.rarity} kind={s.card.foilKind} badge={false} />}
                  {s.prestige && s.printings > 1 && <span className="cycle-best" title={t("cycles.best")}>✦</span>}
                </div>
              </div>
            ))}
          </div>
        </article>
      ))}
    </div>
  );
}

/** Beside them: the set's cycles at a glance, and those nearly complete. */
export function CyclesPanel({ view, onShop }: { view: CyclesView; onShop?: () => void }) {
  const { t, plural } = useI18n();
  const percent = view.total === 0 ? 0 : (view.completed * 100) / view.total;
  const nearly = view.cycles
    .filter((c) => !c.complete && c.owned > 0)
    .sort((a, b) => a.total - a.owned - (b.total - b.owned) || b.owned - a.owned)
    .slice(0, 5);
  const prestige = view.cycles.filter((c) => c.prestige).length;
  return (
    <aside className="frame set-side">
      <div className="cartouche">{t("cycles.panel")}</div>
      <div className="scroll">
        <div className="ring cycles-ring" style={{ "--p": percent } as CSSProperties}>
          <b>
            {view.completed} / {view.total}
          </b>
        </div>
        <p className="cycles-ring-label">{t("cycles.completeCount")}</p>
        {(
          [
            ["cycle", "cycles.countCycles"],
            ["story", "cycles.countStories"],
            ["ours", "cycles.countOurs"],
          ] as const
        ).map(([kind, label]) => {
          const groups = view.cycles.filter((c) => kindOf(c) === kind);
          return (
            groups.length > 0 && (
              <div key={kind} className="ready-line">
                <span>{t(label)}</span>
                <b>
                  {groups.filter((c) => c.complete).length} / {groups.length}
                </b>
              </div>
            )
          );
        })}
        <div className="ready-line">
          <span>{t("cycles.cards")}</span>
          <b>
            {view.slotsOwned} / {view.slotsTotal}
          </b>
        </div>
        <div className="ready-line">
          <span>{t("cycles.prestigeCount")}</span>
          <b>{prestige}</b>
        </div>
        {nearly.length > 0 && (
          <>
            <h3>{t("cycles.nearly")}</h3>
            {nearly.map((c) => {
              const left = c.total - c.owned;
              const one = left === 1 ? c.slots.find((s) => !s.owned) : undefined;
              return (
                <div key={c.id} className="ready-line">
                  <span>{c.name}</span>
                  <em>{one ? t("cycles.missingOne", { card: one.card.name }) : plural("cycles.missingMany", left)}</em>
                </div>
              );
            })}
          </>
        )}
        <p className="hint cycles-note">
          <span className="cycle-badge official">◆</span> {t("cycles.official")}
          <br />
          <span className="cycle-badge story">¶</span> {t("cycles.story")}
          <br />
          <span className="cycle-badge ours">U</span> {t("cycles.ours")}
        </p>
      </div>
      {onShop && (
        <div className="buy-bar">
          <button className="big" onClick={onShop}>
            {t("collection.buyBoosters")}
          </button>
        </div>
      )}
    </aside>
  );
}
