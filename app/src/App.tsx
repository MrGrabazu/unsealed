import { useCallback, useEffect, useState } from "react";
import {
  buyBoosters,
  claimCharity,
  getShop,
  getWallet,
  listSets,
  openBooster,
  sellBoosters,
  takeUnlocks,
  type AchievementView,
  type OpenedCard,
  type SealedStack,
  type SetSummary,
  type ShopItem,
  type WalletView,
} from "./api";
import { AchievementCard } from "./AchievementCard";
import { Achievements } from "./Achievements";
import { BoosterOpening } from "./BoosterOpening";
import { Boosterium } from "./Boosterium";
import { Collection } from "./Collection";
import { CurrencyContext, useMoney } from "./money";

type Tab = "boosterium" | "open" | "collection" | "achievements";

const TAB_LABELS: Record<Tab, string> = {
  boosterium: "Boosterium",
  open: "Open boosters",
  collection: "Collection",
  achievements: "Achievements",
};

/** How long a notification stays on screen. */
const TOAST_MS = 8000;

// Fan Content Policy notice (AGENTS.md, section 2). Check the wording against the current policy.
const DISCLAIMER =
  "Unsealed is unofficial Fan Content permitted under the Fan Content Policy. " +
  "Not approved/endorsed by Wizards. Portions of the materials used are property of " +
  "Wizards of the Coast. ©Wizards of the Coast LLC. Card images and prices are provided by Scryfall.";

function WalletBar({ wallet }: { wallet: WalletView }) {
  const money = useMoney();
  return (
    <div className="wallet" title="Your balance">
      {money(wallet.balanceCents)}
    </div>
  );
}

function SealedRow(props: { stack: SealedStack; busy: boolean; onOpen: () => void; onSell: () => void }) {
  const money = useMoney();
  const { stack: s, busy } = props;
  // Profit if the whole stack were sold back today.
  const pnl = s.quantity * s.sellValueCents - s.costCents;
  return (
    <div className="inventory-row">
      <span>
        {s.setName} · {s.kind} booster
        <span className="hint">
          {" "}
          paid {money(s.costCents)} · worth {money(s.quantity * s.sellValueCents)} if sold ·{" "}
          <span className={pnl >= 0 ? "gain" : "loss"}>
            {pnl >= 0 ? "+" : "−"}
            {money(Math.abs(pnl))}
          </span>
        </span>
      </span>
      <strong>×{s.quantity}</strong>
      <button disabled={busy} onClick={props.onSell}>
        Sell one ({money(s.sellValueCents)})
      </button>
      <button className="primary" disabled={busy} onClick={props.onOpen}>
        Open one
      </button>
    </div>
  );
}

export function App() {
  const [notice, setNotice] = useState<string | null>(null);
  const [sets, setSets] = useState<SetSummary[]>([]);
  const [shop, setShop] = useState<ShopItem[]>([]);
  const [wallet, setWallet] = useState<WalletView | null>(null);
  const [cards, setCards] = useState<OpenedCard[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<Tab>("boosterium");
  // Bumped after each opening so the collection view reloads.
  const [openings, setOpenings] = useState(0);

  // This component renders the currency provider, so it cannot use `useMoney` itself.
  const symbol = wallet?.currency.symbol ?? "$";
  const money = (cents: number) => `${symbol}${(cents / 100).toFixed(2)}`;

  const [toasts, setToasts] = useState<AchievementView[]>([]);
  // Bumped when achievements unlock so the achievements tab reloads.
  const [unlockCount, setUnlockCount] = useState(0);

  /** Refreshes the wallet and shows the achievements unlocked by the last action. */
  const refreshWallet = useCallback(async () => {
    try {
      setWallet(await getWallet());
      const unlocked = await takeUnlocks();
      if (unlocked.length > 0) {
        setToasts((current) => [...current, ...unlocked]);
        setUnlockCount((n) => n + unlocked.length);
        for (const a of unlocked) {
          setTimeout(() => setToasts((current) => current.filter((t) => t.id !== a.id)), TOAST_MS);
        }
      }
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    Promise.all([listSets(), getShop(), getWallet()])
      .then(([sets, shop, wallet]) => {
        setSets(sets);
        setShop(shop);
        setWallet(wallet);
      })
      .catch((e) => setError(String(e)));
  }, []);

  /** Runs a game action with the busy flag and error handling. */
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      await refreshWallet();
    }
  }

  async function open(setCode: string, kind: string) {
    setCards(await openBooster(setCode, kind));
    setOpenings((n) => n + 1);
    setTab("open");
  }

  const buy = (item: ShopItem, quantity: number) =>
    run(() => buyBoosters(item.setCode, item.kind, quantity));
  const buyAndOpen = (item: ShopItem) =>
    run(async () => {
      await buyBoosters(item.setCode, item.kind, 1);
      await open(item.setCode, item.kind);
    });

  return (
    <CurrencyContext.Provider value={symbol}>
      <div className="app">
        <header>
          <h1>Unsealed</h1>
          <p className="tagline">Crack &amp; Collect</p>
          {wallet && <WalletBar wallet={wallet} />}
        </header>

        <nav className="tabs">
          {(Object.keys(TAB_LABELS) as Tab[]).map((t) => (
            <button key={t} className={tab === t ? "active" : ""} onClick={() => setTab(t)}>
              {TAB_LABELS[t]}
              {t === "open" && wallet && wallet.sealed.length > 0 && (
                <span className="tab-count">{wallet.sealed.reduce((n, s) => n + s.quantity, 0)}</span>
              )}
            </button>
          ))}
        </nav>

        {error && <p className="error">{error}</p>}
        {notice && <p className="notice">{notice}</p>}
        {!error && sets.length === 0 && (
          <p className="hint">
            No data found. Run <code>cargo run --release -p data-builder -- --sets BLB,DSK,FDN</code> first.
          </p>
        )}

        {tab === "boosterium" && wallet && (
          <Boosterium
            shop={shop}
            wallet={wallet}
            onBuy={buy}
            onBuyAndOpen={buyAndOpen}
            onClaimCharity={() => run(() => claimCharity())}
            busy={busy}
          />
        )}

        {tab === "open" && wallet && (
          <section>
            <div className="inventory">
              {wallet.sealed.length === 0 && <p className="hint">No sealed booster. Buy some in the Boosterium.</p>}
              {wallet.sealed.map((s) => (
                <SealedRow
                  key={`${s.setCode}/${s.kind}`}
                  stack={s}
                  busy={busy}
                  onOpen={() => run(() => open(s.setCode, s.kind))}
                  onSell={() =>
                    run(async () => {
                      const sale = await sellBoosters(s.setCode, s.kind, 1);
                      setNotice(
                        `Sold 1 ${s.setCode} booster for ${money(sale.totalCents)} (${
                          sale.profitCents >= 0 ? "profit" : "loss"
                        } ${money(Math.abs(sale.profitCents))}).`,
                      );
                    })
                  }
                />
              ))}
            </div>
            {cards && <BoosterOpening cards={cards} />}
          </section>
        )}

        {tab === "collection" && <Collection sets={sets} refreshKey={openings} onChanged={refreshWallet} />}

        {tab === "achievements" && <Achievements refreshKey={unlockCount} />}

        <footer>{DISCLAIMER}</footer>

        <div className="toasts" aria-live="polite">
          {toasts.map((a) => (
            <button
              key={a.id}
              className="toast"
              onClick={() => setToasts((current) => current.filter((t) => t.id !== a.id))}
            >
              <div className="toast-card">
                <AchievementCard item={a} />
              </div>
              <div className="toast-text">
                <strong>Achievement unlocked!</strong>
                <span>{a.name}</span>
                {a.reward && <span className="gain">Reward: {a.reward}</span>}
              </div>
            </button>
          ))}
        </div>
      </div>
    </CurrencyContext.Provider>
  );
}
