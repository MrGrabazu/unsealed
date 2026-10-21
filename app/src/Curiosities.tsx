import { useEffect, useRef, useState } from "react";
import { ArtifactGlyph, artifactMetal } from "./ArtifactGlyph";
import { BackToTop } from "./BackToTop";
import { getArtifacts, type ArtifactView } from "./api";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { Money, useAmountsInText } from "./money";

type Filter = "all" | "owned" | "shop" | "hidden";
const FILTERS: Filter[] = ["all", "owned", "shop", "hidden"];
/** Niches per shelf of the cabinet. */
const PER_SHELF = 5;

/** The cabinet of curiosities: every artifact under its glass bell, on shelves (owned ones
 * shining, those on sale unlit, the others as silhouettes), and the one chosen beside it. */
export function Curiosities({ refreshKey, onShop }: { refreshKey: number; onShop: () => void }) {
  const { t, lang } = useI18n();
  const [items, setItems] = useState<ArtifactView[] | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getArtifacts(lang)
      .then(setItems)
      .catch((e) => setError(String(e)));
  }, [refreshKey, lang]);

  if (error) return <p className="error">{error}</p>;
  if (!items) return <Loading label={t("curios.loading")} />;

  const owned = items.filter((a) => a.status === "owned").length;
  const shown = items.filter((a) => filter === "all" || a.status === filter);
  const latest = [...items].filter((a) => a.status === "owned").sort((a, b) => (b.acquiredAt ?? 0) - (a.acquiredAt ?? 0))[0];
  const current = items.find((a) => a.id === chosen) ?? latest ?? shown[0] ?? null;
  const shelves: ArtifactView[][] = [];
  for (let i = 0; i < shown.length; i += PER_SHELF) shelves.push(shown.slice(i, i + PER_SHELF));

  return (
    <div className="split curios-split">
      <section className="frame">
        <div className="cartouche">{t("curios.cabinet", { owned, total: items.length })}</div>
        <div className="ach-tools">
          <div className="segmented" role="group">
            {FILTERS.map((f) => (
              <button key={f} className={filter === f ? "active" : ""} onClick={() => setFilter(f)}>
                {t(`curios.filter.${f}`)}
              </button>
            ))}
          </div>
          <div className="ach-progress">
            <div className="bar">
              <i style={{ width: `${items.length === 0 ? 0 : (owned * 100) / items.length}%` }} />
            </div>
            <span>
              {owned} / {items.length}
            </span>
          </div>
        </div>
        <div className="scroll" ref={list}>
          <div className="cabinet">
            {shelves.map((shelf, i) => (
              <div key={i} className="cabinet-shelf">
                {shelf.map((a) => (
                  <Niche key={a.id} item={a} selected={current?.id === a.id} onChoose={() => setChosen(a.id)} />
                ))}
              </div>
            ))}
            {shown.length === 0 && <p className="hint">{t("curios.none")}</p>}
          </div>
          <BackToTop target={list} />
        </div>
      </section>
      {current && <CurioDetails item={current} onShop={onShop} />}
    </div>
  );
}

/** One artifact under its bell: shining when owned, unlit when on sale, a silhouette else. */
function Niche({ item, selected, onChoose }: { item: ArtifactView; selected: boolean; onChoose: () => void }) {
  const { t } = useI18n();
  const hidden = item.status === "hidden";
  const metal = artifactMetal(item.glyph, item.rarity);
  return (
    <button className={`niche is-${item.status} ${selected ? "sel" : ""}`} style={{ ["--metal" as string]: metal }} onClick={onChoose}>
      <span className="bell" />
      <ArtifactGlyph glyph={item.glyph} color={metal} silhouette={hidden} />
      <span className="plinth" />
      <span className="niche-label">{hidden ? "? ? ?" : item.name}</span>
      {item.status === "owned" && <span className="niche-tag on">{t("curios.active")}</span>}
      {item.status === "shop" && <span className="niche-tag">{t("curios.onSale")}</span>}
    </button>
  );
}

/** The artifact chosen, under the light: its name, rarity, effect and where it comes from. A
 * hidden one tells only its rarity and the kind of its source. */
function CurioDetails({ item, onShop }: { item: ArtifactView; onShop: () => void }) {
  const { t, formatDate } = useI18n();
  const amountsInText = useAmountsInText();
  const hidden = item.status === "hidden";
  const metal = artifactMetal(item.glyph, item.rarity);
  return (
    <aside className="frame curio-side">
      <div className="cartouche">{t(`curios.rarity.${item.rarity}`)}</div>
      <div className="scroll">
        <div className="showcase" style={{ ["--metal" as string]: metal }}>
          <span className="showcase-light" />
          <span className="plinth" />
          <ArtifactGlyph glyph={item.glyph} color={metal} silhouette={hidden} />
        </div>
        <h2>{hidden ? "? ? ?" : item.name}</h2>
        <div className="meta">
          <b style={{ color: metal }}>{t(`curios.rarity.${item.rarity}`)}</b> · n° {String(item.number).padStart(2, "0")}
        </div>
        <div className="curio-effect">{hidden ? t(`curios.hint.${item.source}`) : amountsInText(item.effect ?? "")}</div>
        {item.status === "owned" && item.acquiredAt != null && (
          <div className="ready-line">
            <span>{t("curios.acquired")}</span>
            <b>{formatDate(item.acquiredAt)}</b>
          </div>
        )}
        {!hidden && (
          <div className="ready-line">
            <span>{t("curios.source")}</span>
            <b>{t(`curios.from.${item.source}`)}</b>
          </div>
        )}
        {!hidden && item.flavor && <p className="curio-flavor">« {item.flavor} »</p>}
      </div>
      <div className="buy-bar">
        {item.status === "owned" ? (
          <div className="curio-active">✦ {t("curios.effectActive")}</div>
        ) : item.status === "shop" ? (
          <button className="big" onClick={onShop}>
            {t("curios.goShop")} · <Money cents={item.priceCents ?? 0} />
          </button>
        ) : (
          <p className="hint">{t("curios.mystery")}</p>
        )}
      </div>
    </aside>
  );
}
