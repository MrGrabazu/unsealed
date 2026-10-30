import { useEffect, useState, type CSSProperties } from "react";
import { getSeasonalOn } from "./settings";

/** The feasts the game dresses up for (mockup .cache/da/v11.html): the dust, the vignette, the
 * wax and the cartouches change, ornaments sit on the strap, the rail and the frames (seasons
 * section of comptoir.css). Nothing ever covers a card, a price or a button. */
export type Season = "halloween" | "christmas" | "easter";

/** Easter Sunday of a year (the anonymous Gregorian computus), as [month (1-12), day]. */
export function easterSunday(year: number): [number, number] {
  const a = year % 19;
  const b = Math.floor(year / 100);
  const c = year % 100;
  const d = Math.floor(b / 4);
  const e = b % 4;
  const f = Math.floor((b + 8) / 25);
  const g = Math.floor((b - f + 1) / 3);
  const h = (19 * a + b - d - g + 15) % 30;
  const i = Math.floor(c / 4);
  const k = c % 4;
  const l = (32 + 2 * e + 2 * i - h - k) % 7;
  const m = Math.floor((a + 11 * h + 22 * l) / 451);
  const n = h + l - 7 * m + 114;
  return [Math.floor(n / 31), (n % 31) + 1];
}

/** The feast of a day, on the player's clock: Halloween from October 30 to November 1,
 * Christmas from December 24 to 26, Easter from Good Friday to Easter Monday. */
export function seasonOf(date: Date): Season | null {
  const md = (date.getMonth() + 1) * 100 + date.getDate();
  if (md >= 1030 && md <= 1101) return "halloween";
  if (md >= 1224 && md <= 1226) return "christmas";
  const [month, day] = easterSunday(date.getFullYear());
  const easter = new Date(date.getFullYear(), month - 1, day);
  const days = Math.round((new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime() - easter.getTime()) / 86_400_000);
  return days >= -2 && days <= 1 ? "easter" : null;
}

/** In development only, a feast forced to look at it any day: `?season=halloween` in the
 * address (the preview), or `unsealed.season` in the storage (the app's devtools). */
function devSeason(): Season | null {
  if (!import.meta.env.DEV) return null;
  const asked = new URLSearchParams(location.search).get("season") ?? localStorage.getItem("unsealed.season");
  return asked === "halloween" || asked === "christmas" || asked === "easter" ? asked : null;
}

/** Today's feast, if the decorations are on (looked at again every ten minutes: the game may
 * stay open past midnight). */
export function useSeason(): Season | null {
  const read = () => (getSeasonalOn() ? (devSeason() ?? seasonOf(new Date())) : null);
  const [season, setSeason] = useState(read);
  useEffect(() => {
    const timer = setInterval(() => setSeason(read()), 10 * 60_000);
    return () => clearInterval(timer);
  }, []);
  return season;
}

const GREETED = "unsealed.seasonGreeted";

/** Whether today's feast was not greeted yet (once a day); remembers it was. */
export function greetToday(): boolean {
  const day = new Date().toDateString();
  try {
    if (localStorage.getItem(GREETED) === day) return false;
    localStorage.setItem(GREETED, day);
  } catch {
    // Without storage, greeted each time the game is shown.
  }
  return true;
}

const BAT = "M23 8q-4-6-10-6q2 4 0 6q-6-4-13 0q6 2 8 8q4-4 9-2q2 6 6 8q4-2 6-8q5-2 9 2q2-6 8-8q-7-4-13 0q-2-2 0-6q-6 0-10 6z";
const WAVE = "M0 6Q50 22 100 6T200 6T300 6T400 6T500 6T600 6T700 6T800 6T900 6T1000 6";
const LIGHTS = ["#ff4a4a", "#ffd34a", "#4ad1ff", "#7dff6a", "#ff7ad9"];
const PASTELS = ["#ffc6dc", "#fff3c4", "#cfe4ff", "#d8f0c0"];

/** The ornaments over the game screen: a garland under the strap, and what crosses or hangs
 * over the stage (behind the panels, or passing over them without taking the mouse). */
export function SeasonalDecor({ season }: { season: Season }) {
  return (
    <div className={`season season-${season}`} aria-hidden="true">
      {season === "halloween" && (
        <>
          <svg className="season-moon" width="120" height="120" viewBox="0 0 120 120">
            <defs>
              <radialGradient id="season-moon-glow">
                <stop offset="0" stopColor="#fff2c8" stopOpacity=".5" />
                <stop offset="1" stopColor="#fff2c8" stopOpacity="0" />
              </radialGradient>
            </defs>
            <circle cx="60" cy="60" r="58" fill="url(#season-moon-glow)" />
            <circle cx="60" cy="60" r="30" fill="#f3e3b0" />
            <g fill="#d8c48a" opacity=".7">
              <circle cx="50" cy="52" r="6" />
              <circle cx="70" cy="70" r="4" />
              <circle cx="66" cy="48" r="3" />
            </g>
          </svg>
          <svg className="season-garland" viewBox="0 0 1000 40" preserveAspectRatio="none">
            <path d={WAVE} stroke="#2a1a10" strokeWidth="1.5" fill="none" />
            {Array.from({ length: 10 }, (_, i) => (
              <path key={i} transform={`translate(${38 + i * 100} 10) scale(.6)`} d={BAT} fill={i % 2 ? "#ff8a2a" : "#1a0d14"} />
            ))}
          </svg>
          <div className="season-fog">
            <i style={{ left: "0%", "--fog": "#6a2a8a44" } as CSSProperties} />
            <i style={{ left: "35%", "--fog": "#2a8a4a2a", animationDelay: "-6s" } as CSSProperties} />
            <i style={{ left: "65%", "--fog": "#6a2a8a38", animationDelay: "-11s" } as CSSProperties} />
          </div>
          {[
            [12, 0, 9, 46],
            [22, -3, 12, 34],
            [8, -6, 10, 28],
            [30, -8, 14, 40],
            [17, -2, 16, 24],
          ].map(([top, delay, duration, width], i) => (
            <svg
              key={i}
              className="season-bat"
              style={{ top: `${top}%`, animationDelay: `${delay}s`, animationDuration: `${duration}s` }}
              width={width}
              height={width / 2}
              viewBox="0 0 46 22"
            >
              <path d={BAT} fill="#120a10" />
            </svg>
          ))}
          <svg className="season-ghost" width="54" height="64" viewBox="0 0 54 64">
            <path d="M4 60q0-56 23-56t23 56l-6-6-6 6-5-6-6 6-5-6-6 6-6-6z" fill="#f4f0ffcc" />
            <ellipse cx="19" cy="26" rx="3.5" ry="5" fill="#1a0d14" />
            <ellipse cx="35" cy="26" rx="3.5" ry="5" fill="#1a0d14" />
            <ellipse cx="27" cy="38" rx="4" ry="5" fill="#1a0d14" />
          </svg>
        </>
      )}
      {season === "christmas" && (
        <>
          <div className="season-garland season-swag">
            <svg viewBox="0 0 1000 34" preserveAspectRatio="none">
              <path d={WAVE} stroke="#1f4a28" strokeWidth="9" fill="none" strokeLinecap="round" />
              <path d={WAVE} stroke="#3d8a4a" strokeWidth="4" fill="none" strokeDasharray="2 5" />
            </svg>
            {Array.from({ length: 36 }, (_, i) => {
              const x = (i + 0.5) / 36;
              return (
                <b
                  key={i}
                  style={{
                    left: `${x * 100}%`,
                    top: `${2 + Math.abs(Math.sin(x * Math.PI * 10)) * 10}px`,
                    background: LIGHTS[i % LIGHTS.length],
                    color: LIGHTS[i % LIGHTS.length],
                    animationDelay: `${(i % 5) * 0.35}s`,
                  }}
                />
              );
            })}
          </div>
          {[
            [14, 12, "#b0202a"],
            [29, 20, "#d9a441"],
            [57, 14, "#2e6b9a"],
            [75, 18, "#b0202a"],
          ].map(([left, length, color], i) => (
            <svg
              key={i}
              className="season-bauble"
              style={{ left: `${left}%`, animationDelay: `${-i * 0.9}s` }}
              width="26"
              height={(length as number) + 26}
              viewBox={`0 0 26 ${(length as number) + 26}`}
            >
              <path d={`M13 0V${length}`} stroke="#d9a44199" strokeWidth="1" />
              <rect x="9" y={(length as number) - 2} width="8" height="5" rx="1" fill="#d9a441" />
              <circle cx="13" cy={(length as number) + 13} r="11" fill={color as string} />
              <ellipse cx="9" cy={(length as number) + 9} rx="3" ry="4" fill="#ffffff55" />
            </svg>
          ))}
        </>
      )}
      {season === "easter" && (
        <>
          <div className="season-rays" />
          <svg className="season-garland" viewBox="0 0 1000 30" preserveAspectRatio="none">
            <path d={WAVE} stroke="#6b8a3a" strokeWidth="2" fill="none" />
            {Array.from({ length: 22 }, (_, i) => {
              const x = 24 + i * 45;
              const y = 8 + Math.abs(Math.sin(i * 1.3)) * 8;
              return (
                <g key={i}>
                  <circle cx={x} cy={y} r="5" fill={PASTELS[i % 4]} />
                  <circle cx={x} cy={y} r="1.6" fill="#f5d68a" />
                </g>
              );
            })}
          </svg>
          {[
            ["#f7a8c8", 0, 13],
            ["#a6d8f0", -5, 16],
            ["#fff3a0", -9, 11],
          ].map(([color, delay, duration], i) => (
            <svg
              key={i}
              className="season-butterfly"
              style={{ animationDelay: `${delay}s`, animationDuration: `${duration}s` }}
              width="30"
              height="24"
              viewBox="0 0 30 24"
            >
              <g className="wing">
                <path d="M15 12q-6-12-13-9q-2 8 13 9q-12 2-9 9q6 2 9-9z" fill={color as string} />
                <path d="M15 12q6-12 13-9q2 8-13 9q12 2 9 9q-6 2-9-9z" fill={color as string} />
              </g>
              <path d="M15 6v12" stroke="#3a2a1a" strokeWidth="1.6" />
            </svg>
          ))}
        </>
      )}
    </div>
  );
}

/** At the foot of the rail, above its last bookmark: candles, gifts and a little fir, a nest of
 * painted eggs and a bunny peeking out. */
export function SeasonalRailFoot({ season }: { season: Season }) {
  return (
    <div className={`season-rail season-${season}`} aria-hidden="true">
      {season === "halloween" && (
        <svg width="66" height="90" viewBox="0 0 48 90">
          <ellipse className="season-glow" cx="24" cy="34" rx="24" ry="26" fill="#ff9a3c33" />
          <path d="M16 20q5 7 0 12q-5-5 0-12z" fill="#ffd27a" />
          <rect x="10" y="34" width="12" height="52" rx="2" fill="#efe3c8" />
          <path d="M33 34q4 6 0 10q-4-4 0-10z" fill="#ffd27a" />
          <rect x="28" y="46" width="10" height="40" rx="2" fill="#e2d4b4" />
          <path d="M10 40q3 6 0 12M22 44q-3 5 0 10" stroke="#d8c8a8" strokeWidth="2" fill="none" />
        </svg>
      )}
      {season === "christmas" && (
        <svg width="66" height="90" viewBox="0 0 52 70">
          <rect x="4" y="40" width="28" height="28" rx="2" fill="#b0202a" />
          <rect x="16" y="40" width="4" height="28" fill="#f5d68a" />
          <rect x="4" y="52" width="28" height="4" fill="#f5d68a" />
          <rect x="26" y="48" width="22" height="20" rx="2" fill="#2e6b3a" />
          <rect x="35" y="48" width="4" height="20" fill="#f5d68a" />
          <rect x="12" y="20" width="22" height="20" rx="2" fill="#d9a441" />
          <rect x="21" y="20" width="4" height="20" fill="#b0202a" />
          <path d="M23 20q-8-10-2-10q4 0 2 10q6-10 10-8q2 4-10 8" fill="#b0202a" />
        </svg>
      )}
      {season === "easter" && (
        <svg width="66" height="90" viewBox="0 0 60 80">
          <g className="season-bunny">
            <ellipse cx="22" cy="18" rx="5" ry="16" fill="#f3ece2" />
            <ellipse cx="22" cy="18" rx="2.5" ry="11" fill="#f7c9de" />
            <ellipse cx="38" cy="18" rx="5" ry="16" fill="#f3ece2" />
            <ellipse cx="38" cy="18" rx="2.5" ry="11" fill="#f7c9de" />
            <ellipse cx="30" cy="44" rx="16" ry="13" fill="#f3ece2" />
            <circle cx="24" cy="41" r="2" fill="#2a1a10" />
            <circle cx="36" cy="41" r="2" fill="#2a1a10" />
            <ellipse cx="30" cy="47" rx="2.6" ry="1.8" fill="#f7a8c8" />
          </g>
          <ellipse cx="30" cy="70" rx="28" ry="9" fill="#8a6a3a" />
          <path d="M4 68q26 10 52 0M6 72q24 8 48 0" stroke="#b8925a" strokeWidth="1.5" fill="none" />
          <ellipse cx="18" cy="60" rx="8" ry="11" fill="#a6d8f0" />
          <path d="M10 58q8-5 16 0" stroke="#fff" strokeWidth="2" fill="none" />
          <ellipse cx="42" cy="60" rx="8" ry="11" fill="#f7c9de" />
          <path d="M34 62q8 5 16 0" stroke="#d98bb0" strokeWidth="2" fill="none" />
          <ellipse cx="30" cy="64" rx="7" ry="9" fill="#fff3c4" />
          <g fill="#9ccb6a">
            <circle cx="27" cy="63" r="1.5" />
            <circle cx="33" cy="65" r="1.5" />
          </g>
        </svg>
      )}
    </div>
  );
}
