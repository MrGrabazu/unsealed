import { createContext, useContext } from "react";

/** Currency symbol of the game, provided by the wallet (configured on the Rust side). */
export const CurrencyContext = createContext("$");

export function useMoney() {
  const symbol = useContext(CurrencyContext);
  return (cents: number) => `${symbol}${(cents / 100).toFixed(2)}`;
}
