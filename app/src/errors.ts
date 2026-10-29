import type { I18n } from "./i18n";
import type { MessageKey } from "./i18n/en";

/** The errors a player can run into, as the Rust sends them (in English, the system's words),
 * and how to say them in the player's language. Any other error is shown as is, after a
 * translated word. */
const KNOWN: { pattern: RegExp; key: MessageKey }[] = [
  { pattern: /^no sealed booster of that kind/, key: "error.noSealedBooster" },
  { pattern: /^not enough copies of card/, key: "error.notOwned" },
  { pattern: /already undone$/, key: "error.alreadyUndone" },
  { pattern: /the boosters were opened or sold$/, key: "error.boostersGone" },
  { pattern: /^too late to undo/, key: "error.tooLateToUndo" },
  { pattern: /^this transaction cannot be undone/, key: "error.cannotUndo" },
  { pattern: /^a box topper cannot be sold/, key: "error.topperNotSold" },
  { pattern: /has no market value and cannot be sold$/, key: "error.noMarketValue" },
  { pattern: /^not an Unsealed save/, key: "error.notASave" },
  { pattern: /^this save was edited outside the game/, key: "error.tampered" },
];

/** An error in the player's language. `moneyText` writes an amount in words. */
export function errorText(t: I18n["t"], moneyText: (cents: number) => string, error: unknown): string {
  const message = String(error).replace(/^Error:\s*/, "");
  const money = /^not enough money: (\d+) needed, (\d+) available/.exec(message);
  if (money) return t("error.notEnoughMoney", { needed: moneyText(Number(money[1])), available: moneyText(Number(money[2])) });
  const known = KNOWN.find((k) => k.pattern.test(message));
  return known ? t(known.key) : t("error.other", { detail: message });
}
