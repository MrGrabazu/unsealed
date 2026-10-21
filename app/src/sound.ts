// The game's sounds, synthesized with the Web Audio API: nothing to download or license,
// and each play varies a little, like the real thing. Turned on or off, and their volume
// set, in the options (kept in the browser storage, like the other app settings).

import type { OpenedCard } from "./api";
import { isBigCard } from "./BatchOpening";

const ENABLED = "unsealed.sound";
const VOLUME = "unsealed.volume";

export const getSoundOn = () => localStorage.getItem(ENABLED) !== "off";
export const setSoundOn = (on: boolean) => localStorage.setItem(ENABLED, on ? "on" : "off");
/** Volume, 0 to 1. */
export const getVolume = () => {
  const v = Number(localStorage.getItem(VOLUME));
  return Number.isFinite(v) && localStorage.getItem(VOLUME) !== null ? Math.min(1, Math.max(0, v)) : 0.6;
};
export const setVolume = (v: number) => localStorage.setItem(VOLUME, String(v));

let audio: AudioContext | null = null;
let master: GainNode | null = null;

/** The audio context shared by the sounds and the music, created on first use (browsers allow
 * it after a user gesture); null where Web Audio is missing. */
export function sharedAudio(): AudioContext | null {
  try {
    audio ??= new AudioContext();
    if (audio.state === "suspended") void audio.resume();
    return audio;
  } catch {
    return null;
  }
}

/** The audio context and the sounds' output, when the sounds are on. */
function context(): { ctx: AudioContext; out: GainNode } | null {
  if (!getSoundOn()) return null;
  try {
    audio = sharedAudio();
    if (!audio) return null;
    if (!master) {
      master = audio.createGain();
      master.connect(audio.destination);
    }
    master.gain.value = getVolume() * 0.8;
    if (audio.state === "suspended") void audio.resume();
    return { ctx: audio, out: master };
  } catch {
    return null;
  }
}

const jitter = (amount: number) => 1 + (Math.random() - 0.5) * amount;

/** A burst of white noise through a filter, shaped by an envelope. */
function noise(
  ctx: AudioContext,
  out: AudioNode,
  { at = 0, duration, type, frequency, q = 1, attack = 0.005, gain = 0.5, sweepTo }: {
    at?: number; duration: number; type: BiquadFilterType; frequency: number; q?: number; attack?: number; gain?: number; sweepTo?: number;
  },
) {
  const start = ctx.currentTime + at;
  const buffer = ctx.createBuffer(1, Math.ceil(ctx.sampleRate * duration), ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1;
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  const filter = ctx.createBiquadFilter();
  filter.type = type;
  filter.frequency.setValueAtTime(frequency, start);
  if (sweepTo) filter.frequency.exponentialRampToValueAtTime(sweepTo, start + duration);
  filter.Q.value = q;
  const env = ctx.createGain();
  env.gain.setValueAtTime(0, start);
  env.gain.linearRampToValueAtTime(gain, start + attack);
  env.gain.exponentialRampToValueAtTime(0.001, start + duration);
  source.connect(filter).connect(env).connect(out);
  source.start(start);
  source.stop(start + duration);
}

/** A tone with a quick attack and a ringing decay. */
function tone(
  ctx: AudioContext,
  out: AudioNode,
  { at = 0, frequency, duration, type = "sine", gain = 0.3, glideTo }: {
    at?: number; frequency: number; duration: number; type?: OscillatorType; gain?: number; glideTo?: number;
  },
) {
  const start = ctx.currentTime + at;
  const osc = ctx.createOscillator();
  osc.type = type;
  osc.frequency.setValueAtTime(frequency, start);
  if (glideTo) osc.frequency.exponentialRampToValueAtTime(glideTo, start + duration);
  const env = ctx.createGain();
  env.gain.setValueAtTime(0, start);
  env.gain.linearRampToValueAtTime(gain, start + 0.008);
  env.gain.exponentialRampToValueAtTime(0.001, start + duration);
  osc.connect(env).connect(out);
  osc.start(start);
  osc.stop(start + duration);
}

/** The sounds of the game. */
export const sounds = {
  /** A foil wrapper torn open: a ripping hiss made of little crackles. */
  tear() {
    const a = context();
    if (!a) return;
    for (let i = 0; i < 9; i++) {
      noise(a.ctx, a.out, { at: i * 0.028 * jitter(0.6), duration: 0.06, type: "bandpass", frequency: 2600 * jitter(0.5), q: 1.4, gain: 0.35 });
    }
    noise(a.ctx, a.out, { duration: 0.35, type: "highpass", frequency: 1800, sweepTo: 5200, gain: 0.25, attack: 0.02 });
  },

  /** A card slid off the pile. */
  slide() {
    const a = context();
    if (!a) return;
    noise(a.ctx, a.out, { duration: 0.22 * jitter(0.3), type: "bandpass", frequency: 1400 * jitter(0.3), sweepTo: 3600, q: 0.8, gain: 0.22, attack: 0.03 });
  },

  /** A card turned face up. */
  flip() {
    const a = context();
    if (!a) return;
    noise(a.ctx, a.out, { duration: 0.09, type: "bandpass", frequency: 3200 * jitter(0.3), q: 2, gain: 0.25 });
  },

  /** A foil revealed: a shimmering chime. */
  shimmer() {
    const a = context();
    if (!a) return;
    [1318.5, 1760, 2093, 2637].forEach((f, i) =>
      tone(a.ctx, a.out, { at: i * 0.06, frequency: f * jitter(0.01), duration: 0.9, gain: 0.07 }),
    );
  },

  /** A great pull (rare, mythic): a deep bell and a rising chord. */
  fanfare(mythic: boolean) {
    const a = context();
    if (!a) return;
    tone(a.ctx, a.out, { frequency: mythic ? 98 : 130.8, duration: 1.6, type: "triangle", gain: 0.25 });
    const chord = mythic ? [392, 493.9, 587.3, 784] : [523.3, 659.3, 784];
    chord.forEach((f, i) => tone(a.ctx, a.out, { at: 0.08 + i * 0.09, frequency: f, duration: 1.4, type: "triangle", gain: 0.09 }));
  },

  /** Coins: a purchase, a sale. */
  coins() {
    const a = context();
    if (!a) return;
    for (let i = 0; i < 4; i++) {
      tone(a.ctx, a.out, { at: i * 0.07 * jitter(0.4), frequency: 2800 * jitter(0.15), duration: 0.25, type: "square", gain: 0.03 });
      tone(a.ctx, a.out, { at: i * 0.07 * jitter(0.4), frequency: 4200 * jitter(0.15), duration: 0.18, gain: 0.05 });
    }
  },

  /** A wax seal cracking: an achievement unlocked. */
  seal() {
    const a = context();
    if (!a) return;
    tone(a.ctx, a.out, { frequency: 140, glideTo: 60, duration: 0.25, type: "sine", gain: 0.5 });
    noise(a.ctx, a.out, { at: 0.01, duration: 0.12, type: "bandpass", frequency: 900, q: 1, gain: 0.4 });
    [659.3, 830.6, 987.8, 1318.5].forEach((f, i) =>
      tone(a.ctx, a.out, { at: 0.15 + i * 0.08, frequency: f, duration: 0.9, type: "triangle", gain: 0.08 }),
    );
  },

  /** A page turned in a binder. */
  page() {
    const a = context();
    if (!a) return;
    noise(a.ctx, a.out, { duration: 0.28, type: "bandpass", frequency: 900, sweepTo: 2400, q: 0.7, gain: 0.18, attack: 0.06 });
  },

  /** A soft click (opening a details view...). */
  click() {
    const a = context();
    if (!a) return;
    tone(a.ctx, a.out, { frequency: 1800, glideTo: 900, duration: 0.05, type: "triangle", gain: 0.08 });
  },
};

/** The card of a booster that deserves its sound: a mythic, else a great pull, else a foil,
 * else the most valuable. (Cards come in a real booster's order, not the best last.) */
export function bestCard(cards: OpenedCard[]): OpenedCard | undefined {
  const score = (c: OpenedCard) =>
    (c.rarity === "mythic" ? 4e9 : 0) + (isBigCard(c) ? 2e9 : 0) + (c.foil ? 1e9 : 0) + (c.priceCents ?? 0);
  return cards.reduce<OpenedCard | undefined>((best, c) => (!best || score(c) > score(best) ? c : best), undefined);
}

/** What a card revealed sounds like: a bell for a great pull, a chime for a foil (both for a
 * great foil), a soft flip otherwise. */
export function revealSound(card: OpenedCard | undefined) {
  if (!card) return;
  const great = card.rarity === "mythic" || isBigCard(card);
  if (great) sounds.fanfare(card.rarity === "mythic");
  if (card.foil) sounds.shimmer();
  if (!great && !card.foil) sounds.flip();
}
