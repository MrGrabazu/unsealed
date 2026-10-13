import type { SealedStack, SetSummary } from "./api";
import { kindLabel } from "./boosterKinds";
import { useI18n } from "./i18n";
import { useMoney } from "./money";
import { PriceTrend } from "./PriceTrend";
import { SetIcon } from "./SetIcon";

/** Batch sizes offered next to "Open one". */
const BATCH_SIZES = [10, 100];

interface Props {
  sealed: SealedStack[];
  sets: SetSummary[];
  busy: boolean;
  onOpen: (stack: SealedStack) => void;
  onOpenMany: (stack: SealedStack, quantity: number) => void;
  onSell: (stack: SealedStack) => void;
  onGoShopping: () => void;
}

/** The sealed boosters the player owns, to pick the next one to open. */
export function SealedBoosters({ sealed, sets, busy, onOpen, onOpenMany, onSell, onGoShopping }: Props) {
  const money = useMoney();
  const { t, tx } = useI18n();
  if (sealed.length === 0) {
    return (
      <div className="empty-state">
        <p>{t("open.noSealed")}</p>
        <button className="primary" onClick={onGoShopping}>
          {t("open.goShopping")}
        </button>
      </div>
    );
  }

  return (
    <div className="sealed-grid">
      {sealed.map((s) => {
        const set = sets.find((candidate) => candidate.code === s.setCode);
        const art = set?.art;
        // Profit if the whole stack were sold back today.
        const pnl = s.quantity * s.sellValueCents - s.costCents;
        return (
          <div key={`${s.setCode}/${s.kind}`} className="sealed-tile">
            {/* Scryfall: an art crop is shown unaltered, with its artist credited next to it. */}
            {art && <img className="sealed-art" src={art.imageUrl} alt={art.cardName} draggable={false} />}
            <div className="sealed-body">
              <div className="sealed-title">
                <strong>
                  <SetIcon url={set?.iconUrl ?? null} name={s.setName} /> {s.setName}
                </strong>
                <span className="sealed-qty">×{s.quantity}</span>
              </div>
              <span className="hint">
                {kindLabel(t, s.kind)} · {tx("open.paid", { amount: money(s.costCents) })} ·{" "}
                {tx("open.worth", { amount: money(s.quantity * s.sellValueCents) })} ·{" "}
                <span className={pnl >= 0 ? "gain" : "loss"}>
                  {pnl >= 0 ? "+" : "−"}
                  {money(Math.abs(pnl))}
                </span>
              </span>
              <PriceTrend history={s.history} />
              {art && <span className="shop-credit">{t("art.credit", { card: art.cardName, artist: art.artist })}</span>}
              <div className="sealed-actions">
                <button className="primary" disabled={busy} onClick={() => onOpen(s)}>
                  {t("open.openOne")}
                </button>
                {BATCH_SIZES.filter((n) => s.quantity >= n).map((n) => (
                  <button key={n} disabled={busy} onClick={() => onOpenMany(s, n)}>
                    {t("open.openMany", { count: n })}
                  </button>
                ))}
                <button disabled={busy} onClick={() => onSell(s)}>
                  {tx("open.sellOne", { amount: money(s.sellValueCents) })}
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
