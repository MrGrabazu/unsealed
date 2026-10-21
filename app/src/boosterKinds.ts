import type { RewardView } from "./api";
import type { I18n } from "./i18n";

/** Display order of booster kinds; unknown kinds come last, alphabetically. */
const KIND_ORDER = ["play", "draft", "set", "default", "collector"];

export const kindRank = (kind: string) => {
  const i = KIND_ORDER.indexOf(kind);
  return i === -1 ? KIND_ORDER.length : i;
};

/** How a booster's wrapper looks, by kind, so the kinds of a set tell apart at a glance and
 * rise with their price: matte paper (draft), foil (play, and the oldest sets' single kind),
 * brighter foil with silver (set), black lacquer with gold and a holographic rule (collector). */
export type WrapperFinish = "matte" | "foil" | "silver" | "collector";

export function wrapperFinish(kind: string): WrapperFinish {
  switch (kind) {
    case "draft":
      return "matte";
    case "set":
      return "silver";
    case "collector":
      return "collector";
    default:
      return "foil";
  }
}

/** "Draft booster", or just "Booster" for the oldest sets, which had a single kind ("default"). */
export function kindLabel(t: I18n["t"], kind: string): string {
  switch (kind) {
    case "play":
      return t("kind.play");
    case "draft":
      return t("kind.draft");
    case "set":
      return t("kind.set");
    case "collector":
      return t("kind.collector");
    case "default":
      return t("kind.default");
    default:
      return t("kind.other", { kind });
  }
}

/** An achievement reward in words, such as "+$5.00 + 2 × Bloomburrow Play booster". */
export function rewardText(i18n: I18n, money: (cents: number) => string, reward: RewardView): string {
  const parts: string[] = [];
  if (reward.cashCents > 0) parts.push(i18n.t("reward.cash", { amount: money(reward.cashCents) }));
  if (reward.sealed) {
    parts.push(
      i18n.plural("reward.boosters", reward.sealed.quantity, {
        set: reward.sealed.setName ?? i18n.t("reward.newestSet"),
        kind: kindLabel(i18n.t, reward.sealed.kind),
      }),
    );
  }
  return parts.join(i18n.t("reward.join"));
}
