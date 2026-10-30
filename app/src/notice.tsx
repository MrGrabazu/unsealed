import { createContext, useContext } from "react";
import { useI18n } from "./i18n";
import type { IconName } from "./icons";
import { useMoneyText } from "./money";

/** Shows a toast (see `notices` of `GameScreen.tsx`): a passing word that leaves on its own. */
export type Notify = (text: string, icon?: IconName, title?: string) => void;

export const NoticeContext = createContext<Notify>(() => {});

/** The toast of a sale, as a purchase has its own: what went and what it brought in. `card`:
 * the name of the one card sold, if only one was. */
export function useSaleNotice(): (count: number, totalCents: number, card?: string) => void {
  const notify = useContext(NoticeContext);
  const { t, plural } = useI18n();
  const moneyText = useMoneyText();
  return (count, totalCents, card) => {
    const amount = moneyText(totalCents);
    notify(
      card && count === 1 ? t("collection.sold", { card, amount }) : plural("sale.doneText", count, { amount }),
      "coin",
      t("sale.done"),
    );
  };
}
