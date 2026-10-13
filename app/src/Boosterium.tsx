import { useState } from "react";
import type { SetSummary, ShopItem, WalletView } from "./api";
import { kindLabel, kindRank } from "./boosterKinds";
import { useI18n } from "./i18n";
import { useMoney, useMoneyText } from "./money";
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
  const moneyText = useMoneyText();
  const { t, tx, plural } = useI18n();
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
    .filter(
      (set) =>
        !query ||
        set.name.toLowerCase().includes(query) ||
        set.englishName.toLowerCase().includes(query) ||
        set.code.toLowerCase() === query,
    )
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
          <p>{tx("shop.charity", { amount: money(wallet.charity.amountCents) })}</p>
          <button className="primary" onClick={onClaimCharity} disabled={busy}>
            {tx("shop.charityAccept", { amount: money(wallet.charity.amountCents) })}
          </button>
        </div>
      )}

      <h2>Boosterium</h2>
      <p className="hint">{t("shop.intro")}</p>

      <div className="controls">
        <input type="search" placeholder={t("shop.search")} value={search} onChange={(e) => setSearch(e.target.value)} />
        <select value={year} onChange={(e) => setYear(e.target.value)}>
          <option value="all">{t("shop.allYears")}</option>
          {years.map((y) => (
            <option key={y} value={y}>
              {y}
            </option>
          ))}
        </select>
        <select value={maxPrice} onChange={(e) => setMaxPrice(Number(e.target.value))}>
          {PRICE_LIMITS.map((limit) => (
            <option key={limit} value={limit}>
              {/* An <option> holds text only: the amount is written out. */}
              {limit === 0 ? t("shop.anyPrice") : t("shop.under", { amount: moneyText(limit) })}
            </option>
          ))}
        </select>
      </div>

      <div className="shop">
        {groups.length === 0 && <p className="hint">{t("shop.noMatch")}</p>}
        {groups.map(({ set, products }) => {
          const expanded = open.has(set.code);
          const cheapest = Math.min(...products.map((p) => p.priceCents));
          return (
            <div key={set.code} className={`shop-set ${expanded ? "open" : ""}`}>
              <button
                className={`shop-set-header ${set.art ? "with-art" : ""}`}
                onClick={() => toggle(set.code)}
                aria-expanded={expanded}
              >
                {/* Scryfall: an art crop is shown unaltered (no blur, no filter, full opacity), with
                    its artist credited on the row. A fade of the panel colour covers its left edge so
                    the text stays readable; the image itself is never recoloured. */}
                {set.art && (
                  <>
                    <img className="shop-art-bg" src={set.art.imageUrl} alt="" aria-hidden="true" draggable={false} />
                    <span className="shop-fade" aria-hidden="true" />
                  </>
                )}
                <span className="shop-chevron">{expanded ? "▾" : "▸"}</span>
                <span className="shop-set-title">
                  <span className="shop-name">
                    <SetIcon url={set.iconUrl} name={set.name} /> {set.name}
                  </span>
                  <span className="hint">
                    {set.code} · {t("shop.released", { date: set.releaseDate })} ·{" "}
                    {plural("shop.boosterTypes", products.length)}
                  </span>
                  {set.art && (
                    <span className="shop-credit">
                      {t("art.credit", { card: set.art.cardName, artist: set.art.artist })}
                    </span>
                  )}
                </span>
                <span className="shop-from">{tx("shop.from", { amount: money(cheapest) })}</span>
              </button>

              {expanded && (
                <div className="shop-products">
                  {products.map((item) => (
                    <div key={`${item.setCode}/${item.kind}`} className="shop-row">
                      <div>
                        <div className="shop-name">{kindLabel(t, item.kind)}</div>
                        <div className="hint">{tx("shop.sellsBack", { amount: money(item.sellBackCents) })}</div>
                      </div>
                      <PriceTrend history={item.history} />
                      <div className="shop-price" title={t("shop.priceTitle")}>
                        {money(item.priceCents)}
                      </div>
                      <div className="shop-actions">
                        <button onClick={() => onBuy(item, 1)} disabled={busy || !canAfford(item, 1)}>
                          {t("shop.buy", { count: 1 })}
                        </button>
                        <button onClick={() => onBuy(item, 10)} disabled={busy || !canAfford(item, 10)}>
                          {t("shop.buy", { count: 10 })}
                        </button>
                        <button
                          className="primary"
                          onClick={() => onBuyAndOpen(item)}
                          disabled={busy || !canAfford(item, 1)}
                        >
                          {t("shop.buyAndOpen")}
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
