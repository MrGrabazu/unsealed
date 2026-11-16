import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import type { CollectionCard, LoreArt, LoreChapter, LoreView } from "./api";
import { Placeholder } from "./Binder";
import { CardImage, FittedImage } from "./CardImage";
import { useI18n } from "./i18n";
import { artCrop, smallImage } from "./images";

const ROMAN = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV", "XV"];
const roman = (i: number) => ROMAN[i] ?? String(i + 1);
/** How long a page takes to turn, in ms (its CSS animation is given this duration). */
const TURN_MS = 700;

/** A card in a slot: its image when owned, else the binder's scrap of paper; a click opens it. */
function Slot({ card, owned, onOpen }: { card: CollectionCard; owned: boolean; onOpen: (uuid: string) => void }) {
  return (
    <div
      className={`pocket lore-slot ${owned ? "" : "missing"}`}
      role="button"
      tabIndex={0}
      title={`#${card.number} ${card.name}`}
      onClick={() => onOpen(card.uuid)}
      onKeyDown={(e) => e.key === "Enter" && onOpen(card.uuid)}
    >
      <div className="thumb">{owned ? <CardImage front={smallImage(card.imageUrl)} back={null} alt={card.name} /> : <Placeholder card={card} />}</div>
    </div>
  );
}

/** A card's art, never cropped further nor altered, its artist credited below it. */
function Plate({ art, onOpen }: { art: LoreArt; onOpen: (uuid: string) => void }) {
  const { t } = useI18n();
  return (
    <figure className="lore-plate">
      <button className="lore-plate-art" onClick={() => onOpen(art.card.uuid)} title={art.card.name}>
        <FittedImage src={artCrop(art.card.imageUrl)} alt={art.card.name} />
      </button>
      <figcaption>{art.artist ? t("lore.credit", { card: art.card.name, artist: art.artist }) : art.card.name}</figcaption>
    </figure>
  );
}

/** How many spreads the chapter takes, read from where its end marker landed among the columns:
 * the flow is two columns of the book's height, and every column past them is another page. */
function spreadsOf(flow: HTMLElement, end: HTMLElement): { count: number; step: number } {
  const style = getComputedStyle(flow);
  const pad = parseFloat(style.paddingLeft) || 0;
  const gap = parseFloat(style.columnGap) || 0;
  const step = flow.clientWidth - 2 * pad + gap;
  const x = end.getBoundingClientRect().left - flow.getBoundingClientRect().left - pad;
  return { count: step > 0 ? Math.max(1, Math.floor(x / step) + 1) : 1, step };
}

/** A place in the book: a chapter and one of its spreads ("last" until a sheet has measured it). */
type Place = { chapter: number; spread: number | "last" };

/** The pages of a chapter as one paper sheet of the book's size: its content in two columns per
 * spread, the spread shown slid into view (no animation), its folios. The book shows it whole;
 * a page being turned shows halves of two of them (`Clip`). */
function Sheet({
  chapter,
  index,
  spread,
  measured,
  onOpen,
  onLayout,
}: {
  chapter: LoreChapter;
  index: number;
  spread: number | "last";
  /** False on the back of a turning leaf: drawn mirrored, it cannot be measured (it is given a
   * spread already measured elsewhere). */
  measured: boolean;
  onOpen: (uuid: string) => void;
  onLayout?: (count: number, shown: number) => void;
}) {
  const { t, plural } = useI18n();
  const flowRef = useRef<HTMLDivElement>(null);
  const endRef = useRef<HTMLSpanElement>(null);
  const [layout, setLayout] = useState({ count: 1, step: 0 });

  const measure = useCallback(() => {
    const flow = flowRef.current;
    const end = endRef.current;
    if (!flow || !end) return;
    const next = spreadsOf(flow, end);
    if (!measured) next.count = 1;
    setLayout((old) => (old.count === next.count && old.step === next.step ? old : next));
  }, [measured]);
  useLayoutEffect(() => measure(), [chapter, measure]);
  useEffect(() => {
    const flow = flowRef.current;
    if (!flow) return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(flow);
    return () => observer.disconnect();
  }, [measure]);

  const shown = spread === "last" ? layout.count - 1 : measured ? Math.min(spread, layout.count - 1) : spread;
  const report = useRef(onLayout);
  report.current = onLayout;
  useLayoutEffect(() => report.current?.(layout.count, shown), [layout.count, shown]);

  const missing = chapter.keys.filter((k) => !k.owned).map((k) => k.card.name);
  return (
    <div className="lore-sheet">
      <div className="lore-flow" ref={flowRef} style={{ transform: `translateX(${-shown * layout.step}px)` }}>
        <p className="lore-kicker">{t("lore.chapter", { n: roman(index) })}</p>
        <h2 className="lore-title">{chapter.title}</h2>
        <div className="lore-rule" />
        {chapter.unlocked ? (
          <>
            {chapter.illustration && <Plate art={chapter.illustration} onOpen={onOpen} />}
            {chapter.paragraphs.map((p, i) => (
              <p key={i} className={i === 0 ? "lore-drop" : ""}>
                {p}
              </p>
            ))}
          </>
        ) : (
          <>
            <div className="lore-plate sealed">
              <span className="lore-wax big">U</span>
              <b>{t("lore.sealedTitle")}</b>
            </div>
            <div className="lore-seal">
              <p>{plural("lore.sealed", chapter.keys.length)}</p>
              <p className="lore-missing">{t("lore.missing", { cards: missing.join(", ") })}</p>
            </div>
          </>
        )}
        {chapter.keys.length > 0 && (
          <div className="lore-endkeys">
            <p className="lore-see">{t("lore.inChapter")}</p>
            <div className="lore-keys" style={{ "--keys": Math.min(chapter.keys.length, 6) } as CSSProperties}>
              {chapter.keys.map((k) => (
                <Slot key={k.card.uuid} card={k.card} owned={k.owned} onOpen={onOpen} />
              ))}
            </div>
          </div>
        )}
        <span className="lore-end" ref={endRef} />
      </div>
      <span className="lore-folio left">{shown * 2 + 1}</span>
      <span className="lore-folio right">{shown * 2 + 2}</span>
    </div>
  );
}

/** One page (the left or right half) of a sheet, clipped where it stands: on the book, or on a
 * face of the turning leaf. */
function Clip({
  className,
  show,
  place,
  view,
  measured = true,
  onOpen,
  onLayout,
}: {
  className: string;
  show: "left" | "right";
  place: Place;
  view: LoreView;
  measured?: boolean;
  onOpen: (uuid: string) => void;
  onLayout?: (count: number, shown: number) => void;
}) {
  const chapter = view.chapters[place.chapter];
  if (!chapter) return null;
  return (
    <div className={`lore-clip show-${show} ${className}`}>
      <Sheet chapter={chapter} index={place.chapter} spread={place.spread} measured={measured} onOpen={onOpen} onLayout={onLayout} />
    </div>
  );
}

type Turn = { dir: "next" | "prev"; from: Place; to: Place; id: number };

/** A set's story as an open book (maquette A of `.cache/da/v19.html`): the chapter's text flows
 * from the left page to the right one, its art engraved at its head and its key cards at its end,
 * over as many spreads as it needs; ‹ › and ←/→ turn the pages, then the chapters, and the tabs
 * jump to a chapter. A page turns like a real leaf, printed on both sides: going forward, its
 * front is the right page being left and its back the next left page, while the old left page
 * stays until the leaf lands on it and the next right page shows as the leaf lifts (backward, the
 * other way round): nothing appears before its time. A chapter stays sealed until every key card
 * is owned (maquette D of `v18.html`): its title, a seal and its keys only. */
export function LoreBook({
  view,
  at,
  onAt,
  onOpen,
  active,
}: {
  view: LoreView;
  at: number;
  onAt: (at: number) => void;
  onOpen: (uuid: string) => void;
  /** False while something covers the book (card details): the arrow keys are theirs. */
  active: boolean;
}) {
  const { t } = useI18n();
  const last = view.chapters.length - 1;
  const current = Math.min(at, last);
  const [spread, setSpread] = useState<number | "last">(0);
  const [shown, setShown] = useState({ count: 1, spread: 0 });
  const [turn, setTurn] = useState<Turn | null>(null);
  // The spread to open a chapter on once the parent has moved to it (else its first).
  const pending = useRef<number | "last" | null>(null);

  useLayoutEffect(() => {
    setSpread(pending.current ?? 0);
    pending.current = null;
  }, [current, view.setCode]);

  // The book's own sheet reports what it shows; during a turn it already shows where the turn
  // goes, under the turning pages, so "last" becomes a number for the back of the leaf, which is
  // drawn mirrored and cannot be measured.
  const onShown = useCallback((count: number, at: number) => {
    setShown((old) => (old.count === count && old.spread === at ? old : { count, spread: at }));
    setTurn((old) => (old && old.to.spread === "last" ? { ...old, to: { ...old.to, spread: at } } : old));
  }, []);

  const land = (to: Place) => {
    if (to.chapter === current) setSpread(to.spread);
    else {
      pending.current = to.spread;
      onAt(to.chapter);
    }
  };
  const go = (dir: "next" | "prev", to: Place) => {
    if (turn) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) land(to);
    else setTurn({ dir, from: { chapter: current, spread: shown.spread }, to, id: Date.now() });
  };
  const next = () => {
    if (shown.spread < shown.count - 1) go("next", { chapter: current, spread: shown.spread + 1 });
    else if (current < last) go("next", { chapter: current + 1, spread: 0 });
  };
  const previous = () => {
    if (shown.spread > 0) go("prev", { chapter: current, spread: shown.spread - 1 });
    else if (current > 0) go("prev", { chapter: current - 1, spread: "last" });
  };
  const jump = (i: number) => {
    if (i !== current) go(i > current ? "next" : "prev", { chapter: i, spread: 0 });
  };

  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") next();
      else if (e.key === "ArrowLeft") previous();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const chapter = view.chapters[current];
  if (!chapter) return null;
  const atStart = current === 0 && shown.spread === 0;
  const atEnd = current === last && shown.spread >= shown.count - 1;
  const toSpread = turn && typeof turn.to.spread === "number" ? turn.to.spread : 0;
  return (
    <div className="lore">
      <div className="lore-tabs" role="tablist">
        {view.chapters.map((c, i) => (
          <button
            key={i}
            role="tab"
            aria-selected={i === current}
            className={`${i === current ? "active" : ""} ${c.unlocked ? "" : "sealed"}`}
            title={c.title}
            onClick={() => jump(i)}
          >
            {roman(i)}
          </button>
        ))}
      </div>
      <div className="lore-book">
        <button className="lore-turn left" disabled={atStart || !!turn} aria-label={t("lore.previous")} onClick={previous}>
          ‹
        </button>
        <div className="lore-spread">
          {/* Always there, so that nothing is drawn anew when a turn ends: during one it already
              shows where the turn goes, hidden under the turning pages. */}
          <Sheet
            chapter={turn ? (view.chapters[turn.to.chapter] ?? chapter) : chapter}
            index={turn ? turn.to.chapter : current}
            spread={turn ? turn.to.spread : spread}
            measured
            onOpen={onOpen}
            onLayout={onShown}
          />
          {turn && (
            <div className="lore-turning" key={turn.id}>
              {/* Under the leaf: the page it leaves uncovered shows where it goes, the other one
                  where it comes from, until the leaf lands on it. */}
              <Clip className="at-left" show="left" place={turn.dir === "next" ? turn.from : turn.to} view={view} onOpen={onOpen} />
              <Clip className="at-right" show="right" place={turn.dir === "next" ? turn.to : turn.from} view={view} onOpen={onOpen} />
              <div
                className={`lore-leaf ${turn.dir}`}
                style={{ animationDuration: `${TURN_MS}ms` }}
                onAnimationEnd={(e) => {
                  if (e.target !== e.currentTarget) return;
                  land({ ...turn.to, spread: toSpread });
                  setTurn(null);
                }}
              >
                <Clip className="front" show={turn.dir === "next" ? "right" : "left"} place={turn.from} view={view} onOpen={onOpen} />
                <Clip
                  className="back"
                  show={turn.dir === "next" ? "left" : "right"}
                  place={{ ...turn.to, spread: toSpread }}
                  view={view}
                  measured={false}
                  onOpen={onOpen}
                />
              </div>
            </div>
          )}
        </div>
        <button className="lore-turn right" disabled={atEnd || !!turn} aria-label={t("lore.next")} onClick={next}>
          ›
        </button>
      </div>
      <p className="lore-pages">
        {t("lore.pages", { n: roman(current), from: shown.spread * 2 + 1, to: shown.spread * 2 + 2, total: shown.count * 2 })}
      </p>
    </div>
  );
}

/** Beside it: the plane, the chapters opened and the characters (their cards, owned or not). */
export function LorePanel({
  view,
  year,
  onShop,
  onOpen,
}: {
  view: LoreView;
  year: string | null;
  onShop?: () => void;
  onOpen: (uuid: string) => void;
}) {
  const { t } = useI18n();
  const owned = view.characters.filter((c) => c.owned).length;
  const art = view.illustration;
  return (
    <aside className="frame set-side lore-side">
      <div className="cartouche">{view.plane}</div>
      <div className="scroll">
        {art && <Plate art={art} onOpen={onOpen} />}
        <div className="ready-line">
          <span>{t("lore.plane")}</span>
          <b>{view.plane}</b>
        </div>
        {year && (
          <div className="ready-line">
            <span>{t("lore.released")}</span>
            <b>{year}</b>
          </div>
        )}
        <div className="ready-line">
          <span>{t("lore.chapters")}</span>
          <b>
            {view.unlocked} / {view.total}
          </b>
        </div>
        <div className="lore-gauge">
          <span style={{ width: `${view.total === 0 ? 0 : (view.unlocked * 100) / view.total}%` }} />
        </div>
        {view.characters.length > 0 && (
          <>
            <h3>
              {t("lore.characters")} · {owned} / {view.characters.length}
            </h3>
            <div className="lore-cast">
              {view.characters.map((c) => (
                <div key={c.card.uuid} className={`lore-who ${c.owned ? "" : "off"}`}>
                  <Slot card={c.card} owned={c.owned} onOpen={onOpen} />
                  <span>
                    <b>{c.name}</b>
                    <em>{c.role}</em>
                  </span>
                </div>
              ))}
            </div>
          </>
        )}
        <p className="hint cycles-note">{t("lore.hint")}</p>
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
