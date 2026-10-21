import { motion } from "framer-motion";
import type { CSSProperties } from "react";
import type { SetSummary } from "./api";
import { kindLabel, wrapperFinish } from "./boosterKinds";
import { useI18n } from "./i18n";
import { SetIcon } from "./SetIcon";

// A sealed booster, drawn for Unsealed: a metallic wrapper with crimped ends and a window
// showing the set's featured art. Real wrappers are not used: they are not in the data
// sources and they carry the Magic logo (AGENTS.md, section 2). The art crop is shown
// unaltered, its artist credited below; the foil sheen only plays on the wrapper.

/** Crimped edge as a CSS polygon: `teeth` zigzags along the top or bottom of an element. */
function crimp(teeth: number, side: "top" | "bottom"): string {
  const points: string[] = [];
  for (let i = 0; i <= teeth * 2; i++) {
    const x = (i * 100) / (teeth * 2);
    const y = i % 2 === 0 ? 0 : 100;
    points.push(`${x}% ${side === "top" ? y : 100 - y}%`);
  }
  return side === "top"
    ? `polygon(${points.join(", ")}, 100% 100%, 0% 100%)`
    : `polygon(0% 0%, 100% 0%, ${points.reverse().join(", ")})`;
}

const TOP_EDGE = crimp(18, "top");
const BOTTOM_EDGE = crimp(18, "bottom");

/** A colour per set, so each wrapper looks different (stable for a set). */
export function hueOf(code: string): number {
  let h = 0;
  for (const c of code) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

interface Props {
  setCode: string;
  set: SetSummary | undefined;
  kind: string;
  /** The top strip has been torn off. */
  torn: boolean;
  onTear: () => void;
  disabled: boolean;
}

export function Pack({ setCode, set, kind, torn, onTear, disabled }: Props) {
  const { t } = useI18n();
  const hue = hueOf(setCode);
  const style = { "--pack-hue": hue } as CSSProperties;
  return (
    <div className="pack-stage">
      <motion.button
        type="button"
        className={`pack finish-${wrapperFinish(kind)}`}
        style={style}
        onClick={() => !torn && !disabled && onTear()}
        disabled={disabled || torn}
        aria-label={t("pack.tear")}
        initial={{ y: 30, opacity: 0, rotate: -2 }}
        animate={torn ? { y: 60, opacity: 0, scale: 0.92 } : { y: 0, opacity: 1, rotate: 0 }}
        transition={{ duration: torn ? 0.55 : 0.5, delay: torn ? 0.35 : 0, ease: "easeOut" }}
        whileHover={torn ? undefined : { rotate: -1.5, scale: 1.02 }}
      >
        <motion.span
          className="pack-strip"
          style={{ clipPath: TOP_EDGE }}
          animate={torn ? { y: -140, x: 60, rotate: 24, opacity: 0 } : { y: 0, x: 0, rotate: 0, opacity: 1 }}
          transition={{ duration: 0.5, ease: "easeIn" }}
        />
        <span className="pack-body">
          <span className="pack-foil" />
          <span className="pack-header">
            <SetIcon url={set?.iconUrl ?? null} name={set?.name ?? setCode} />
            <span className="pack-set">{set?.name ?? setCode}</span>
          </span>
          <span className="pack-window">
            {set?.art ? <img src={set.art.imageUrl} alt="" draggable={false} /> : <span className="pack-window-empty" />}
          </span>
          <span className="pack-kind">{kindLabel(t, kind)}</span>
          <span className="pack-brand">Unsealed</span>
        </span>
        <span className="pack-strip pack-strip-bottom" style={{ clipPath: BOTTOM_EDGE }} />
      </motion.button>
      {!torn && <p className="hint pack-hint">{t("pack.tear")}</p>}
      {set?.art && <p className="shop-credit">{t("art.credit", { card: set.art.cardName, artist: set.art.artist })}</p>}
    </div>
  );
}
