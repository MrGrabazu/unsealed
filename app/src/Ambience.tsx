/** Dust floating in the light, always the same (drawn once), and a vignette on the edges:
 * the game's ambience, behind its panels. */
const MOTES = Array.from({ length: 22 }, (_, i) => ({
  left: `${(i * 41) % 100}%`,
  top: `${30 + ((i * 29) % 70)}%`,
  animationDelay: `${-((i * 0.9) % 14)}s`,
  animationDuration: `${10 + ((i * 7) % 8)}s`,
}));

export function Ambience() {
  return (
    <>
      <div className="game-vignette" aria-hidden="true" />
      <div className="motes" aria-hidden="true">
        {MOTES.map((style, i) => (
          <i key={i} style={style} />
        ))}
      </div>
    </>
  );
}
