import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { getLibrary, type LibraryBinder, type SetSummary } from "./api";
import { coverOf, seeded } from "./Binder";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { Money } from "./money";
import { SetIcon } from "./SetIcon";

/** Room between two binders on a shelf, and on either side of the shelf (keep in sync with
 * `.shelf-books` and `.shelf` in theme.css). */
const GAP = 2;
const SHELF_PADDING = 6;
/** Cards per binder page, as in `Binder.tsx`. */
const PER_PAGE = 9;
/** Shelves drawn even when there are few binders: a bookcase, not a single plank. */
const MIN_SHELVES = 2;

interface Props {
  sets: SetSummary[];
  /** Changes whenever the collection may have changed elsewhere (a booster was opened). */
  refreshKey: number;
  /** Opens a set's binder. */
  onOpen: (setCode: string) => void;
}

interface Book {
  binder: LibraryBinder;
  set: SetSummary | undefined;
  width: number;
  height: number;
}

/** A binder's thickness follows its number of pages, as a real one would: from a slim
 * binder for a small set to a thick one for a set of several hundred cards. */
function widthOf(b: LibraryBinder): number {
  const pages = Math.ceil((b.totalCount + b.extrasTotal + b.tokensTotal + b.artsTotal) / PER_PAGE);
  return Math.round(Math.max(34, Math.min(110, 26 + pages * 0.9)));
}

/** Room along a spine's label for its name (px), and Cinzel's mean capital width (em). */
const LABEL_LENGTH = 138;
const CHAR_WIDTH = 0.74;

/** The name fills its label: smaller when long, and on up to three lines when the spine is
 * wide enough and fewer lines would be too small to read. */
function nameStyle(name: string, spineWidth: number): React.CSSProperties {
  const labelWidth = spineWidth - 24;
  let best = { lines: 1, size: 0 };
  for (let lines = 1; lines <= 3; lines++) {
    const size = Math.min(13, (lines * LABEL_LENGTH) / (CHAR_WIDTH * name.length));
    if (lines > 1 && lines * size * 1.15 > labelWidth) break;
    best = { lines, size };
    if (size >= 10) break;
  }
  const fontSize = Math.max(8, best.size);
  return best.lines === 1 ? { fontSize } : { fontSize, whiteSpace: "normal", lineHeight: 1.1 };
}

/** The collection as a bookcase: one binder per set owned, standing on shelves, oldest first.
 * Each binder has the cover of its set's binder; hovering one tells how full it is, clicking
 * it opens it. */
export function Library({ sets, refreshKey, onOpen }: Props) {
  const { t, plural } = useI18n();
  const [binders, setBinders] = useState<LibraryBinder[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [width, setWidth] = useState(0);
  const [hover, setHover] = useState<{ book: Book; x: number; y: number } | null>(null);
  const caseRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getLibrary()
      .then((list) => {
        setBinders(list);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, [refreshKey]);

  // Shelves are filled to the bookcase's width: follow it.
  useLayoutEffect(() => {
    const node = caseRef.current;
    if (!node) return;
    const observer = new ResizeObserver(() => setWidth(node.clientWidth));
    observer.observe(node);
    setWidth(node.clientWidth);
    return () => observer.disconnect();
  }, [binders !== null]);

  const books = useMemo<Book[]>(() => {
    const byCode = new Map(sets.map((s) => [s.code, s]));
    return (binders ?? []).map((binder) => ({
      binder,
      set: byCode.get(binder.setCode),
      width: widthOf(binder),
      height: 226 + Math.round(seeded(binder.setCode, 103) * 22),
    }));
  }, [binders, sets]);

  // Binders fill a shelf, then the next one.
  const shelves = useMemo(() => {
    const room = Math.max(200, width - SHELF_PADDING * 2);
    const rows: Book[][] = [];
    let used = Infinity;
    for (const book of books) {
      if (used + GAP + book.width > room) {
        rows.push([]);
        used = -GAP;
      }
      rows[rows.length - 1].push(book);
      used += GAP + book.width;
    }
    while (rows.length < MIN_SHELVES) rows.push([]);
    return rows;
  }, [books, width]);

  if (error) return <p className="error">{error}</p>;
  if (!binders) return <Loading label={t("collection.loading")} />;

  const totalValue = binders.reduce((sum, b) => sum + b.valueCents, 0);
  return (
    <div className="library">
      <p className="progress-label">
        {binders.length === 0 ? (
          t("library.empty")
        ) : (
          <>
            {plural("library.summary", binders.length)} · <Money cents={totalValue} />
          </>
        )}
      </p>
      <div className="bookcase" ref={caseRef} onMouseLeave={() => setHover(null)}>
        {shelves.map((row, i) => (
          <div key={i} className="shelf">
            <div className="shelf-books">
              {row.map((book) => (
                <Spine
                  key={book.binder.setCode}
                  book={book}
                  onOpen={onOpen}
                  onHover={(x, y) => setHover({ book, x, y })}
                  onLeave={() => setHover(null)}
                />
              ))}
            </div>
            <div className="shelf-plank" aria-hidden="true" />
          </div>
        ))}
      </div>
      {hover && <BinderTooltip book={hover.book} x={hover.x} y={hover.y} />}
    </div>
  );
}

function Spine({
  book,
  onOpen,
  onHover,
  onLeave,
}: {
  book: Book;
  onOpen: (setCode: string) => void;
  onHover: (x: number, y: number) => void;
  onLeave: () => void;
}) {
  const { binder, set } = book;
  const cover = coverOf(binder.setCode);
  const name = set?.name ?? binder.setCode;
  const complete = binder.totalCount > 0 && binder.ownedCount >= binder.totalCount;
  return (
    <button
      className={`library-spine ${complete ? "complete" : ""}`}
      data-cover={cover.material}
      style={{ "--cover-h": cover.hue, width: book.width, height: book.height } as React.CSSProperties}
      aria-label={name}
      onClick={() => onOpen(binder.setCode)}
      onMouseMove={(e) => onHover(e.clientX, e.clientY)}
      onMouseLeave={onLeave}
    >
      <span className="spine-icon">
        {set?.iconUrl ? <SetIcon url={set.iconUrl} name={name} /> : <span className="spine-star">✦</span>}
      </span>
      <span className="spine-label">
        <span className="spine-name" style={nameStyle(name, book.width)}>
          {name}
        </span>
      </span>
      <span className="spine-code">{binder.setCode}</span>
    </button>
  );
}

/** What a binder holds, next to the mouse (and kept inside the window). */
function BinderTooltip({ book, x, y }: { book: Book; x: number; y: number }) {
  const { t, plural, formatNumber } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 300, height: 320 });
  useLayoutEffect(() => {
    if (ref.current) setSize({ width: ref.current.offsetWidth, height: ref.current.offsetHeight });
  }, [book]);
  const { binder: b, set } = book;
  const left = x + 18 + size.width > window.innerWidth ? x - 18 - size.width : x + 18;
  const top = Math.max(8, Math.min(y + 18, window.innerHeight - size.height - 8));
  const count = (owned: number, total: number) => `${formatNumber(owned)} / ${formatNumber(total)}`;
  const rows: [string, React.ReactNode][] = [
    [t("library.rares"), count(b.raresOwned, b.raresTotal)],
    [t("library.mythics"), count(b.mythicsOwned, b.mythicsTotal)],
  ];
  if (b.extrasTotal > 0) rows.push([t("collection.extras"), count(b.extrasOwned, b.extrasTotal)]);
  if (b.tokensTotal > 0) rows.push([t("collection.tokens"), count(b.tokensOwned, b.tokensTotal)]);
  if (b.artsTotal > 0) rows.push([t("collection.arts"), count(b.artsOwned, b.artsTotal)]);
  rows.push(
    [t("library.copies"), t("library.copiesValue", { copies: formatNumber(b.copies), foil: formatNumber(b.foilCopies) })],
    [t("library.boosters"), formatNumber(b.boostersOpened)],
    [t("library.value"), <Money cents={b.valueCents} />],
    [t("library.sellValue"), <Money cents={b.sellValueCents} />],
  );
  return (
    <div className="library-tooltip" ref={ref} style={{ left, top }} role="tooltip">
      <div className="library-tooltip-head">
        <SetIcon url={set?.iconUrl ?? null} name={set?.name ?? b.setCode} />
        <div>
          <strong>{set?.name ?? b.setCode}</strong>
          <div className="hint">
            {b.setCode}
            {set && ` · ${set.releaseDate.slice(0, 4)}`}
            {` · ${plural("library.cardCount", b.totalCount)}`}
          </div>
        </div>
      </div>
      <div className="progress">
        <div className="progress-bar" style={{ width: `${b.completionPercent}%` }} />
      </div>
      <p className="library-tooltip-progress">
        {t("collection.progress", {
          owned: formatNumber(b.ownedCount),
          total: formatNumber(b.totalCount),
          percent: b.completionPercent.toFixed(1),
        })}
      </p>
      <dl>
        {rows.map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      {b.bestCard && (
        <p className="library-tooltip-best">
          <span className="hint">{t("library.best")}</span> {b.bestCard.name}
          {b.bestCard.foil && <span className="tag"> {t("collection.foilBadge")}</span>} · <Money cents={b.bestCard.priceCents} />
        </p>
      )}
      <p className="hint library-tooltip-open">{t("library.open")}</p>
    </div>
  );
}
