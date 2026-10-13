import { useCallback, useEffect, useState } from "react";
import { getHistory, undoTransaction, type HistoryEntry } from "./api";
import { kindLabel } from "./boosterKinds";
import { useI18n, type I18n } from "./i18n";
import { Loading } from "./Loading";
import { useMoney } from "./money";

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
] as const;

function kindName(t: I18n["t"], kind: string): string {
  return (KINDS as readonly string[]).includes(kind)
    ? t(`history.kind.${kind as (typeof KINDS)[number]}`)
    : t("history.kind.other", { kind });
}

/** Every movement of the wallet, with the purchases and sales in detail; sales (and very
 * recent purchases) can be undone. */
export function History({ onChanged }: { onChanged: () => void }) {
  const { t, tx, formatDate } = useI18n();
  const money = useMoney();
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // Transaction waiting for a second click to confirm undoing it.
  const [confirming, setConfirming] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

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

  return (
    <section>
      <p className="hint">{t("history.hint")}</p>
      {error && <p className="error">{error}</p>}
      {notice && <p className="notice">{notice}</p>}
      {entries && entries.length === 0 && <p className="hint">{t("history.empty")}</p>}
      {entries && entries.length > 0 && (
        <table className="stat-table history-table">
          <thead>
            <tr>
              <th>{t("history.colDate")}</th>
              <th>{t("history.colWhat")}</th>
              <th>{t("history.colAmount")}</th>
              <th>{t("history.colUndo")}</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e, i) => (
              <tr key={`${e.at}-${i}`} className={e.undoneAt ? "undone" : ""}>
                <td>{formatDate(e.at)}</td>
                <td>
                  <strong>{kindName(t, e.kind)}</strong>
                  {e.items.map((item, j) => (
                    <div key={j} className="hint">
                      {t("history.item", {
                        quantity: item.quantity,
                        name: item.boosterKind
                          ? `${item.name} · ${kindLabel(t, item.boosterKind)}`
                          : `${item.name}${item.foil ? t("stats.foilSuffix") : ""}`,
                      })}{" "}
                      · {tx("history.each", { amount: money(item.unitCents) })}
                    </div>
                  ))}
                </td>
                <td className={e.amountCents >= 0 ? "gain" : "loss"}>
                  {e.amountCents >= 0 ? "+" : "−"}
                  {money(Math.abs(e.amountCents))}
                </td>
                <td>
                  {e.undoneAt != null && (
                    <span className="hint">{t("history.undone", { date: formatDate(e.undoneAt) })}</span>
                  )}
                  {e.undo && e.transactionId != null && (
                    <button
                      className={`sell ${confirming === e.transactionId ? "confirm" : ""}`}
                      disabled={busy || !e.undo.affordable}
                      title={e.undo.affordable ? undefined : t("history.cannotAfford")}
                      onMouseLeave={() => confirming === e.transactionId && setConfirming(null)}
                      onClick={() => void undo(e)}
                    >
                      {confirming === e.transactionId
                        ? t("history.undoConfirm")
                        : e.kind === "booster_purchase"
                          ? tx("history.undoPurchase", { amount: money(e.undo.amountCents), penalty: e.undo.penaltyPercent })
                          : tx("history.undoSale", { amount: money(e.undo.amountCents), penalty: e.undo.penaltyPercent })}
                    </button>
                  )}
                </td>
              </tr>
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
    </section>
  );
}
