import { useEffect, useMemo, useState } from "react";
import type { CollectionCard } from "./api";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Money } from "./money";
import { sheenClass } from "./RevealedCards";
import { sounds } from "./sound";

/** Cards per binder page: 3 rows of 3, like a real binder page. */
const PER_PAGE = 9;

export interface BinderSection {
  /** Names the section's tab (none for the set's main cards). */
  title: string | null;
  cards: CollectionCard[];
}

interface Page {
  /** Index of the section the page belongs to. */
  section: number;
  cards: CollectionCard[];
}

/** Cover materials of the binders (see `.binder-spread[data-cover]` in the styles). */
const COVERS = ["leather", "canvas", "vinyl", "suede"] as const;

/** Each set's binder has its own cover, colour and material, drawn from its code: the same
 * every time, with nothing to store (the library's shelves show the same covers). */
export function coverOf(setCode: string) {
  return { hue: Math.floor(seeded(setCode, 101) * 360), material: COVERS[Math.floor(seeded(setCode, 102) * COVERS.length)] };
}

interface Props {
  setCode: string;
  sections: BinderSection[];
  onOpen: (uuid: string) => void;
  /** False while something covers the binder (card details): the arrow keys are theirs. */
  active: boolean;
  /** Called with the index (in `sections`) of the section shown, when it changes. */
  onSection?: (section: number) => void;
}

/** A set's collection as a binder: pages of 9 pockets, two facing pages at a time, every
 * card in its place (missing ones greyed out). Each section starts on a new page; nothing
 * is written on the pages, as in a real binder: a card's name and price show on hover. */
export function Binder({ setCode, sections, onOpen, active, onSection }: Props) {
  const { t } = useI18n();
  const cover = coverOf(setCode);
  const pages = useMemo(() => {
    const out: Page[] = [];
    sections.forEach((section, index) => {
      for (let i = 0; i < section.cards.length; i += PER_PAGE) {
        out.push({ section: index, cards: section.cards.slice(i, i + PER_PAGE) });
      }
    });
    return out;
  }, [sections]);
  const spreads = Math.max(1, Math.ceil(pages.length / 2));
  const [spread, setSpread] = useState(0);
  const go = (to: number) => {
    const target = Math.min(spreads - 1, Math.max(0, to));
    if (target === spread) return;
    sounds.page();
    setSpread(target);
  };

  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowRight") go(spread + 1);
      else if (e.key === "ArrowLeft") go(spread - 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // The section shown: the left page's, or the right one's where a new section starts on
  // the right (as a jump to that section's tab shows it).
  const [leftPage, rightPage] = [pages[spread * 2], pages[spread * 2 + 1]];
  const shownSection = leftPage && rightPage && rightPage.section !== leftPage.section ? rightPage.section : (leftPage?.section ?? 0);
  useEffect(() => onSection?.(shownSection), [shownSection]);

  // Tabs to jump to a section (the main cards, collector extras, tokens).
  const firstSpreadOf = (section: number) => Math.floor(pages.findIndex((p) => p.section === section) / 2);
  const from = spread * 2 + 1;
  const to = Math.min(pages.length, from + 1);

  return (
    <div className="binder">
      {sections.length > 1 && (
        <div className="binder-bar">
          {sections.map((s, i) => {
            const first = firstSpreadOf(i);
            const next = sections.findIndex((_, j) => j > i);
            const inSection = spread >= first && (next < 0 || spread < firstSpreadOf(next));
            return (
              <button key={i} className={`binder-tab ${inSection ? "active" : ""}`} onClick={() => go(first)}>
                {s.title ?? t("binder.cards")}
              </button>
            );
          })}
        </div>
      )}
      {/* The page turners sit on either side of the binder, where a hand would turn them. */}
      <div className="binder-frame">
        <button
          className="binder-turn"
          onClick={() => go(spread - 1)}
          disabled={spread === 0}
          aria-label={t("binder.previous")}
          title={t("binder.previous")}
        >
          ‹
        </button>
        <div
          className="binder-spread"
          data-cover={cover.material}
          style={{ "--cover-h": cover.hue } as React.CSSProperties}
        >
          <BinderPage page={pages[spread * 2]} side="left" onOpen={onOpen} />
          <div className="binder-spine" aria-hidden="true" />
          <BinderPage page={pages[spread * 2 + 1]} side="right" onOpen={onOpen} />
        </div>
        <button
          className="binder-turn"
          onClick={() => go(spread + 1)}
          disabled={spread >= spreads - 1}
          aria-label={t("binder.next")}
          title={t("binder.next")}
        >
          ›
        </button>
      </div>
      <p className="binder-pages hint">{t("binder.pages", { from, to, total: pages.length })}</p>
    </div>
  );
}

function BinderPage({ page, side, onOpen }: { page: Page | undefined; side: "left" | "right"; onOpen: (uuid: string) => void }) {
  const pockets: (CollectionCard | null)[] = page ? [...page.cards] : [];
  while (pockets.length < PER_PAGE) pockets.push(null);
  return (
    <div className={`binder-page ${side}`}>
      <div className="binder-pockets">
        {pockets.map((card, i) =>
          card ? (
            <Pocket key={card.uuid} card={card} onOpen={onOpen} />
          ) : (
            <div key={`empty-${i}`} className="pocket empty" aria-hidden="true" />
          ),
        )}
      </div>
    </div>
  );
}

/** A number in [0, 1) drawn from a text, the same every time (a card keeps its scrap). */
export function seeded(text: string, salt: number): number {
  let h = 2166136261 ^ salt;
  for (const c of text) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return ((h >>> 0) % 10000) / 10000;
}

const INKS = ["#1d2b5c", "#202020", "#2a3f8f", "#3a3a46", "#1f3d6b"];

/** Where a card is still missing: a scrap of paper cut in a hurry, its name (in the card
 * language) and number written by hand, slanted, as collectors do in real binders. */
export function Placeholder({ card }: { card: CollectionCard }) {
  const r = (salt: number) => seeded(card.uuid, salt);
  const style = {
    "--paper-tilt": `${(r(1) - 0.5) * 9}deg`,
    "--paper-width": `${78 + r(2) * 16}%`,
    "--paper-height": `${34 + r(3) * 20}%`,
    "--paper-x": `${(r(4) - 0.5) * 10}%`,
    "--paper-y": `${(r(5) - 0.5) * 16}%`,
    "--ink-tilt": `${(r(6) - 0.5) * 8}deg`,
    "--ink": INKS[Math.floor(r(7) * INKS.length)],
    "--ink-size": `${0.92 + r(8) * 0.2}`,
    // A cut edge: each corner a little off.
    clipPath: `polygon(${r(9) * 4}% ${r(10) * 6}%, ${100 - r(11) * 4}% ${r(12) * 5}%, ${100 - r(13) * 3}% ${100 - r(14) * 6}%, ${r(15) * 4}% ${100 - r(16) * 5}%)`,
  } as React.CSSProperties;
  return (
    <div className="paper" style={style} aria-hidden="true">
      <span className="paper-name">{card.name}</span>
      <span className="paper-number">#{card.number}</span>
    </div>
  );
}

function Pocket({ card, onOpen }: { card: CollectionCard; onOpen: (uuid: string) => void }) {
  const { t } = useI18n();
  const total = card.quantity + card.foilQuantity;
  return (
    <div
      className={`pocket ${total > 0 ? "" : "missing"}`}
      role="button"
      tabIndex={0}
      aria-label={`#${card.number} ${card.name}`}
      onClick={() => onOpen(card.uuid)}
      onKeyDown={(e) => e.key === "Enter" && onOpen(card.uuid)}
    >
      <div className="thumb">
        {total > 0 ? (
          <CardImage front={card.imageUrl} back={card.backImageUrl} alt={card.name} lazy />
        ) : (
          <Placeholder card={card} />
        )}
        {card.foilQuantity > 0 && <div className={sheenClass(card.rarity)} />}
        {total > 1 && <span className="badge-qty">×{total}</span>}
        <div className="pocket-hover">
          <span className="name">
            #{card.number} {card.name}
          </span>
          {card.priceCents != null ? (
            <Money cents={card.priceCents} estimated={card.priceEstimated} />
          ) : (
            <span>{t("common.none")}</span>
          )}
        </div>
      </div>
    </div>
  );
}
