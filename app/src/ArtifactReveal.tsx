import { useEffect } from "react";
import { ArtifactGlyph, artifactMetal } from "./ArtifactGlyph";
import { Backdrop } from "./Backdrop";
import type { ArtifactView } from "./api";
import { useI18n } from "./i18n";
import { useAmountsInText } from "./money";
import { sounds } from "./sound";

/** The moment an artifact is got (found in a booster, given by an achievement): it rises
 * under its glass bell, in the light, with its name and effect. Closed by a click. */
export function ArtifactReveal({ item, onClose }: { item: ArtifactView; onClose: () => void }) {
  const { t } = useI18n();
  const amountsInText = useAmountsInText();
  const metal = artifactMetal(item.glyph, item.rarity);

  useEffect(() => {
    sounds.fanfare(item.rarity === "mythic");
    const onKey = (e: KeyboardEvent) => (e.key === "Escape" || e.key === "Enter") && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [item.id]);

  return (
    <Backdrop onClose={onClose}>
      <div className="frame artifact-reveal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()} style={{ ["--metal" as string]: metal }}>
        <div className="cartouche">{t(item.source === "booster" ? "curios.foundTitle" : "curios.givenTitle")}</div>
        <div className="showcase reveal">
          <span className="showcase-light" />
          <span className="reveal-rays" />
          <span className="plinth" />
          <span className="bell" />
          <ArtifactGlyph glyph={item.glyph} color={metal} />
        </div>
        <h2>{item.name}</h2>
        <div className="meta">
          <b style={{ color: metal }}>{t(`curios.rarity.${item.rarity}`)}</b> · n° {String(item.number).padStart(2, "0")}
        </div>
        <div className="curio-effect">{amountsInText(item.effect ?? "")}</div>
        {item.flavor && <p className="curio-flavor">« {item.flavor} »</p>}
        <button className="big" onClick={onClose} autoFocus>
          {t("curios.toCabinet")}
        </button>
      </div>
    </Backdrop>
  );
}
