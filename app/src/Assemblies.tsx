import { useState, type CSSProperties } from "react";
import type { AssembliesView, AssemblyPart } from "./api";
import { NearImage } from "./CardImage";
import { useI18n } from "./i18n";

/** Where each metal seam's middle is, as a share of the card's width (measured on Scryfall's
 * images: the augment's at its right edge, the host's left of its middle), and where the augment
 * is cut, just past its seam (its name and type line run up to its edge). Maquette
 * `.cache/unstable/combos.html` (`node .cache/ust-combos.js`). */
const AUGMENT_SEAM = 0.973;
const HOST_SEAM = 0.39;
const AUGMENT_CUT = 0.99;

type Filter = "all" | "assembled" | "missing";
const FILTERS: Filter[] = ["all", "assembled", "missing"];

const SEAMS = { "--aug": AUGMENT_SEAM, "--host": HOST_SEAM, "--cut": AUGMENT_CUT } as CSSProperties;

/** One creature: the augment laid over the host, their seams lined up; a card not owned yet is
 * shown faded. A click on either half opens that card's details. */
function Creature({ augment, host, onOpen }: { augment: AssemblyPart; host: AssemblyPart; onOpen: (uuid: string) => void }) {
  const done = augment.owned && host.owned;
  // Not buttons: the theme dresses those as brass plates.
  const half = (part: AssemblyPart, side: "host" | "augment") => (
    <div
      className={`assembly-${side} ${part.owned ? "" : "missing"}`}
      role="button"
      tabIndex={0}
      title={part.card.name}
      onClick={() => onOpen(part.card.uuid)}
      onKeyDown={(e) => e.key === "Enter" && onOpen(part.card.uuid)}
    >
      <NearImage src={part.card.imageUrl} alt={part.card.name} draggable={false} />
    </div>
  );
  return (
    <figure className={`assembly ${done ? "done" : ""}`}>
      <div className="assembly-pair">
        {half(host, "host")}
        {half(augment, "augment")}
      </div>
      <figcaption>
        {done && <span className="assembly-mark">✦ </span>}
        <b>{augment.card.name}</b> + {host.card.name}
      </figcaption>
    </figure>
  );
}

/** A set's assemblies (Unstable): every augment over every host, grouped by augment, filtered
 * by state or by augment. */
export function AssembliesList({ view, onOpen }: { view: AssembliesView; onOpen: (uuid: string) => void }) {
  const { t } = useI18n();
  const [filter, setFilter] = useState<Filter>("all");
  const [only, setOnly] = useState<string>("");
  const matches = (a: AssemblyPart, h: AssemblyPart) => {
    const done = a.owned && h.owned;
    return filter === "all" || (filter === "assembled") === done;
  };
  return (
    <div className="assemblies" style={SEAMS}>
      <div className="cycles-tools">
        <div className="segmented" role="group">
          {FILTERS.map((f) => (
            <button key={f} className={filter === f ? "active" : ""} onClick={() => setFilter(f)}>
              {t(`assemblies.filter.${f}`)}
            </button>
          ))}
        </div>
        <select value={only} onChange={(e) => setOnly(e.target.value)}>
          <option value="">{t("assemblies.everyAugment")}</option>
          {view.augments.map((a) => (
            <option key={a.card.uuid} value={a.card.uuid}>
              {a.card.name}
            </option>
          ))}
        </select>
      </div>
      {view.augments
        .filter((a) => !only || a.card.uuid === only)
        .map((augment) => {
          const hosts = view.hosts.filter((h) => matches(augment, h));
          if (hosts.length === 0) return null;
          const assembled = augment.owned ? view.hosts.filter((h) => h.owned).length : 0;
          return (
            <section key={augment.card.uuid} className="assembly-group">
              <h3>
                {augment.card.name}
                <span className="hint">
                  {" "}
                  · {assembled} / {view.hosts.length}
                  {!augment.owned && ` · ${t("assemblies.augmentMissing")}`}
                </span>
              </h3>
              <div className="assembly-grid">
                {hosts.map((host) => (
                  <Creature key={host.card.uuid} augment={augment} host={host} onOpen={onOpen} />
                ))}
              </div>
            </section>
          );
        })}
    </div>
  );
}

/** Beside the assemblies: the creatures assembled, the augments and hosts owned, and the cards
 * still missing (each opening its details). */
export function AssembliesPanel({ view, onShop, onOpen }: { view: AssembliesView; onShop?: () => void; onOpen: (uuid: string) => void }) {
  const { t } = useI18n();
  const percent = view.total === 0 ? 0 : (view.assembled * 100) / view.total;
  const owned = (list: AssemblyPart[]) => list.filter((p) => p.owned).length;
  const missing = [...view.augments, ...view.hosts].filter((p) => !p.owned);
  return (
    <aside className="frame set-side">
      <div className="cartouche">{t("assemblies.panel")}</div>
      <div className="scroll">
        <div className="ring cycles-ring" style={{ "--p": percent } as CSSProperties}>
          <b>
            {view.assembled} / {view.total}
          </b>
        </div>
        <p className="cycles-ring-label">{t("assemblies.assembledCount")}</p>
        <div className="ready-line">
          <span>{t("assemblies.augments")}</span>
          <b>
            {owned(view.augments)} / {view.augments.length}
          </b>
        </div>
        <div className="ready-line">
          <span>{t("assemblies.hosts")}</span>
          <b>
            {owned(view.hosts)} / {view.hosts.length}
          </b>
        </div>
        {missing.length > 0 && (
          <>
            <h3>{t("assemblies.missing")}</h3>
            {missing.map((p) => (
              <button key={p.card.uuid} className="ready-line linkish" onClick={() => onOpen(p.card.uuid)}>
                <span>{p.card.name}</span>
                <em>#{p.card.number}</em>
              </button>
            ))}
          </>
        )}
        <p className="hint cycles-note">{t("assemblies.hint")}</p>
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
