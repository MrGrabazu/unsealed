import { isCraftedKind, isJapaneseKind, usualKind } from "./boosterKinds";
import { useI18n } from "./i18n";
import type { MessageKey } from "./i18n/en";

/** The sets whose Japanese boosters hold cards of their own, the English kinds those cards hide
 * in (`japanese_guest` of game.rs), and what they are. */
const NOTES: Record<string, { kinds: string[]; note: MessageKey }> = {
  WAR: { kinds: ["draft"], note: "jpNote.WAR" },
  IKO: { kinds: ["draft", "collector"], note: "jpNote.IKO" },
  STX: { kinds: ["draft", "set"], note: "jpNote.STX" },
};

/** A word on the boosters that are not quite like the others, in the Boosterium and the
 * reserve: the cards of Japan's booster that may hide in this one, with how to tell them (a
 * Japanese booster, no longer sold, still opens); that a crafted booster (Secret Lair) does not
 * exist, its odds being the game's. */
export function BoosterNote({ setCode, kind }: { setCode: string; kind: string }) {
  const { t } = useI18n();
  if (isCraftedKind(kind)) {
    return (
      <div className="jp-note">
        <strong>{t("craftedNote.title")}</strong>
        <p>{t("craftedNote.why")}</p>
        <p>{t("craftedNote.content")}</p>
      </div>
    );
  }
  const japanese = isJapaneseKind(kind);
  const entry = NOTES[setCode];
  if (!japanese && !entry?.kinds.includes(usualKind(kind))) return null;
  return (
    <div className="jp-note">
      <strong>{t("jpNote.title")}</strong>
      <p>{t(japanese ? "jpNote.sealed" : "jpNote.why")}</p>
      {entry && <p>{t(entry.note)}</p>}
    </div>
  );
}
