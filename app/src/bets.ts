import type { Bet, BetRequest, CardFilter, CardType, Clause, ColorChoice, KeptBet } from "./api";
import type { I18n } from "./i18n";
import type { MessageKey } from "./i18n/en";

// Shared by the bet slip and the results: what a bet says, its odds, the kept bets.

export const RARITIES = ["common", "uncommon", "rare", "mythic", "special"] as const;
export const CARD_TYPES: CardType[] = [
  "land",
  "basicLand",
  "creature",
  "instant",
  "sorcery",
  "artifact",
  "enchantment",
  "planeswalker",
  "battle",
];
export const COLORS: ColorChoice[] = ["white", "blue", "black", "red", "green", "colorless", "multicolor"];

/** Odds paid on a bet that wins with chance `probability` (the stake included): the fair
 * odds, less the house's margin. */
export const oddsOf = (probability: number, marginPercent: number) => (1 - marginPercent / 100) / probability;

/** Odds as shown: "×4.00", "×1,250". */
export function formatOdds(odds: number, formatNumber: (n: number) => string): string {
  return `×${odds >= 100 ? formatNumber(Math.floor(odds)) : odds.toFixed(2)}`;
}

/** A chance as "1 in N" when it is small, else as a percentage. */
export function formatChance(probability: number, t: I18n["t"], formatNumber: (n: number) => string): string {
  return probability < 0.1
    ? t("bet.oneIn", { count: formatNumber(Math.round(1 / probability)) })
    : `${(probability * 100).toFixed(probability < 0.995 ? 1 : 2)} %`;
}

/** Identifies a bet, whatever its stake. */
export const betKey = (bet: Bet) => JSON.stringify(bet);

/** The card a bet names, if any. */
export const namedCard = (bet: Bet) =>
  bet.clauses.find((c): c is Extract<Clause, { type: "cards" }> => c.type === "cards" && !!c.filter.card)?.filter.card ?? null;

/** The kept bets that apply to a booster of `setCode` (one naming a card: its set's only),
 * paused ones included. */
export const keptFor = (bets: KeptBet[], setCode: string): BetRequest[] =>
  bets
    .filter((b) => b.setCode == null || b.setCode === setCode)
    .map(({ bet, stakeCents, active }) => ({ bet, stakeCents, active: active !== false }));

/** The bets to place: the active ones (a paused bet stays in the slip, not on the booster). */
export const activeBets = (bets: BetRequest[]): BetRequest[] =>
  bets.filter((b) => b.active !== false).map(({ bet, stakeCents }) => ({ bet, stakeCents }));

type Describe = Pick<I18n, "t" | "plural">;

/** What a filter's criteria say: "basic land, green, foil". */
function criteria(filter: CardFilter, i18n: Describe, moneyText: (cents: number) => string, cardName: (uuid: string) => string): string[] {
  const { t } = i18n;
  const parts: string[] = [];
  if (filter.card) parts.push(cardName(filter.card));
  if (filter.rarities?.length) parts.push(filter.rarities.map((r) => t(`bet.rarity.${r}` as MessageKey)).join(t("bet.or")));
  if (filter.types?.length) parts.push(filter.types.map((ty) => t(`bet.type.${ty}` as MessageKey)).join(t("bet.or")));
  if (filter.colors?.length) parts.push(filter.colors.map((c) => t(`bet.color.${c}` as MessageKey)).join(t("bet.or")));
  if (filter.finish === "foil") parts.push(t("bet.finish.foil"));
  if (filter.finish === "normal") parts.push(t("bet.finish.normal"));
  if (filter.treatment === "fullArt") parts.push(t("bet.treatment.fullArt"));
  if (filter.treatment === "specialFrame") parts.push(t("bet.treatment.specialFrame"));
  if (filter.minCents != null) parts.push(t("bet.priceMin", { amount: moneyText(filter.minCents) }));
  if (filter.maxCents != null) parts.push(t("bet.priceMax", { amount: moneyText(filter.maxCents) }));
  return parts;
}

/** What a condition says: "At least 1 card: basic land, green, foil", "Value: 2,000 or more". */
export function describeClause(
  clause: Clause,
  i18n: Describe,
  moneyText: (cents: number) => string,
  cardName: (uuid: string) => string,
): string {
  const { t, plural } = i18n;
  if (clause.type === "value") {
    return t(clause.compare === "atMost" ? "bet.valueAtMost" : "bet.valueAtLeast", { amount: moneyText(clause.cents) });
  }
  const kind = clause.filter.card ? "cards" : (clause.filter.kind ?? "cards");
  const none = clause.compare !== "atLeast" && clause.count === 0;
  const noun = plural(`bet.noun.${kind}` as "bet.noun.cards", none ? 1 : clause.count);
  const head = none
    ? t("bet.none", { noun })
    : t(`bet.compare.${clause.compare}` as MessageKey, { count: clause.count, noun });
  const parts = criteria(clause.filter, i18n, moneyText, cardName);
  return parts.length > 0 ? `${head}${t("bet.colon")}${parts.join(", ")}` : head;
}

/** What a whole bet says, its conditions joined by "and". */
export function describeBet(bet: Bet, i18n: Describe, moneyText: (cents: number) => string, cardName: (uuid: string) => string): string {
  return bet.clauses.map((c) => describeClause(c, i18n, moneyText, cardName)).join(` ${i18n.t("bet.and")} `);
}
