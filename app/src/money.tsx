import { createContext, useContext, type ReactNode } from "react";
import { useI18n } from "./i18n";

// The game's currency: Seals. The Rust side counts in cents of the market currency; the
// player only ever sees Seals, derived from them (see `economy.json`, `currency`), always
// with the same coin, so every amount reads as one and the same currency.

/** How cents become Seals, sent by the game with the wallet. */
export interface CurrencyRules {
  sealsPerCent: number;
}

export const DEFAULT_CURRENCY: CurrencyRules = { sealsPerCent: 1 };
export const CurrencyContext = createContext<CurrencyRules>(DEFAULT_CURRENCY);

const toSeals = (rules: CurrencyRules, cents: number) => Math.round(cents * rules.sealsPerCent);

/** A wax-seal coin, drawn for Unsealed. */
function Coin() {
  return (
    <svg className="coin" viewBox="0 0 24 24" aria-hidden="true">
      <path
        className="coin-edge"
        d="M12 1.5l2.1 1.6 2.6-.3 1.2 2.3 2.4 1.1-.2 2.6 1.6 2.1-1.6 2.1.2 2.6-2.4 1.1-1.2 2.3-2.6-.3L12 22.5l-2.1-1.6-2.6.3-1.2-2.3-2.4-1.1.2-2.6L2.3 13.1 3.9 11l-.2-2.6 2.4-1.1 1.2-2.3 2.6.3z"
      />
      <circle className="coin-face" cx="12" cy="12" r="6.6" />
      <path className="coin-mark" d="M9.2 8.6v3.9a2.8 2.8 0 0 0 5.6 0V8.6" />
    </svg>
  );
}

/** An amount in Seals with its coin. `cents` are cents of the market currency. */
export function Money({ cents }: { cents: number }) {
  const rules = useContext(CurrencyContext);
  const { formatNumber, plural } = useI18n();
  const seals = toSeals(rules, cents);
  return (
    <span className="money" title={plural("currency.amount", seals)}>
      {formatNumber(seals)}
      <Coin />
    </span>
  );
}

/** Amounts as elements (number and coin), for display. */
export function useMoney(): (cents: number) => ReactNode {
  return (cents) => <Money cents={cents} />;
}

/** Texts from the configuration (achievement descriptions) write amounts as `{cents:N}`,
 * so they stay right whatever the Seals ratio: this writes them out ("10 000 Seals"). */
export function useAmountsInText(): (text: string) => string {
  const moneyText = useMoneyText();
  return (text) => text.replace(/\{cents:(\d+)\}/g, (_, cents: string) => moneyText(Number(cents)));
}

/** Amounts as plain text ("761 Seals"), where an element cannot go (titles, options, notices). */
export function useMoneyText(): (cents: number) => string {
  const rules = useContext(CurrencyContext);
  const { plural } = useI18n();
  return (cents) => plural("currency.amount", toSeals(rules, cents));
}
