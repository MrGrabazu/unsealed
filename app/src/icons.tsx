/** Engraved icons of the game's chrome (original drawings, filled with the current colour). */
const PATHS = {
  shop: "M7 7V6a5 5 0 0110 0v1h3l-1.3 13.1A2 2 0 0116.7 22H7.3a2 2 0 01-2-1.9L4 7h3zm2 0h6V6a3 3 0 00-6 0v1zm3 4a3 3 0 100 6 3 3 0 000-6z",
  pack: "M6 2h12l-1 2 1 2v12l-1 2 1 2H6l1-2-1-2V6l1-2-1-2zm3 6v6h6V8H9z",
  book: "M4 3h7a3 3 0 013 3v15a2 2 0 00-2-2H4V3zm16 0h-4a3 3 0 00-1 .2V21a2 2 0 012-2h3V3z",
  trophy:
    "M6 3h12v2h3v3a4 4 0 01-4 4h-.4A6 6 0 0113 15.9V18h3v3H8v-3h3v-2.1A6 6 0 017.4 12H7a4 4 0 01-4-4V5h3V3zm0 4H5v1a2 2 0 001.3 1.9C6.1 9 6 8 6 7zm12 0c0 1-.1 2-.3 2.9A2 2 0 0019 8V7h-1z",
  chart: "M3 21V3h2v16h16v2H3zm4-4V11h3v6H7zm5 0V7h3v10h-3zm5 0v-4h3v4h-3z",
  scroll: "M6 3h11a3 3 0 013 3v1h-3v11a3 3 0 01-3 3H5a3 3 0 01-3-3v-1h3V6a3 3 0 011-3zm2 4v2h7V7H8zm0 4v2h7v-2H8zm0 4v2h5v-2H8z",
  gem: "M6 3h12l4 6-10 12L2 9l4-6zm1.2 2L5 8.5h4.2L11 5H7.2zm9.6 0H13l1.8 3.5H19L16.8 5z",
  star: "M12 2.6l2.9 6 6.6.8-4.9 4.5 1.3 6.5L12 17.2l-5.9 3.2 1.3-6.5-4.9-4.5 6.6-.8z",
  soundOn: "M3 9h4l5-4v14l-5-4H3V9zm13.5 3a4.5 4.5 0 00-2.5-4v8a4.5 4.5 0 002.5-4zM14 3.2v2.1a7 7 0 010 13.4v2.1a9 9 0 000-17.6z",
  soundOff: "M3 9h4l5-4v14l-5-4H3V9zm13.6-.6L18 7l2.5 2.5L23 7l1.4 1.4L21.9 11l2.5 2.5-1.4 1.4-2.5-2.5-2.5 2.5-1.4-1.4 2.5-2.5-2.5-2.6z",
  save: "M5 3h11l4 4v12a2 2 0 01-2 2H6a2 2 0 01-2-2V5a2 2 0 011-2zm2 2v5h9V5H7zm5 8a3 3 0 100 6 3 3 0 000-6z",
  menu: "M3 5h18v3H3V5zm0 5.5h18v3H3v-3zM3 16h18v3H3v-3z",
  dice:
    "M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm2.5 3a1.5 1.5 0 100 3 1.5 1.5 0 000-3zm9 0a1.5 1.5 0 100 3 1.5 1.5 0 000-3zM12 10.5a1.5 1.5 0 100 3 1.5 1.5 0 000-3zM7.5 15a1.5 1.5 0 100 3 1.5 1.5 0 000-3zm9 0a1.5 1.5 0 100 3 1.5 1.5 0 000-3z",
  undo: "M9 5L3 10l6 5v-3.5h6a3.5 3.5 0 010 7h-3v3h3a6.5 6.5 0 000-13H9V5z",
  // A curio under its glass bell (the cabinet of curiosities).
  curio: "M12 2a1.5 1.5 0 0 1 1.5 1.5v.7A7 7 0 0 1 19 11v7h1.5v3h-17v-3H5v-7a7 7 0 0 1 5.5-6.8v-.7A1.5 1.5 0 0 1 12 2zm0 9.5l-2.5 2.5 2.5 2.5 2.5-2.5z",
  keyboard:
    "M3 5h18a2 2 0 012 2v10a2 2 0 01-2 2H3a2 2 0 01-2-2V7a2 2 0 012-2zm1 3v2h2V8H4zm3.5 0v2h2V8h-2zm3.5 0v2h2V8h-2zm3.5 0v2h2V8h-2zM18 8v2h2V8h-2zM5.5 11.5v2h2v-2h-2zm3.5 0v2h2v-2H9zm3.5 0v2h2v-2h-2zm3.5 0v2h2v-2h-2zM7 15v2h10v-2H7z",
} as const;

export type IconName = keyof typeof PATHS;

/** Icons drawn with holes (a die's pips, a keyboard's keys). */
const EVEN_ODD = new Set<IconName>(["dice", "keyboard", "curio"]);

export function Icon({ name, className }: { name: IconName; className?: string }) {
  return (
    <svg className={`icon ${className ?? ""}`} viewBox="0 0 24 24" aria-hidden="true">
      <path d={PATHS[name]} fillRule={EVEN_ODD.has(name) ? "evenodd" : undefined} />
    </svg>
  );
}
