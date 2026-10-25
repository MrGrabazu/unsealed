import type { RewardView } from "./api";
import type { I18n } from "./i18n";
import type { MessageKey } from "./i18n/en";

/** Display order of booster kinds (a theme booster at "theme-*"); unknown kinds come last,
 * alphabetically. */
const KIND_ORDER = [
  "play",
  "draft",
  "set",
  "default",
  "six",
  "value",
  "theme-*",
  "jumpstart",
  "jumpstart-v2",
  "mirran",
  "phyrexian",
  "premium",
  "collector",
  "collector-special",
];

export const kindRank = (kind: string) => {
  const i = KIND_ORDER.indexOf(kind.startsWith("theme-") ? "theme-*" : kind);
  return i === -1 ? KIND_ORDER.length : i;
};

/** A collector booster, or one of its editions ("collector-special"). */
const isCollectorKind = (kind: string) => kind === "collector" || kind.startsWith("collector-");

/** The theme of a theme booster: a color ("theme-b"), else a guild, a college, a family…
 * ("theme-boros"), whose name stays as is, or a tribe or a mechanic ("theme-vikings"). */
const THEMES = ["w", "u", "b", "r", "g", "party", "vikings", "monsters", "dungeons", "werewolves", "vampires", "ninjas"];

function themeName(t: I18n["t"], theme: string): string {
  if (THEMES.includes(theme)) return t(`theme.${theme}` as MessageKey);
  return theme.charAt(0).toUpperCase() + theme.slice(1);
}

/** How a booster's wrapper looks, by kind, so the kinds of a set tell apart at a glance and
 * rise with their price: matte paper (draft), foil (play, and the oldest sets' single kind),
 * brighter foil with silver (set), black lacquer with gold and a holographic rule (collector). */
export type WrapperFinish = "matte" | "foil" | "silver" | "collector" | "prism";

/** A box topper's pack ("box-topper", "box-topper-foil", "box-topper-zne"…): given with a box
 * of boosters, never sold (see toppers.rs of the game). */
export const isTopperKind = (kind: string) => kind.startsWith("box-topper");

export function wrapperFinish(kind: string): WrapperFinish {
  // A box topper (a pouch of a booster's size, with a card or two): the prism, the collector's
  // opposite, a pearly lacquer all iridescent (mockup .cache/da/v10.html, proposal D).
  if (isTopperKind(kind)) return "prism";
  if (isCollectorKind(kind)) return "collector";
  if (kind.startsWith("theme-")) return "matte";
  switch (kind) {
    case "draft":
    case "six":
      return "matte";
    case "set":
    case "premium":
      return "silver";
    default:
      return "foil";
  }
}

/** "Draft booster", or just "Booster" for the oldest sets, which had a single kind ("default"). */
export function kindLabel(t: I18n["t"], kind: string): string {
  if (isTopperKind(kind)) return t(kind.startsWith("box-topper-foil") ? "kind.topperFoil" : "kind.topper");
  if (kind.startsWith("theme-")) return t("kind.theme", { theme: themeName(t, kind.slice("theme-".length)) });
  switch (kind) {
    case "six":
      return t("kind.six");
    case "value":
      return t("kind.value");
    case "jumpstart":
      return t("kind.jumpstart");
    case "jumpstart-v2":
      return t("kind.jumpstartV2");
    case "mirran":
      return t("kind.mirran");
    case "phyrexian":
      return t("kind.phyrexian");
    case "premium":
      return t("kind.premium");
    case "collector-special":
      return t("kind.collectorSpecial");
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
