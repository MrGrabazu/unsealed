import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { CollectionCard, SceneCell, SceneView, ScenesView } from "./api";
import { Backdrop } from "./Backdrop";
import { Placeholder } from "./Binder";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { FoilSheen } from "./foils";
import { largeImage } from "./images";

/** The face of a cell's card that belongs to the scene (a double-faced card's back may be). */
function faceOf(cell: SceneCell): CollectionCard {
  const names = cell.card.name.split(" // ");
  const name = (cell.back ? names[1] : names[0]) ?? cell.card.name;
  return { ...cell.card, name };
}

/** The frames a painting may hang in (maquette `.cache/scenes/museum.html`). */
const FRAMES = ["baroque", "ripple", "silver", "walnut", "reeded"] as const;

/** A painting's frame, always the same for a scene: drawn from its set and id. */
function frameOf(setCode: string, id: string): (typeof FRAMES)[number] {
  let h = 2166136261;
  for (const c of `${setCode}/${id}`) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0;
  return FRAMES[Math.floor((h / 2 ** 32) * FRAMES.length)];
}

/** How wide each card may be for a scene to fit a box, its frame included (the moulding and the
 * mat take about half a card's width around it). */
function fit(scene: SceneView, width: number, height: number, max: number): number {
  const byWidth = width / (scene.columns + 0.52);
  const byHeight = height / ((scene.rows * 88) / 63 + 0.52);
  return Math.max(24, Math.floor(Math.min(byWidth, byHeight, max)));
}

/** A corner ornament: an acanthus leaf curling out of a C-scroll, a shell at the corner. Drawn
 * for the top-left corner, turned for the others; filled with the frame's metal. */
function Corner({ at }: { at: "tl" | "tr" | "bl" | "br" }) {
  return (
    <svg className={`orn ${at}`} viewBox="0 0 100 100" aria-hidden="true">
      <path d="M6 6 C30 2 52 4 70 14 C58 12 46 14 38 22 C50 20 60 26 62 36 C54 30 42 30 34 38 C30 44 30 54 36 62 C26 60 20 50 22 38 C14 46 12 58 14 70 C4 52 2 30 6 6 Z" />
      <path d="M10 10 C22 8 34 10 44 16 C34 18 26 22 22 30 C18 22 14 16 10 10 Z" className="hi" />
      <circle cx="14" cy="14" r="7" />
      <circle cx="14" cy="14" r="3.4" className="hi" />
      <path d="M70 14 C80 18 88 26 92 36 C84 30 76 28 68 30 C72 24 72 18 70 14 Z" />
      <path d="M14 70 C18 80 26 88 36 92 C30 84 28 76 30 68 C24 72 18 72 14 70 Z" />
    </svg>
  );
}

/** The baroque frame's crest: a shell between two scrolls. */
function Crest() {
  return (
    <svg className="crest" viewBox="0 0 120 50" aria-hidden="true">
      <path d="M60 4 C72 4 82 14 84 28 C76 22 68 22 60 28 C52 22 44 22 36 28 C38 14 48 4 60 4 Z" />
      <path d="M60 10 L60 26 M50 12 L54 26 M70 12 L66 26 M42 18 L48 27 M78 18 L72 27" className="line" />
      <path d="M36 28 C26 26 14 30 6 40 C16 36 26 36 34 40 C30 34 32 30 36 28 Z M84 28 C94 26 106 30 114 40 C104 36 94 36 86 40 C90 34 88 30 84 28 Z" />
      <circle cx="60" cy="38" r="6" />
      <circle cx="60" cy="38" r="2.6" className="hi" />
    </svg>
  );
}

/** The metals of the ornaments, defined once for every frame of the page. */
function Metals() {
  return (
    <svg width="0" height="0" className="museum-defs" aria-hidden="true">
      <defs>
        <linearGradient id="museum-gold" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#fff3c4" />
          <stop offset=".3" stopColor="#e9bf62" />
          <stop offset=".6" stopColor="#a06c1e" />
          <stop offset="1" stopColor="#5a3608" />
        </linearGradient>
        <linearGradient id="museum-silver" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset=".35" stopColor="#cfd5db" />
          <stop offset=".7" stopColor="#6c747c" />
          <stop offset="1" stopColor="#2e3338" />
        </linearGradient>
        <linearGradient id="museum-wood" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#c99a62" />
          <stop offset=".4" stopColor="#8a5a32" />
          <stop offset="1" stopColor="#3a200e" />
        </linearGradient>
      </defs>
    </svg>
  );
}

/** The wall sign: an engraved brass plate with a pointing hand. */
function Hand({ left }: { left?: boolean }) {
  return (
    <svg viewBox="0 0 64 40" aria-hidden="true">
      <g transform={left ? "translate(64 0) scale(-1 1)" : undefined}>
        <path d="M6 15 h22 c3 0 4 -1 6 -3 l5 -5 c2 -2 5 -1 5 2 c0 2 -2 3 -3 5 h13 c3 0 4 2 4 3.5 s-1 3.5 -4 3.5 h-2 c2 0 3 1.5 3 3 s-1 3 -3 3 h-2 c2 0 3 1.5 3 3 s-1 3 -3 3 h-3 c2 0 2.5 1.2 2.5 2.4 s-1 2.6 -3 2.6 h-15 c-4 0 -7 -2 -9 -4 h-17 z" />
        <path d="M6 15 v18" className="cuff" />
      </g>
    </svg>
  );
}

interface PaintingProps {
  setCode: string;
  scene: SceneView;
  /** Each card's width, in pixels. */
  cardWidth: number;
  /** False far from the painting looked at: its cards' images are left out of the page. */
  shown: boolean;
  large?: boolean;
  onClick?: () => void;
  onCard?: (uuid: string) => void;
}

/** A scene in its frame: its cards edge to edge, each keeping its own rounded corners, a missing
 * one an empty place holding the binder's scrap of paper. */
function Painting({ setCode, scene, cardWidth, shown, large, onClick, onCard }: PaintingProps) {
  const frame = frameOf(setCode, scene.id);
  const style = {
    "--cw": `${cardWidth}px`,
    "--m": `${Math.max(14, Math.round(cardWidth * 0.17))}px`,
    "--mat": `${Math.max(8, Math.round(cardWidth * 0.09))}px`,
    "--columns": scene.columns,
  } as CSSProperties;
  return (
    <div className={`museum-frame f-${frame}`} style={style} onClick={onClick} role={onClick ? "button" : undefined}>
      <div className="band" />
      {frame !== "reeded" && (["tl", "tr", "bl", "br"] as const).map((at) => <Corner key={at} at={at} />)}
      {frame === "baroque" && <Crest />}
      <div className="museum-mat">
        <div className="museum-fresco">
          {scene.cells.map((cell, i) => {
            const face = faceOf(cell);
            const url = (cell.back && cell.card.backImageUrl) || cell.card.imageUrl;
            return (
              <div
                key={i}
                className={`pocket scene-cell ${cell.owned ? "" : "missing"}`}
                title={`#${face.number} ${face.name}`}
                onClick={
                  onCard &&
                  ((e) => {
                    e.stopPropagation();
                    onCard(cell.card.uuid);
                  })
                }
              >
                <div className="thumb">
                  {cell.owned
                    ? shown && <CardImage front={large ? largeImage(url) : url} back={null} alt={face.name} />
                    : <Placeholder card={face} />}
                  {cell.foil && shown && <FoilSheen rarity={cell.card.rarity} kind={cell.card.foilKind} badge={false} />}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** A set's scenes as a museum gallery: one painting at a time on the wall, in its own frame,
 * under a lamp, a small brass label below; brass signs on either side (and ←/→) walk along the
 * wall to the next one; a click on the painting shows it large over everything, where a click on
 * a card opens its details. */
export function ScenesGallery({
  view,
  at,
  onAt,
  onOpen,
  active,
}: {
  view: ScenesView;
  at: number;
  onAt: (at: number) => void;
  onOpen: (uuid: string) => void;
  /** False while something covers the gallery (card details): the arrow keys are theirs. */
  active: boolean;
}) {
  const { t } = useI18n();
  const hall = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 900, height: 600 });
  const [large, setLarge] = useState(false);
  const current = Math.min(at, view.scenes.length - 1);
  const go = (i: number) => onAt(Math.max(0, Math.min(view.scenes.length - 1, i)));

  useEffect(() => {
    const node = hall.current;
    if (!node) return;
    const observer = new ResizeObserver(() => setSize({ width: node.clientWidth, height: node.clientHeight }));
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (large) {
        if (e.key === "Escape") setLarge(false);
        return;
      }
      if (e.key === "ArrowRight") go(current + 1);
      else if (e.key === "ArrowLeft") go(current - 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const scene = view.scenes[current];
  if (!scene) return null;
  // The wall above the wainscot, less the label and the room around the painting.
  const wall = { width: size.width * 0.66, height: size.height * 0.93 - 110 };
  return (
    <div className="museum" ref={hall}>
      <Metals />
      <div className="museum-track" style={{ transform: `translateX(${-100 * current}%)` }}>
        {view.scenes.map((s, i) => (
          <div key={s.id} className="museum-spot">
            <div className="museum-lamp" />
            <Painting
              setCode={view.setCode}
              scene={s}
              cardWidth={fit(s, wall.width, wall.height, 260)}
              shown={Math.abs(i - current) <= 1}
              onClick={i === current ? () => setLarge(true) : undefined}
            />
            <div className="museum-label">
              <b>{s.name}</b>
              <span className={s.complete ? "done" : ""}>
                {s.complete && `✦ ${t(s.allFoil ? "scenes.allFoil" : "cycles.complete")} · `}
                {t("scenes.count", { owned: s.owned, total: s.total })}
              </span>
            </div>
          </div>
        ))}
      </div>
      <button className="museum-sign left" disabled={current === 0} onClick={() => go(current - 1)}>
        <Hand left />
        {t("scenes.previous")}
      </button>
      <button className="museum-sign right" disabled={current === view.scenes.length - 1} onClick={() => go(current + 1)}>
        <Hand />
        {t("scenes.next")}
      </button>
      <div className="museum-where">{t("scenes.where", { n: current + 1, total: view.scenes.length })}</div>
      {large && (
        <Backdrop onClose={() => setLarge(false)}>
          <div className="museum-large">
            <Painting
              setCode={view.setCode}
              scene={scene}
              cardWidth={fit(scene, window.innerWidth * 0.9, window.innerHeight * 0.86, 520)}
              shown
              large
              onCard={(uuid) => {
                setLarge(false);
                onOpen(uuid);
              }}
            />
            <button className="museum-close" aria-label={t("scenes.close")} onClick={() => setLarge(false)}>
              ✕
            </button>
          </div>
        </Backdrop>
      )}
    </div>
  );
}

/** Beside them: the set's scenes at a glance, and those nearly complete (a click shows one). */
export function ScenesPanel({ view, onShop, onShow }: { view: ScenesView; onShop?: () => void; onShow: (at: number) => void }) {
  const { t, plural } = useI18n();
  const percent = view.total === 0 ? 0 : (view.completed * 100) / view.total;
  const nearly = view.scenes
    .map((s, i) => ({ s, i }))
    .filter(({ s }) => !s.complete && s.owned > 0)
    .sort((a, b) => a.s.total - a.s.owned - (b.s.total - b.s.owned) || b.s.owned - a.s.owned)
    .slice(0, 5);
  const allFoil = view.scenes.filter((s) => s.complete && s.allFoil).length;
  return (
    <aside className="frame set-side">
      <div className="cartouche">{t("scenes.panel")}</div>
      <div className="scroll">
        <div className="ring cycles-ring" style={{ "--p": percent } as CSSProperties}>
          <b>
            {view.completed} / {view.total}
          </b>
        </div>
        <p className="cycles-ring-label">{t("scenes.completeCount")}</p>
        <div className="ready-line">
          <span>{t("scenes.cards")}</span>
          <b>
            {view.cardsOwned} / {view.cardsTotal}
          </b>
        </div>
        <div className="ready-line">
          <span>{t("scenes.allFoilCount")}</span>
          <b>{allFoil}</b>
        </div>
        {nearly.length > 0 && (
          <>
            <h3>{t("cycles.nearly")}</h3>
            {nearly.map(({ s, i }) => {
              const left = s.total - s.owned;
              const one = left === 1 ? s.cells.find((c) => !c.owned) : undefined;
              return (
                <button key={s.id} className="ready-line linkish" onClick={() => onShow(i)}>
                  <span>{s.name}</span>
                  <em>{one ? t("cycles.missingOne", { card: faceOf(one).name }) : plural("cycles.missingMany", left)}</em>
                </button>
              );
            })}
          </>
        )}
        <p className="hint cycles-note">{t("scenes.hint")}</p>
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
