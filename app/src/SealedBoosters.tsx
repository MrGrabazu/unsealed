import type { SealedStack, SetSummary } from "./api";
import { kindLabel } from "./boosterKinds";
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
  if (sealed.length === 0) {
    return (
      <div className="empty-state">
        <p>You have no sealed booster left.</p>
        <button className="primary" onClick={onGoShopping}>
          Go to the Boosterium
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
                {kindLabel(s.kind)} · paid {money(s.costCents)} · worth {money(s.quantity * s.sellValueCents)} if sold ·{" "}
                <span className={pnl >= 0 ? "gain" : "loss"}>
                  {pnl >= 0 ? "+" : "−"}
                  {money(Math.abs(pnl))}
                </span>
              </span>
              <PriceTrend history={s.history} />
              {art && (
                <span className="shop-credit">
                  Art: {art.cardName} by {art.artist} · © Wizards of the Coast
                </span>
              )}
              <div className="sealed-actions">
                <button className="primary" disabled={busy} onClick={() => onOpen(s)}>
                  Open one
                </button>
                {BATCH_SIZES.filter((n) => s.quantity >= n).map((n) => (
                  <button key={n} disabled={busy} onClick={() => onOpenMany(s, n)}>
                    Open ×{n}
                  </button>
                ))}
                <button disabled={busy} onClick={() => onSell(s)}>
                  Sell one ({money(s.sellValueCents)})
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
