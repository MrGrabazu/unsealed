// The game's music: an ambience generated as it plays with the Web Audio API (nothing to
// download or license, never twice the same). A back shop at night: a low drone that breathes,
// slow minor chords, a few bells echoing in the dark, the hush of the room and a candle's
// crackle now and then, all in a long reverb. On by default, its own volume in the options;
// the header's sound button silences it with the rest.

import { getSoundOn, sharedAudio } from "./sound";

const ENABLED = "unsealed.music";
const VOLUME = "unsealed.musicVolume";

export const getMusicOn = () => localStorage.getItem(ENABLED) !== "off";
export const setMusicOn = (on: boolean) => {
  localStorage.setItem(ENABLED, on ? "on" : "off");
  refreshMusic();
};
/** Volume, 0 to 1. */
export const getMusicVolume = () => {
  const v = Number(localStorage.getItem(VOLUME));
  return Number.isFinite(v) && localStorage.getItem(VOLUME) !== null ? Math.min(1, Math.max(0, v)) : 0.4;
};
export const setMusicVolume = (v: number) => {
  localStorage.setItem(VOLUME, String(v));
  refreshMusic();
};

/** D minor (aeolian), in Hz from its MIDI number. */
const hz = (midi: number) => 440 * 2 ** ((midi - 69) / 12);
/** Chords of the progression (MIDI notes), each held for a while: i – VI – III – VII, and iv. */
const CHORDS = [
  [50, 57, 62, 65], // Dm
  [46, 53, 58, 62], // Bb
  [53, 57, 60, 65], // F
  [48, 55, 60, 64], // C
  [50, 57, 62, 65], // Dm
  [43, 50, 55, 58], // Gm
  [45, 52, 57, 61], // A (the dominant, a touch of mystery)
];
/** The bells' notes: D minor pentatonic, high up. */
const BELLS = [74, 77, 79, 81, 84, 86, 89];

const random = (from: number, to: number) => from + Math.random() * (to - from);

interface Playing {
  ctx: AudioContext;
  out: GainNode;
  wet: GainNode;
  timers: number[];
  stops: (() => void)[];
}

let playing: Playing | null = null;
/** Whether a user gesture has happened yet (before one, browsers keep the audio suspended). */
let unlocked = false;

/** A long, dark reverb made from decaying noise. */
function reverb(ctx: AudioContext, seconds: number): ConvolverNode {
  const length = Math.ceil(ctx.sampleRate * seconds);
  const impulse = ctx.createBuffer(2, length, ctx.sampleRate);
  for (let channel = 0; channel < 2; channel++) {
    const data = impulse.getChannelData(channel);
    for (let i = 0; i < length; i++) data[i] = (Math.random() * 2 - 1) * (1 - i / length) ** 3;
  }
  const node = ctx.createConvolver();
  node.buffer = impulse;
  return node;
}

/** The low drone: two detuned voices on D and A, through a filter that slowly opens and closes. */
function drone(p: Playing) {
  const { ctx } = p;
  const filter = ctx.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = 380;
  filter.Q.value = 3;
  const lfo = ctx.createOscillator();
  lfo.frequency.value = 0.045;
  const depth = ctx.createGain();
  depth.gain.value = 160;
  lfo.connect(depth).connect(filter.frequency);
  const gain = ctx.createGain();
  gain.gain.value = 0.11;
  filter.connect(gain);
  gain.connect(p.out);
  gain.connect(p.wet);
  const voices = [hz(38), hz(38) * 1.004, hz(45), hz(26)].map((f, i) => {
    const osc = ctx.createOscillator();
    osc.type = i === 3 ? "sine" : "sawtooth";
    osc.frequency.value = f;
    const level = ctx.createGain();
    level.gain.value = i === 3 ? 0.9 : 0.28;
    osc.connect(level).connect(filter);
    osc.start();
    return osc;
  });
  lfo.start();
  p.stops.push(() => [lfo, ...voices].forEach((o) => o.stop()));
}

/** One chord of the progression: soft voices that swell in, hold, and fade into the reverb. */
function chord(p: Playing, notes: number[], seconds: number) {
  const { ctx } = p;
  const start = ctx.currentTime + 0.1;
  const filter = ctx.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = random(900, 1500);
  const env = ctx.createGain();
  env.gain.setValueAtTime(0, start);
  env.gain.linearRampToValueAtTime(0.06, start + seconds * 0.35);
  env.gain.setValueAtTime(0.06, start + seconds * 0.7);
  env.gain.linearRampToValueAtTime(0, start + seconds * 1.25);
  filter.connect(env);
  env.connect(p.out);
  env.connect(p.wet);
  for (const note of notes) {
    for (const detune of [-6, 6]) {
      const osc = ctx.createOscillator();
      osc.type = "triangle";
      osc.frequency.value = hz(note + 12);
      osc.detune.value = detune + random(-3, 3);
      osc.connect(filter);
      osc.start(start);
      osc.stop(start + seconds * 1.3);
    }
  }
}

/** A bell in the distance, with its echo. */
function bell(p: Playing) {
  const { ctx } = p;
  const start = ctx.currentTime + 0.05;
  const note = BELLS[Math.floor(Math.random() * BELLS.length)];
  const env = ctx.createGain();
  env.gain.setValueAtTime(0, start);
  env.gain.linearRampToValueAtTime(random(0.025, 0.045), start + 0.01);
  env.gain.exponentialRampToValueAtTime(0.0005, start + 4.5);
  const echo = ctx.createDelay(2);
  echo.delayTime.value = random(0.45, 0.75);
  const feedback = ctx.createGain();
  feedback.gain.value = 0.35;
  echo.connect(feedback).connect(echo);
  env.connect(echo);
  env.connect(p.wet);
  echo.connect(p.wet);
  env.connect(p.out);
  // A bell: its fundamental and a few inharmonic partials.
  [1, 2.76, 5.4].forEach((ratio, i) => {
    const osc = ctx.createOscillator();
    osc.frequency.value = hz(note) * ratio;
    const level = ctx.createGain();
    level.gain.value = [1, 0.35, 0.12][i];
    osc.connect(level).connect(env);
    osc.start(start);
    osc.stop(start + 5);
  });
}

/** The hush of the room: a faint, dark noise; and a candle's crackle now and then. */
function room(p: Playing) {
  const { ctx } = p;
  const buffer = ctx.createBuffer(1, ctx.sampleRate * 4, ctx.sampleRate);
  const data = buffer.getChannelData(0);
  let last = 0;
  for (let i = 0; i < data.length; i++) {
    last = (last + 0.02 * (Math.random() * 2 - 1)) / 1.02;
    data[i] = last * 3.5;
  }
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.loop = true;
  const filter = ctx.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.value = 500;
  const gain = ctx.createGain();
  gain.gain.value = 0.05;
  source.connect(filter).connect(gain).connect(p.out);
  source.start();
  p.stops.push(() => source.stop());
}

function crackle(p: Playing) {
  const { ctx } = p;
  for (let i = 0; i < Math.floor(random(1, 4)); i++) {
    const start = ctx.currentTime + i * random(0.04, 0.12);
    const buffer = ctx.createBuffer(1, Math.ceil(ctx.sampleRate * 0.02), ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let j = 0; j < data.length; j++) data[j] = (Math.random() * 2 - 1) * (1 - j / data.length);
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    const filter = ctx.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.value = random(1500, 3500);
    const gain = ctx.createGain();
    gain.gain.value = random(0.02, 0.05);
    source.connect(filter).connect(gain).connect(p.out);
    source.start(start);
  }
}

/** Calls `play` again and again, `from` to `to` seconds apart, while the music plays. */
function every(p: Playing, from: number, to: number, play: () => void, first = random(from, to)) {
  const next = (delay: number) => {
    p.timers.push(
      window.setTimeout(() => {
        if (playing !== p) return;
        play();
        next(random(from, to));
      }, delay * 1000),
    );
  };
  next(first);
}

function start() {
  const ctx = sharedAudio();
  if (!ctx) return;
  const out = ctx.createGain();
  out.gain.setValueAtTime(0, ctx.currentTime);
  // A slow fade in, like walking into the shop.
  out.gain.linearRampToValueAtTime(getMusicVolume() * 0.9, ctx.currentTime + 6);
  out.connect(ctx.destination);
  const wet = ctx.createGain();
  wet.gain.value = 0.9;
  wet.connect(reverb(ctx, 5)).connect(out);
  const p: Playing = { ctx, out, wet, timers: [], stops: [] };
  playing = p;
  drone(p);
  room(p);
  let index = 0;
  const nextChord = () => {
    const seconds = random(15, 20);
    chord(p, CHORDS[index % CHORDS.length], seconds);
    index += Math.random() < 0.2 ? 2 : 1;
  };
  nextChord();
  every(p, 13, 17, nextChord);
  every(p, 3, 9, () => bell(p), 2);
  every(p, 6, 18, () => crackle(p));
}

function stop() {
  const p = playing;
  if (!p) return;
  playing = null;
  p.timers.forEach((t) => window.clearTimeout(t));
  const now = p.ctx.currentTime;
  p.out.gain.cancelScheduledValues(now);
  p.out.gain.setValueAtTime(p.out.gain.value, now);
  p.out.gain.linearRampToValueAtTime(0, now + 1.5);
  window.setTimeout(() => {
    p.stops.forEach((s) => {
      try {
        s();
      } catch {
        /* already stopped */
      }
    });
    p.out.disconnect();
  }, 1700);
}

/** Starts, stops or re-levels the music after a change of the settings (music, sound, volumes). */
export function refreshMusic() {
  const wanted = unlocked && getSoundOn() && getMusicOn() && getMusicVolume() > 0;
  if (!wanted) return stop();
  if (!playing) return start();
  const now = playing.ctx.currentTime;
  playing.out.gain.cancelScheduledValues(now);
  playing.out.gain.setValueAtTime(playing.out.gain.value, now);
  playing.out.gain.linearRampToValueAtTime(getMusicVolume() * 0.9, now + 0.4);
}

/** Starts the music at the first click or key press (browsers allow sound only after one). */
export function startMusicOnFirstGesture() {
  const go = () => {
    window.removeEventListener("pointerdown", go);
    window.removeEventListener("keydown", go);
    unlocked = true;
    refreshMusic();
  };
  window.addEventListener("pointerdown", go);
  window.addEventListener("keydown", go);
}
