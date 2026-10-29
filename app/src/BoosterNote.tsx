import { isCraftedKind, isJapaneseKind } from "./boosterKinds";
import { useI18n } from "./i18n";
import type { MessageKey } from "./i18n/en";

/** What each set's Japanese booster holds that its English one does not. */
const NOTES: Record<string, MessageKey> = {
  WAR: "jpNote.WAR",
  IKO: "jpNote.IKO",
  STX: "jpNote.STX",
};

/** A word on the boosters that are not quite like the others, in the Boosterium and the
 * reserve: why a Japanese booster is offered (the game sells no booster per language) and what
 * it may give, with how to tell it; that a crafted booster (Secret Lair) does not exist, its
 * odds being the game's. */
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
  if (!isJapaneseKind(kind)) return null;
  const note = NOTES[setCode];
  return (
    <div className="jp-note">
      <strong>{t("jpNote.title")}</strong>
      <p>{t("jpNote.why")}</p>
      {note && <p>{t(note)}</p>}
    </div>
  );
}
