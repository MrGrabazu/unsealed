// The artifacts' drawings (original, 24×24), by the `glyph` of `config/artifacts.json`.
// Filled shapes take the colour; the parts drawn as lines carry a stroke width. A glyph the
// list does not know shows a plain curio.

const KEY = '<circle cx="7" cy="12" r="4" fill="none" stroke-width="2"/><path d="M11 12h10M17 12v4M20 12v3" stroke-width="2" stroke-linecap="round"/>';

const GLYPHS: Record<string, string> = {
  loupe: '<circle cx="10" cy="10" r="6" fill="none" stroke-width="2"/><path d="M14.5 14.5l6 6" stroke-width="3" stroke-linecap="round"/><path d="M7 8a4 4 0 0 1 3-2" fill="none" stroke-width="1.2"/>',
  hourglass: '<path d="M6 3h12M6 21h12M7 3c0 5 10 5 10 9s-10 4-10 9M17 3c0 5-10 5-10 9s10 4 10 9" fill="none" stroke-width="1.8"/><path d="M9 18h6l-3-3z"/>',
  purse: '<path d="M8 7c0-3 8-3 8 0" fill="none" stroke-width="1.8"/><path d="M5 8h14l-1.5 11a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2z"/><circle cx="12" cy="14" r="2" class="hole"/>',
  token: '<circle cx="12" cy="12" r="8.5" fill="none" stroke-width="2"/><circle cx="12" cy="12" r="5.5"/><path d="M10 10v3a2 2 0 0 0 4 0v-3" fill="none" class="hole-line" stroke-width="1.5"/>',
  seal: '<path d="M12 2l2 2.5 3-.6.8 3 2.8 1.3-.9 3 1.7 2.6-2.4 1.9.2 3.1-3.1.4-1.5 2.7-2.6-1.6-2.6 1.6-1.5-2.7-3.1-.4.2-3.1L2.6 14.4l1.7-2.6-.9-3 2.8-1.3.8-3 3 .6z"/><path d="M9.5 9v3.2a2.5 2.5 0 0 0 5 0V9" fill="none" class="hole-line" stroke-width="1.6"/>',
  lectern: '<path d="M4 7l8 3 8-3-8-3z"/><path d="M12 10v10M8 21h8" stroke-width="2" stroke-linecap="round"/>',
  quill: '<path d="M20 3C12 5 7 11 5 19l2 .5C9 13 13 8 20 3z"/><path d="M4 21l3-3" stroke-width="1.6" stroke-linecap="round"/>',
  die: '<rect x="4" y="4" width="16" height="16" rx="3"/><g class="hole"><circle cx="8.5" cy="8.5" r="1.4"/><circle cx="15.5" cy="15.5" r="1.4"/><circle cx="12" cy="12" r="1.4"/></g>',
  horseshoe: '<path d="M6 4v8a6 6 0 0 0 12 0V4" fill="none" stroke-width="3.2" stroke-linecap="round"/><g class="hole"><circle cx="6" cy="8" r=".8"/><circle cx="18" cy="8" r=".8"/></g>',
  lantern: '<path d="M9 3h6M12 3v2M8 6h8l-1 12H9z"/><path d="M7 18h10v2H7z"/><path d="M12 9c1.5 2 1.5 4 0 6-1.5-2-1.5-4 0-6z" class="hole"/>',
  coffer: '<path d="M3 9a3 3 0 0 1 3-3h12a3 3 0 0 1 3 3v2H3z"/><path d="M3 12h18v7H3z"/><rect x="10.5" y="10" width="3" height="4" class="hole"/>',
  balance: '<path d="M12 3v17M7 21h10M5 7h14" stroke-width="1.8" stroke-linecap="round"/><path d="M5 7l-3 7h6zM19 7l-3 7h6z"/>',
  eye: '<path d="M2 12c3-5 7-7 10-7s7 2 10 7c-3 5-7 7-10 7S5 17 2 12z"/><circle cx="12" cy="12" r="3.5" class="hole"/><circle cx="12" cy="12" r="1.5"/>',
  crown: '<path d="M3 18l2-10 4 4 3-6 3 6 4-4 2 10z"/><path d="M3 20h18" stroke-width="2"/>',
  compass: '<circle cx="12" cy="12" r="9" fill="none" stroke-width="1.8"/><path d="M12 5l2.2 7H9.8z"/><path d="M12 19l-2.2-7h4.4z" class="hole"/><circle cx="12" cy="12" r="1.2"/>',
  "broken-seal": '<path d="M11 2.5l-2 4 2 3-2.5 4 2 3.5-1.5 4.5C5 20.5 2.5 17 2.5 12S6 3 11 2.5z"/><path d="M13.5 2.6C18 3.4 21.5 7.3 21.5 12s-3.5 8.6-8 9.4l1.5-4.4-2-3.5 2.5-4-2-3z"/>',
  grimoire: '<path d="M5 3h12a2 2 0 0 1 2 2v16H7a2 2 0 0 1-2-2z"/><path d="M7 21a2 2 0 0 1 0-4h12" class="hole-line" fill="none" stroke-width="1.4"/><path d="M12 6l1.2 2.6 2.8.3-2.1 1.9.6 2.8-2.5-1.5-2.5 1.5.6-2.8-2.1-1.9 2.8-.3z" class="hole"/>',
  prism: '<path d="M12 3l8 15H4z"/><path d="M1 13l8-1.5" stroke-width="1.4" stroke-linecap="round"/><path d="M16 12l7-2M16.5 13l7 0M17 14l7 2" stroke-width="1.2" stroke-linecap="round"/>',
  monocle: '<circle cx="10" cy="10" r="6.5" fill="none" stroke-width="2.4"/><path d="M14.6 14.6c2 2 2.5 4 2 6.5" fill="none" stroke-width="1.4" stroke-linecap="round"/><path d="M7 8.5a3.5 3.5 0 0 1 2.5-2.3" fill="none" stroke-width="1.2"/>',
  "master-key": '<path d="M7 3a4 4 0 1 1 0 8 4 4 0 0 1 0-8z"/><circle cx="7" cy="7" r="1.6" class="hole"/><path d="M10 9.5l10 10M15 14.5l-2 2M18 17.5l-2 2" stroke-width="2.2" stroke-linecap="round"/>',
  horn: '<path d="M3 6c5 0 12 3 15 9l3-1-1 6-5-2 2-1C14 12 9 9 3 9z"/><circle cx="19" cy="20" r="1.6"/><circle cx="15.5" cy="21" r="1.3"/>',
  clepsydra: '<path d="M6 3h12l-4 7v1l4 7H6l4-7v-1z"/><path d="M12 11v3" class="hole-line" stroke-width="1.2"/><path d="M5 21h14" stroke-width="2" stroke-linecap="round"/><path d="M9 16.5h6l1 1.5H8z" class="hole"/>',
  star: '<path d="M12 2l2.6 6.6L21.5 9l-5.3 4.5 1.7 7L12 16.8 6.1 20.5l1.7-7L2.5 9l6.9-.4z"/><path d="M12 7.5l1.2 3 3.2.2-2.5 2 .8 3.1L12 14l-2.7 1.8.8-3.1-2.5-2 3.2-.2z" class="hole"/>',
  clover: '<circle cx="8.5" cy="8.5" r="3.8"/><circle cx="15.5" cy="8.5" r="3.8"/><circle cx="8.5" cy="15.5" r="3.8"/><circle cx="15.5" cy="15.5" r="3.8"/><path d="M12 12c2 3 4 6 7 9" fill="none" stroke-width="1.6" stroke-linecap="round"/>',
  egg: '<path d="M12 2c4 0 7 6.5 7 11.5A7 7 0 0 1 5 13.5C5 8.5 8 2 12 2z"/><path d="M8 11l2 1.5 2-2 2 2 2-1.5" fill="none" class="hole-line" stroke-width="1.2"/><path d="M8.5 16.5l2-1 1.5 1.5 1.5-1.5 2 1" fill="none" class="hole-line" stroke-width="1.2"/>',
  "key-bronze": KEY,
  "key-silver": KEY,
  "key-gold": KEY,
  "key-diamond": KEY,
  "key-obsidian": KEY,
};

const CURIO = '<circle cx="12" cy="12" r="7"/>';

/** The metal an artifact shines in: its own for the keys, else its rarity's. */
const METALS: Record<string, string> = {
  common: "#c9bfa8",
  uncommon: "#cdd6e0",
  rare: "#f0c35e",
  mythic: "#ff9a4a",
  "key-bronze": "#d08b4c",
  "key-silver": "#e3e7ee",
  "key-gold": "#f5cf63",
  "key-diamond": "#a9ecff",
  "key-obsidian": "#9d7bd8",
};

export function artifactMetal(glyph: string, rarity: string): string {
  return METALS[glyph] ?? METALS[rarity] ?? METALS.common;
}

/** An artifact's drawing, in `color`; as a dark silhouette when it is still a mystery. */
export function ArtifactGlyph({ glyph, color, silhouette = false, className = "" }: { glyph: string; color: string; silhouette?: boolean; className?: string }) {
  return (
    <svg
      className={`artifact-glyph ${silhouette ? "silhouette" : ""} ${className}`}
      viewBox="0 0 24 24"
      fill="currentColor"
      stroke="currentColor"
      strokeWidth={0}
      style={{ color: silhouette ? undefined : color }}
      aria-hidden="true"
      dangerouslySetInnerHTML={{ __html: GLYPHS[glyph] ?? CURIO }}
    />
  );
}
