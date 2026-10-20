import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import { getHistory, undoTransaction, type HistoryEntry } from "./api";
import { BackToTop } from "./BackToTop";
import { kindLabel } from "./boosterKinds";
import { useI18n, type I18n } from "./i18n";
import { Icon, type IconName } from "./icons";
import { Loading } from "./Loading";
import { Money, useMoney } from "./money";

/** Entries loaded at a time. */
const PAGE = 50;

const KINDS = [
  "booster_purchase",
  "card_sale",
  "booster_sale",
  "sale_undone",
  "purchase_undone",
  "charity",
  "achievement_reward",
  "starting_balance",
  "bet_stake",
  "bet_win",
  "advance",
  "income",
  "debt_repaid",
] as const;

const ICONS: Record<string, IconName> = {
  booster_purchase: "shop",
  card_sale: "gem",
  booster_sale: "pack",
  sale_undone: "undo",
  purchase_undone: "undo",
  achievement_reward: "trophy",
  income: "chart",
  bet_stake: "dice",
  bet_win: "dice",
};

/** The ledger's filters, by kind of movement. */
const FILTERS = {
  all: null,
  purchases: ["booster_purchase", "purchase_undone"],
  sales: ["card_sale", "booster_sale", "sale_undone"],
  income: ["income", "charity", "achievement_reward", "starting_balance", "advance", "debt_repaid"],
  bets: ["bet_stake", "bet_win"],
} as const;
type Filter = keyof typeof FILTERS;

function kindName(t: I18n["t"], kind: string): string {
  return (KINDS as readonly string[]).includes(kind)
    ? t(`history.kind.${kind as (typeof KINDS)[number]}`)
    : t("history.kind.other", { kind });
}

const entryKey = (e: HistoryEntry, i: number) => `${e.at}-${e.kind}-${i}`;

/** The account book: every movement of the wallet, day by day, and the one chosen beside it
 * in detail; sales (and very recent purchases) can be undone from there. */
export function History({ onChanged }: { onChanged: () => void }) {
  const { t, tx, lang } = useI18n();
  const money = useMoney();
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [chosen, setChosen] = useState<string | null>(null);
  // Transaction waiting for a second click to confirm undoing it.
  const [confirming, setConfirming] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const list = useRef<HTMLDivElement>(null);

  const load = useCallback(async (count: number) => {
    try {
      const view = await getHistory(count, 0);
      setEntries(view.entries);
      setHasMore(view.hasMore);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load(PAGE);
  }, [load]);

  async function undo(entry: HistoryEntry) {
    if (entry.transactionId == null) return;
    if (confirming !== entry.transactionId) {
      setConfirming(entry.transactionId);
      return;
    }
    setConfirming(null);
    setBusy(true);
    try {
      await undoTransaction(entry.transactionId);
      setNotice(t("history.undoneNotice"));
      await load(Math.max(PAGE, entries?.length ?? 0));
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!entries && !error) return <Loading label={t("history.loading")} />;

  const kinds = FILTERS[filter] as readonly string[] | null;
  const indexed = (entries ?? []).map((e, i) => ({ e, key: entryKey(e, i) }));
  const shown = indexed.filter(({ e }) => !kinds || kinds.includes(e.kind));
  const current = indexed.find((x) => x.key === chosen) ?? shown[0] ?? null;

  // Day by day: today, yesterday, then the date.
  const dayOf = (at: number) => new Date(at * 1000).toDateString();
  const today = new Date().toDateString();
  const yesterday = new Date(Date.now() - 86_400_000).toDateString();
  const dayLabel = (at: number) => {
    const day = dayOf(at);
    if (day === today) return t("history.today");
    if (day === yesterday) return t("history.yesterday");
    return new Date(at * 1000).toLocaleDateString(lang, { dateStyle: "full" });
  };
  const time = (at: number) => new Date(at * 1000).toLocaleTimeString(lang, { timeStyle: "short" });
  const itemName = (item: HistoryEntry["items"][number]) =>
    item.boosterKind ? `${item.name} · ${kindLabel(t, item.boosterKind)}` : `${item.name}${item.foil ? t("stats.foilSuffix") : ""}`;
  const signed = (cents: number) => (
    <span className={cents >= 0 ? "gain" : "loss"}>
      {cents >= 0 ? "+ " : "− "}
      {money(Math.abs(cents))}
    </span>
  );

  return (
    <div className="split history-split">
      <section className="frame">
        <div className="cartouche">{t("history.book")}</div>
        <div className="segmented" role="group" style={{ marginBottom: 10 }}>
          {(Object.keys(FILTERS) as Filter[]).map((f) => (
            <button key={f} className={filter === f ? "active" : ""} onClick={() => setFilter(f)}>
              {t(`history.filter.${f}`)}
            </button>
          ))}
        </div>
        {error && <p className="error">{error}</p>}
        {notice && <p className="notice">{notice}</p>}
        <div className="scroll" ref={list}>
          {shown.length === 0 ? (
            <p className="hint">{t("history.empty")}</p>
          ) : (
            <table className="ledger">
              <tbody>
                {shown.map(({ e, key }, i) => (
                  <Fragment key={key}>
                    {(i === 0 || dayOf(shown[i - 1].e.at) !== dayOf(e.at)) && (
                      <tr className="ledger-day">
                        <td colSpan={3}>{dayLabel(e.at)}</td>
                      </tr>
                    )}
                    <tr
                      className={`${current?.key === key ? "sel" : ""} ${e.undoneAt ? "undone" : ""}`}
                      onClick={() => {
                        setChosen(key);
                        setConfirming(null);
                      }}
                    >
                      <td>
                        <span className="ledger-ico">
                          <Icon name={ICONS[e.kind] ?? "star"} />
                        </span>
                        {kindName(t, e.kind)}
                        {e.items.length > 0 && (
                          <span className="ledger-sub">
                            {e.items.length === 1
                              ? t("history.item", { quantity: e.items[0].quantity, name: itemName(e.items[0]) })
                              : t("history.items", { count: e.items.reduce((n, item) => n + item.quantity, 0) })}
                          </span>
                        )}
                      </td>
                      <td className="ledger-time">{time(e.at)}</td>
                      <td className="ledger-amount">{signed(e.amountCents)}</td>
                    </tr>
                  </Fragment>
                ))}
              </tbody>
            </table>
          )}
          {hasMore && (
            <div className="controls">
              <button disabled={busy} onClick={() => void load((entries?.length ?? 0) + PAGE)}>
                {t("history.more")}
              </button>
            </div>
          )}
          <BackToTop target={list} />
        </div>
      </section>

      <aside className="frame history-side">
        {current ? (
          <>
            <div className="cartouche">
              {kindName(t, current.e.kind)} · {time(current.e.at)}
            </div>
            <div className="scroll">
              <div className="bigvalue">
                <small>{dayLabel(current.e.at)}</small>
                <b>{signed(current.e.amountCents)}</b>
              </div>
              {current.e.items.length > 0 && (
                <div className="history-items">
                  {current.e.items.map((item, j) => (
                    <div key={j} className="history-item">
                      <span>{t("history.item", { quantity: item.quantity, name: itemName(item) })}</span>
                      <b>
                        <Money cents={item.unitCents * item.quantity} />
                      </b>
                      <small>{tx("history.each", { amount: money(item.unitCents) })}</small>
                    </div>
                  ))}
                </div>
              )}
              {current.e.undoneAt != null && (
                <p className="hint" style={{ textAlign: "center" }}>
                  {t("history.undone", { date: new Date(current.e.undoneAt * 1000).toLocaleString(lang) })}
                </p>
              )}
            </div>
            {current.e.undo && current.e.transactionId != null && (
              <div className="buy-bar">
                <p className="meta" style={{ textAlign: "center", margin: 0 }}>
                  {current.e.undo.affordable ? t("history.undoHint", { penalty: current.e.undo.penaltyPercent }) : t("history.cannotAfford")}
                </p>
                <button
                  className="big wax"
                  disabled={busy || !current.e.undo.affordable}
                  onMouseLeave={() => confirming === current.e.transactionId && setConfirming(null)}
                  onClick={() => void undo(current.e)}
                >
                  {confirming === current.e.transactionId
                    ? t("history.undoConfirm")
                    : current.e.kind === "booster_purchase"
                      ? tx("history.undoPurchase", { amount: money(current.e.undo.amountCents), penalty: current.e.undo.penaltyPercent })
                      : tx("history.undoSale", { amount: money(current.e.undo.amountCents), penalty: current.e.undo.penaltyPercent })}
                </button>
              </div>
            )}
          </>
        ) : (
          <>
            <div className="cartouche">{t("history.book")}</div>
            <p className="hint">{t("history.hint")}</p>
          </>
        )}
      </aside>
    </div>
  );
}
