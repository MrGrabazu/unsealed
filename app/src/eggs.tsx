import { useEffect, useRef, useState } from "react";
import { foundEasterEgg } from "./api";
import { useI18n } from "./i18n";
import { viaCache } from "./images";
import { sounds } from "./sound";

// The game's little secrets, for the curious (each one a secret achievement, game/eggs.rs).
// Never described where a player reads: no wiki, changelog nor commit message.

/** Sent on `window` once a secret is found: the game screen shows its achievement, and the card
 * it brings (`detail.card`). */
export const EGG_FOUND = "unsealed-egg-found";

/** Reports a secret found; never fails (a secret must not get in the way). */
export function findEgg(egg: string) {
  foundEasterEgg(egg)
    .then((found) => window.dispatchEvent(new CustomEvent(EGG_FOUND, { detail: { egg, card: found.card } })))
    .catch(() => {});
}

/** ↑ ↑ ↓ ↓ ← → ← → B A, typed anywhere but in a text field. */
const KONAMI = ["ArrowUp", "ArrowUp", "ArrowDown", "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowLeft", "ArrowRight", "b", "a"];

/** Listens for the oldest code of all. */
export function useKonamiCode() {
  useEffect(() => {
    let at = 0;
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return;
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      at = key === KONAMI[at] ? at + 1 : key === KONAMI[0] ? 1 : 0;
      if (at === KONAMI.length) {
        at = 0;
        sounds.arcade();
        findEgg("konami");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

/** Magic's birthday, on the player's clock: Alpha came out on August 5, 1993. */
export const isMagicBirthday = () => {
  const now = new Date();
  return now.getMonth() === 7 && now.getDate() === 5;
};

/** A booster opened between 3 and 4 in the morning. */
export function nightOwl() {
  if (new Date().getHours() === 3) findEgg("insomniac");
}

/** On Magic's birthday, a ribbon and confetti over each opening. */
export function BirthdayRibbon() {
  const { t } = useI18n();
  const [shown, setShown] = useState(true);
  useEffect(() => {
    const timer = setTimeout(() => setShown(false), 7000);
    return () => clearTimeout(timer);
  }, []);
  if (!shown) return null;
  return (
    <div className="birthday" aria-live="polite">
      <div className="birthday-ribbon">🎂 {t("egg.birthday")}</div>
      <div className="confetti" aria-hidden="true">
        {Array.from({ length: 36 }, (_, i) => (
          <i key={i} style={{ left: `${(i * 37) % 100}%`, animationDelay: `${(i % 9) * 0.18}s`, ["--hue" as string]: (i * 47) % 360 }} />
        ))}
      </div>
    </div>
  );
}

/** Alpha's Black Lotus (the card whole, as Scryfall asks: artist and copyright in view). */
const BLACK_LOTUS = viaCache("https://cards.scryfall.io/normal/front/b/0/b0faa7f2-b547-42c4-a810-839da50dadfe.jpg");

/** Whether a search names the lotus. */
export const isLotus = (search: string) => /^(black\s+)?lotus$/i.test(search.trim());

/** A Black Lotus drifting across the screen, once per search that names it. */
export function LotusPassing({ onGone }: { onGone: () => void }) {
  const found = useRef(false);
  useEffect(() => {
    if (!found.current) {
      found.current = true;
      sounds.shimmer();
      findEgg("lotus");
    }
  }, []);
  return <img className="lotus-passing alpha-corners" src={BLACK_LOTUS} alt="" aria-hidden="true" draggable={false} onAnimationEnd={onGone} />;
}

/** Counts quick clicks (each within `gap` ms of the last): true once `count` are reached. */
export function useQuickClicks(count: number, gap = 1500) {
  const clicks = useRef<number[]>([]);
  return () => {
    const now = Date.now();
    const last = clicks.current[clicks.current.length - 1];
    clicks.current = last !== undefined && now - last < gap ? [...clicks.current, now] : [now];
    if (clicks.current.length >= count) {
      clicks.current = [];
      return true;
    }
    return false;
  };
}
