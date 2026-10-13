import type { ShopItem, WalletView } from "./api";
import { useMoney } from "./money";

interface Props {
  shop: ShopItem[];
  wallet: WalletView;
  onBuy: (item: ShopItem, quantity: number) => void;
  onBuyAndOpen: (item: ShopItem) => void;
  onClaimCharity: () => void;
  busy: boolean;
}

/** The shop: buy sealed boosters, and ask for help when truly stranded. */
export function Boosterium({ shop, wallet, onBuy, onBuyAndOpen, onClaimCharity, busy }: Props) {
  const money = useMoney();
  const canAfford = (item: ShopItem, quantity: number) => wallet.balanceCents >= item.priceCents * quantity;

  return (
    <section>
      {wallet.charity.available && (
        <div className="charity">
          <p>
            You have no money, no unopened booster and no card worth selling. The Boosterium offers{" "}
            <strong>{money(wallet.charity.amountCents)}</strong> — once per day, and only when you are really stranded.
          </p>
          <button className="primary" onClick={onClaimCharity} disabled={busy}>
            Accept {money(wallet.charity.amountCents)}
          </button>
        </div>
      )}

      <h2>Boosterium</h2>
      <p className="hint">
        Buy sealed boosters, then open them. Prices follow the market value of the cards, so unopened boosters can
        gain or lose value: sell them back, or open them. Sell cards from your collection to fund the next ones.
      </p>
      <div className="shop">
        {shop.map((item) => (
          <div key={`${item.setCode}/${item.kind}`} className="shop-row">
            <div>
              <div className="shop-name">{item.setName}</div>
              <div className="hint">
                {item.setCode} · {item.kind} booster · sells back for {money(item.sellBackCents)}
              </div>
            </div>
            <div className="shop-price" title="Follows the market value of the cards">
              {money(item.priceCents)}
            </div>
            <div className="shop-actions">
              <button onClick={() => onBuy(item, 1)} disabled={busy || !canAfford(item, 1)}>
                Buy 1
              </button>
              <button onClick={() => onBuy(item, 10)} disabled={busy || !canAfford(item, 10)}>
                Buy 10
              </button>
              <button className="primary" onClick={() => onBuyAndOpen(item)} disabled={busy || !canAfford(item, 1)}>
                Buy &amp; open
              </button>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
