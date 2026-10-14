// The Unsealed logotype: engraved gilded capitals (Cinzel, bundled under the OFL) whose
// "A" is a cracked wax seal, the booster's seal broken open; "Crack & Collect" below,
// between two blades. Original work: nothing of Magic's logo or lettering. Written as an
// SVG string so the preview script (.cache) renders exactly what the game shows.
// `{{id}}` makes the gradient ids unique on a page.

export const LOGO_SVG = /*svg*/ `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1080 250" role="img" aria-label="Unsealed — Crack &amp; Collect">
  <defs>
    <linearGradient id="{{id}}gold" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#fff4c8"/>
      <stop offset="0.32" stop-color="#f0c35e"/>
      <stop offset="0.52" stop-color="#a8742a"/>
      <stop offset="0.6" stop-color="#e9c271"/>
      <stop offset="1" stop-color="#6d4512"/>
    </linearGradient>
    <linearGradient id="{{id}}pale" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#f6e2a8"/>
      <stop offset="1" stop-color="#b98b3e"/>
    </linearGradient>
    <radialGradient id="{{id}}wax" cx="0.38" cy="0.3" r="0.8">
      <stop offset="0" stop-color="#e2545a"/>
      <stop offset="0.45" stop-color="#a61f28"/>
      <stop offset="1" stop-color="#4f080e"/>
    </radialGradient>
    <filter id="{{id}}glow" x="-20%" y="-40%" width="140%" height="180%">
      <feGaussianBlur stdDeviation="9"/>
    </filter>
  </defs>

  <!-- A warm halo behind the word -->
  <ellipse cx="548" cy="122" rx="470" ry="70" fill="#d99a3a" opacity="0.18" filter="url(#{{id}}glow)"/>

  <g font-family="Cinzel, serif" font-weight="900" text-anchor="middle">
    <!-- Depth: the letters' dark extrusion, then the gilded face with a bronze edge -->
    <g fill="#1b0f05" transform="translate(0 6)">
      <text x="110" y="175" font-size="178">U</text>
      <text x="253" y="175" font-size="150">N</text>
      <text x="371" y="175" font-size="150">S</text>
      <text x="475" y="175" font-size="150">E</text>
      <text x="744" y="175" font-size="150">L</text>
      <text x="851" y="175" font-size="150">E</text>
      <text x="982" y="175" font-size="178">D</text>
    </g>
    <g fill="url(#{{id}}gold)" stroke="#3a240c" stroke-width="2.5" paint-order="stroke">
      <text x="110" y="175" font-size="178">U</text>
      <text x="253" y="175" font-size="150">N</text>
      <text x="371" y="175" font-size="150">S</text>
      <text x="475" y="175" font-size="150">E</text>
      <text x="744" y="175" font-size="150">L</text>
      <text x="851" y="175" font-size="150">E</text>
      <text x="982" y="175" font-size="178">D</text>
    </g>
  </g>

  <!-- The A: a wax seal, an A pressed in it, cracked open -->
  <g transform="translate(611 120)">
    <path transform="translate(2 6)" fill="#1b0f05" opacity="0.8" d="M62.6 0.0 C62.8 6.1 65.0 13.1 63.0 18.5 C61.0 23.9 54.6 28.1 50.6 32.5 C46.6 36.9 43.0 40.7 39.0 45.0 C35.0 49.2 31.5 54.6 26.6 58.2 C21.7 61.8 15.4 66.3 9.6 66.6 C3.7 66.8 -2.6 61.3 -8.6 59.9 C-14.7 58.5 -21.1 60.3 -26.7 58.4 C-32.2 56.5 -37.8 52.6 -42.1 48.5 C-46.4 44.4 -48.5 38.6 -52.5 33.7 C-56.4 28.9 -64.0 24.9 -65.8 19.3 C-67.6 13.7 -63.8 6.3 -63.2 0.0 C-62.6 -6.3 -64.3 -12.9 -62.1 -18.2 C-60.0 -23.6 -54.0 -27.7 -50.3 -32.3 C-46.6 -36.9 -43.5 -41.0 -39.9 -46.0 C-36.3 -51.1 -33.7 -59.7 -28.6 -62.6 C-23.4 -65.4 -15.2 -63.9 -9.1 -63.1 C-2.9 -62.2 2.4 -58.4 8.3 -57.5 C14.2 -56.7 21.1 -59.7 26.4 -57.8 C31.8 -56.0 34.9 -49.9 40.4 -46.6 C45.9 -43.4 55.8 -42.9 59.4 -38.2 C63.0 -33.4 61.5 -24.6 62.0 -18.2 C62.6 -11.9 62.5 -6.1 62.6 0.0 Z"/>
    <path fill="url(#{{id}}wax)" stroke="#3d060b" stroke-width="2" d="M62.6 0.0 C62.8 6.1 65.0 13.1 63.0 18.5 C61.0 23.9 54.6 28.1 50.6 32.5 C46.6 36.9 43.0 40.7 39.0 45.0 C35.0 49.2 31.5 54.6 26.6 58.2 C21.7 61.8 15.4 66.3 9.6 66.6 C3.7 66.8 -2.6 61.3 -8.6 59.9 C-14.7 58.5 -21.1 60.3 -26.7 58.4 C-32.2 56.5 -37.8 52.6 -42.1 48.5 C-46.4 44.4 -48.5 38.6 -52.5 33.7 C-56.4 28.9 -64.0 24.9 -65.8 19.3 C-67.6 13.7 -63.8 6.3 -63.2 0.0 C-62.6 -6.3 -64.3 -12.9 -62.1 -18.2 C-60.0 -23.6 -54.0 -27.7 -50.3 -32.3 C-46.6 -36.9 -43.5 -41.0 -39.9 -46.0 C-36.3 -51.1 -33.7 -59.7 -28.6 -62.6 C-23.4 -65.4 -15.2 -63.9 -9.1 -63.1 C-2.9 -62.2 2.4 -58.4 8.3 -57.5 C14.2 -56.7 21.1 -59.7 26.4 -57.8 C31.8 -56.0 34.9 -49.9 40.4 -46.6 C45.9 -43.4 55.8 -42.9 59.4 -38.2 C63.0 -33.4 61.5 -24.6 62.0 -18.2 C62.6 -11.9 62.5 -6.1 62.6 0.0 Z"/>
    <!-- The pressed ring and the A stamped inside it -->
    <circle r="44" fill="none" stroke="#5c0a10" stroke-width="5" opacity="0.8"/>
    <circle r="44" fill="none" stroke="#f27a7c" stroke-width="1.5" opacity="0.45" transform="translate(-1 -1.5)"/>
    <text y="27" font-family="Cinzel, serif" font-weight="900" font-size="78" text-anchor="middle" fill="#ef7276" opacity="0.5" transform="translate(-1.5 -2)">A</text>
    <text y="27" font-family="Cinzel, serif" font-weight="900" font-size="78" text-anchor="middle" fill="#6e0c14">A</text>
    <!-- The crack, from rim to rim -->
    <path d="M24 -58 L13 -34 L22 -17 L4 4 L12 22 L-6 42 L-12 64" fill="none" stroke="#2a0306" stroke-width="5" stroke-linejoin="bevel"/>
    <path d="M26 -57 L15 -33 L24 -17 L6 4 L14 22 L-4 42 L-10 63" fill="none" stroke="#ff9a9c" stroke-width="1.4" opacity="0.6" stroke-linejoin="bevel"/>
  </g>

  <!-- Crack & Collect, between two blades -->
  <g fill="url(#{{id}}pale)">
    <path d="M196 222 L312 218 L320 222 L312 226 Z"/>
    <path d="M900 222 L784 218 L776 222 L784 226 Z"/>
    <path d="M178 222 l9 -7 l9 7 l-9 7 Z"/>
    <path d="M918 222 l-9 -7 l-9 7 l9 7 Z"/>
    <text x="548" y="233" font-family="Cinzel, serif" font-weight="700" font-size="32" letter-spacing="9" text-anchor="middle">CRACK &amp; COLLECT</text>
  </g>
</svg>`;

/** The logo's markup, its gradient ids made unique with `id`. */
export function logoSvg(id: string): string {
  return LOGO_SVG.replaceAll("{{id}}", id);
}
