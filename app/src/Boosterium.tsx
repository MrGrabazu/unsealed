import { useState } from "react";
import type { SetSummary, ShopItem, WalletView } from "./api";
import { kindLabel, kindRank } from "./boosterKinds";
import { useMoney } from "./money";
import { PriceTrend } from "./PriceTrend";
import { SetIcon } from "./SetIcon";

interface Props {
  shop: ShopItem[];
  sets: SetSummary[];
  wallet: WalletView;
  onBuy: (item: ShopItem, quantity: number) => void;
  onBuyAndOpen: (item: ShopItem) => void;
  onClaimCharity: () => void;
  busy: boolean;
}

/** Budget filter choices, in cents (0 = any price). */
const PRICE_LIMITS = [0, 1_000, 2_500, 5_000];

interface SetGroup {
  set: SetSummary;
  /** What the Boosterium sells for this set. Booster boxes will join this list later. */
  products: ShopItem[];
}

/** The shop: buy sealed boosters, and ask for help when truly stranded. */
export function Boosterium({ shop, sets, wallet, onBuy, onBuyAndOpen, onClaimCharity, busy }: Props) {
  const money = useMoney();
  const [search, setSearch] = useState("");
  const [year, setYear] = useState("all");
  const [maxPrice, setMaxPrice] = useState(0);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const canAfford = (item: ShopItem, quantity: number) => wallet.balanceCents >= item.priceCents * quantity;

  const inBudget = (item: ShopItem) => maxPrice === 0 || item.priceCents <= maxPrice;
  const years = [...new Set(sets.map((s) => s.releaseDate.slice(0, 4)))];
  // `sets` comes newest first.
  const query = search.trim().toLowerCase();
  const groups: SetGroup[] = sets
    .filter((set) => year === "all" || set.releaseDate.startsWith(year))
    .filter((set) => !query || set.name.toLowerCase().includes(query) || set.code.toLowerCase() === query)
    .map((set) => ({
      set,
      products: shop
        .filter((item) => item.setCode === set.code && inBudget(item))
        .sort((a, b) => kindRank(a.kind) - kindRank(b.kind) || a.kind.localeCompare(b.kind)),
    }))
    .filter((g) => g.products.length > 0);

  const toggle = (code: string) =>
    setOpen((current) => {
      const next = new Set(current);
      if (!next.delete(code)) next.add(code);
      return next;
    });

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

      <div className="controls">
        <input
          type="search"
          placeholder="Search a set (name or code)"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <select value={year} onChange={(e) => setYear(e.target.value)}>
          <option value="all">All years</option>
          {years.map((y) => (
            <option key={y} value={y}>
              {y}
            </option>
          ))}
        </select>
        <select value={maxPrice} onChange={(e) => setMaxPrice(Number(e.target.value))}>
          {PRICE_LIMITS.map((limit) => (
            <option key={limit} value={limit}>
              {limit === 0 ? "Any price" : `Under ${money(limit)}`}
            </option>
          ))}
        </select>
      </div>

      <div className="shop">
        {groups.length === 0 && <p className="hint">No booster matches these filters.</p>}
        {groups.map(({ set, products }) => {
          const expanded = open.has(set.code);
          const cheapest = Math.min(...products.map((p) => p.priceCents));
          return (
            <div key={set.code} className={`shop-set ${expanded ? "open" : ""}`}>
              <button className="shop-set-header" onClick={() => toggle(set.code)} aria-expanded={expanded}>
                <span className="shop-chevron">{expanded ? "▾" : "▸"}</span>
                {/* Scryfall: an art crop is shown unaltered, with its artist credited next to it. */}
                {set.art && <img className="shop-art" src={set.art.imageUrl} alt={set.art.cardName} draggable={false} />}
                <span className="shop-set-title">
                  <span className="shop-name">
                    <SetIcon url={set.iconUrl} name={set.name} /> {set.name}
                  </span>
                  <span className="hint">
                    {set.code} · released {set.releaseDate} · {products.length} booster
                    {products.length > 1 ? " types" : " type"}
                  </span>
                  {set.art && (
                    <span className="shop-credit">
                      Art: {set.art.cardName} by {set.art.artist} · © Wizards of the Coast
                    </span>
                  )}
                </span>
                <span className="shop-from">from {money(cheapest)}</span>
              </button>

              {expanded && (
                <div className="shop-products">
                  {products.map((item) => (
                    <div key={`${item.setCode}/${item.kind}`} className="shop-row">
                      <div>
                        <div className="shop-name">{kindLabel(item.kind)}</div>
                        <div className="hint">sells back for {money(item.sellBackCents)}</div>
                      </div>
                      <PriceTrend history={item.history} />
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
                        <button
                          className="primary"
                          onClick={() => onBuyAndOpen(item)}
                          disabled={busy || !canAfford(item, 1)}
                        >
                          Buy &amp; open
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
