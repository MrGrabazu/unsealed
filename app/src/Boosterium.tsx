import { useEffect, useMemo, useRef, useState } from "react";
import { getArtifacts, getPinnedSets, setPinnedSets, type ArtifactView, type SetSummary, type ShopItem, type WalletView } from "./api";
import { ArtifactGlyph, artifactMetal } from "./ArtifactGlyph";
import { BackToTop } from "./BackToTop";
import { NearImage } from "./CardImage";
import { kindLabel, kindRank } from "./boosterKinds";
import { useI18n } from "./i18n";
import { Money, useAmountsInText, useMoney, useMoneyText } from "./money";
import { PriceTrend } from "./PriceTrend";
import { BoxGauge } from "./SealedBoosters";
import { BoosterNote } from "./BoosterNote";
import { SetIcon } from "./SetIcon";

interface Props {
  shop: ShopItem[];
  sets: SetSummary[];
  wallet: WalletView;
  /** Buys `quantity` boosters, and opens one of them at once with `openNow`. */
  onBuy: (item: ShopItem, quantity: number, openNow: boolean) => void;
  onClaimCharity: () => void;
  busy: boolean;
  /** A set to show on arrival (from its collection, or `CURIOS`); `at` tells two requests apart. */
  focus?: { setCode: string; at: number } | null;
  /** Bumped when an artifact is got: the curios on sale are read again. */
  artifactsKey: number;
  onBuyArtifact: (id: string) => void;
}

/** The curios' entry in the list (in place of a set code). */
export const CURIOS = "__curios";

/** Budget filter choices, in cents (0 = any price). */
const PRICE_LIMITS = [0, 1_000, 2_500, 5_000];
/** Largest number of boosters bought at once (as the game allows). */
const MAX_QUANTITY = 100;
/** A set released this recently is new. */
const NEW_FOR_DAYS = 45;

interface SetGroup {
  set: SetSummary;
  /** What the Boosterium sells for this set. Booster boxes will join this list later. */
  products: ShopItem[];
}

/** The shop, laid out like a game's: every set as an illustrated slot (the favourites
 * first), and the chosen one beside them with its boosters and a single way to buy. */
export function Boosterium({ shop, sets, wallet, onBuy, onClaimCharity, busy, focus, artifactsKey, onBuyArtifact }: Props) {
  const money = useMoney();
  const moneyText = useMoneyText();
  const { t, tx, lang, plural } = useI18n();
  const amountsInText = useAmountsInText();
  // The artifacts on sale (each one once: gone when bought).
  const [curios, setCurios] = useState<ArtifactView[]>([]);
  useEffect(() => {
    getArtifacts(lang)
      .then((all) => setCurios(all.filter((a) => a.status === "shop")))
      .catch(() => setCurios([]));
  }, [artifactsKey, lang]);
  const [search, setSearch] = useState("");
  const [year, setYear] = useState("all");
  const [maxPrice, setMaxPrice] = useState(0);
  const [chosen, setChosen] = useState<string | null>(null);
  const [kind, setKind] = useState<string | null>(null);
  const [quantity, setQuantity] = useState(1);
  const [openNow, setOpenNow] = useState(false);
  const list = useRef<HTMLDivElement>(null);
  // Sets pinned as favourites (kept in the game).
  const [pinned, setPinned] = useState<string[]>([]);
  useEffect(() => {
    getPinnedSets()
      .then(setPinned)
      .catch(() => {});
  }, []);
  const togglePin = (code: string) => {
    const next = pinned.includes(code) ? pinned.filter((c) => c !== code) : [...pinned, code];
    setPinned(next);
    setPinnedSets(next).catch(() => {});
  };

  // Arriving for one set: no filter hides it, it is the one chosen, and in view.
  useEffect(() => {
    if (!focus) return;
    setSearch("");
    setYear("all");
    setMaxPrice(0);
    setChosen(focus.setCode);
    requestAnimationFrame(() =>
      document.getElementById(`shop-set-${focus.setCode}`)?.scrollIntoView({ block: "center", behavior: "smooth" }),
    );
  }, [focus]);

  const years = [...new Set(sets.map((s) => s.releaseDate.slice(0, 4)))];
  const query = search.trim().toLowerCase();
  // `sets` comes newest first.
  const groups: SetGroup[] = useMemo(
    () =>
      sets
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
            .filter((item) => item.setCode === set.code && (maxPrice === 0 || item.priceCents <= maxPrice))
            .sort((a, b) => kindRank(a.kind) - kindRank(b.kind) || a.kind.localeCompare(b.kind)),
        }))
        .filter((g) => g.products.length > 0),
    [sets, shop, year, query, maxPrice],
  );
  const favorites = groups.filter((g) => pinned.includes(g.set.code));
  const others = groups.filter((g) => !pinned.includes(g.set.code));

  // The chosen set: the one picked, else the first favourite, else the newest; or the curios.
  const showCurios = chosen === CURIOS && curios.length > 0;
  const current = showCurios ? null : (groups.find((g) => g.set.code === chosen) ?? favorites[0] ?? others[0] ?? null);
  const item = current?.products.find((p) => p.kind === kind) ?? current?.products[0] ?? null;
  const total = item ? item.priceCents * quantity : 0;
  const affordable = !!item && wallet.balanceCents >= total;
  const buy = () => item && affordable && !busy && onBuy(item, quantity, openNow);

  // Enter buys; + and − change the quantity; F makes the set a favourite (not while typing).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (["INPUT", "SELECT", "TEXTAREA", "BUTTON"].includes(target.tagName)) return;
      if (e.key === "Enter") buy();
      else if (e.key === "+") setQuantity((q) => Math.min(MAX_QUANTITY, q + 1));
      else if (e.key === "-") setQuantity((q) => Math.max(1, q - 1));
      else if (e.key.toLowerCase() === "f" && current) togglePin(current.set.code);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const today = Date.now();
  const isNew = (set: SetSummary) => (today - Date.parse(set.releaseDate)) / 86_400_000 < NEW_FOR_DAYS;

  const slot = ({ set, products }: SetGroup) => {
    const cheapest = Math.min(...products.map((p) => p.priceCents));
    const favorite = pinned.includes(set.code);
    return (
      <button
        key={set.code}
        id={`shop-set-${set.code}`}
        className={`slot ${current?.set.code === set.code ? "sel" : ""}`}
        onClick={() => {
          setChosen(set.code);
          setKind(null);
          setQuantity(1);
        }}
      >
        {set.art ? (
          <NearImage className="slot-art" src={set.art.imageUrl} alt="" />
        ) : (
          <span className="slot-symbol">
            <SetIcon url={set.iconUrl} name={set.name} />
          </span>
        )}
        {isNew(set) && <span className="slot-new">{t("shop.new")}</span>}
        <span className="slot-top">
          <span className="medallion">
            <SetIcon url={set.iconUrl} name={set.name} />
          </span>
          <span
            className={`star-toggle ${favorite ? "on" : ""}`}
            role="switch"
            aria-checked={favorite}
            title={t(favorite ? "shop.unpin" : "shop.pin")}
            onClick={(e) => {
              e.stopPropagation();
              togglePin(set.code);
            }}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 2.6l2.9 6 6.6.8-4.9 4.5 1.3 6.5L12 17.2l-5.9 3.2 1.3-6.5-4.9-4.5 6.6-.8z" />
            </svg>
          </span>
        </span>
        <span className="slot-plate">
          <b>{set.name}</b>
          <span>{tx("shop.from", { amount: money(cheapest) })}</span>
        </span>
      </button>
    );
  };

  return (
    <div className="split">
      <section className="frame">
        <div className="cartouche">{t("shop.boutique")}</div>
        {wallet.charity.available && (
          <div className="charity">
            <p>{tx("shop.charity", { amount: money(wallet.charity.amountCents) })}</p>
            <button className="primary" onClick={onClaimCharity} disabled={busy}>
              {tx("shop.charityAccept", { amount: money(wallet.charity.amountCents) })}
            </button>
          </div>
        )}
        <div className="shop-tools">
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
        <div className="scroll" ref={list}>
          {/* The curios on sale: one entry, opening them in the panel. */}
          {curios.length > 0 && (
            <button
              id={`shop-set-${CURIOS}`}
              className={`slot curios-slot ${showCurios ? "sel" : ""}`}
              onClick={() => setChosen(CURIOS)}
            >
              <span className="curios-row">
                {curios.slice(0, 5).map((a) => (
                  <ArtifactGlyph key={a.id} glyph={a.glyph} color={artifactMetal(a.glyph, a.rarity)} />
                ))}
              </span>
              <span className="slot-plate">
                <b>{t("curios.shopTitle")}</b>
                <span>{plural("curios.onSaleCount", curios.length)}</span>
              </span>
            </button>
          )}
          {groups.length === 0 && <p className="hint">{t("shop.noMatch")}</p>}
          {favorites.length > 0 && (
            <>
              <h3 className="shelf-title">★ {t("shop.favorites")}</h3>
              <div className="slots">{favorites.map(slot)}</div>
              {others.length > 0 && <h3 className="shelf-title">{t("shop.allSets")}</h3>}
            </>
          )}
          <div className="slots">{others.map(slot)}</div>
          <BackToTop target={list} />
        </div>
      </section>

      <section className="frame set-detail">
        {showCurios ? (
          <>
            <div className="cartouche">{t("curios.shopTitle")}</div>
            <div className="scroll">
              <p className="hint">{t("curios.shopHint")}</p>
              {curios.map((a) => {
                const metal = artifactMetal(a.glyph, a.rarity);
                const price = a.priceCents ?? 0;
                return (
                  <div key={a.id} className="curio-ware" style={{ ["--metal" as string]: metal }}>
                    <span className="curio-ware-glyph">
                      <ArtifactGlyph glyph={a.glyph} color={metal} />
                    </span>
                    <div>
                      <b>{a.name}</b>
                      <small style={{ color: metal }}>{t(`curios.rarity.${a.rarity}`)}</small>
                      <p>{amountsInText(a.effect ?? "")}</p>
                      <button className="big" disabled={busy || wallet.balanceCents < price} onClick={() => onBuyArtifact(a.id)}>
                        {tx("shop.buyFor", { amount: money(price) })}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </>
        ) : current && item ? (
          <>
            <div className="cartouche">{current.set.name}</div>
            <div className="scroll">
            {/* Scryfall: an art crop is shown unaltered, its artist credited beside it. */}
            <div className="set-plate-art">
              {current.set.art ? (
                <img className="art" src={current.set.art.imageUrl} alt="" draggable={false} />
              ) : (
                <SetIcon url={current.set.iconUrl} name={current.set.name} />
              )}
            </div>
            {current.set.art && (
              <div className="shop-credit" style={{ marginTop: 6 }}>
                {t("art.credit", { card: current.set.art.cardName, artist: current.set.art.artist })}
              </div>
            )}
            <h2>
              <span className="medallion">
                <SetIcon url={current.set.iconUrl} name={current.set.name} />
              </span>
              {current.set.name}
            </h2>
            <div className="meta">
              {current.set.code} · {t("shop.released", { date: current.set.releaseDate })}
            </div>
            <div className="gild-divider" />
            {current.products.map((p) => (
              <button key={p.kind} className={`product ${p.kind === item.kind ? "sel" : ""}`} onClick={() => setKind(p.kind)}>
                <b>{kindLabel(t, p.kind)}</b>
                <span className="p" title={t("shop.priceTitle")}>
                  <Money cents={p.priceCents} />
                </span>
                <small>{tx("shop.sellsBack", { amount: money(p.sellBackCents) })}</small>
                <PriceTrend history={p.history} />
              </button>
            ))}
            {/* The chosen kind's box holds a box topper: how far the next one is. */}
            {item.boxProgress && <BoxGauge progress={item.boxProgress} />}
            <BoosterNote setCode={item.setCode} kind={item.kind} />
            </div>
            <div className="buy-bar">
              <div className="buy-row">
                <span className="stepper">
                  <button onClick={() => setQuantity((q) => Math.max(1, q - 1))} disabled={quantity <= 1} aria-label="−">
                    −
                  </button>
                  <input
                    type="number"
                    min={1}
                    max={MAX_QUANTITY}
                    value={quantity}
                    onChange={(e) => setQuantity(Math.min(MAX_QUANTITY, Math.max(1, Math.floor(Number(e.target.value) || 1))))}
                  />
                  <button onClick={() => setQuantity((q) => Math.min(MAX_QUANTITY, q + 1))} disabled={quantity >= MAX_QUANTITY} aria-label="+">
                    +
                  </button>
                </span>
                <label className="tick">
                  <input type="checkbox" checked={openNow} onChange={(e) => setOpenNow(e.target.checked)} />
                  {t("shop.openNow")}
                </label>
              </div>
              <button className="big" disabled={busy || !affordable} onClick={buy}>
                {tx("shop.buyFor", { amount: money(total) })}
              </button>
            </div>
          </>
        ) : (
          <p className="hint">{t("shop.noMatch")}</p>
        )}
      </section>
    </div>
  );
}
