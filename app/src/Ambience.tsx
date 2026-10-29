import { useEffect, useState } from "react";
import { findEgg } from "./eggs";
import { useSeason, type Season } from "./seasons";
import { sounds } from "./sound";

/** Dust floating in the light, always the same (drawn once), and a vignette on the edges:
 * the game's ambience, behind its panels. On the feasts of the year, embers, snow or petals,
 * more of them (seasons.tsx). */
const MOTES = Array.from({ length: 90 }, (_, i) => ({
  left: `${(i * 41) % 100}%`,
  top: `${30 + ((i * 29) % 70)}%`,
  animationDelay: `${-((i * 0.9) % 14)}s`,
  animationDuration: `${10 + ((i * 7) % 8)}s`,
}));

/** How many motes float, by feast. */
const COUNT: Record<Season | "none", number> = { none: 22, halloween: 45, christmas: 90, easter: 55 };

/** Now and then (one chance in this many each minute), a speck shines a little brighter, for a
 * few seconds (`SPECK_MS`): a click catches it. */
const SPECK_ONE_IN = 8;
const SPECK_MS = 9000;

export function Ambience() {
  const season = useSeason();
  const [speck, setSpeck] = useState<{ left: string; top: string } | null>(null);
  useEffect(() => {
    const timer = setInterval(() => {
      if (Math.random() * SPECK_ONE_IN >= 1) return;
      setSpeck({ left: `${10 + Math.random() * 80}%`, top: `${25 + Math.random() * 60}%` });
      setTimeout(() => setSpeck(null), SPECK_MS);
    }, 60_000);
    return () => clearInterval(timer);
  }, []);
  return (
    <>
      <div className={`game-vignette${season ? ` season-${season}` : ""}`} aria-hidden="true" />
      <div className={`motes${season ? ` season-${season}` : ""}`} aria-hidden="true">
        {MOTES.slice(0, COUNT[season ?? "none"]).map((style, i) => (
          <i key={i} style={style} />
        ))}
      </div>
      {speck && (
        <button
          className="mote-lucky"
          style={speck}
          aria-hidden="true"
          tabIndex={-1}
          onClick={() => {
            setSpeck(null);
            sounds.shimmer();
            findEgg("speck");
          }}
        />
      )}
    </>
  );
}
