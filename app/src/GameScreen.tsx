import { useCallback, useEffect, useState } from "react";
import {
  buyBoosters,
  claimCharity,
  createSavePoint,
  getShop,
  getWallet,
  listSets,
  openBooster,
  openBoosters,
  sellBoosters,
  takeUnlocks,
  type AchievementView,
  type OpenedCard,
  type SaveInfo,
  type SetSummary,
  type ShopItem,
  type WalletView,
} from "./api";
import { AchievementCard } from "./AchievementCard";
import { Achievements } from "./Achievements";
import { BatchOpening } from "./BatchOpening";
import { BoosterOpening } from "./BoosterOpening";
import { Boosterium } from "./Boosterium";
import { Collection } from "./Collection";
import { DataStatusBar, useDataStatus } from "./DataStatus";
import { FOOTER_NOTICE } from "./legal";
import { Loading } from "./Loading";
import { CurrencyContext, useMoney } from "./money";
import { SealedBoosters } from "./SealedBoosters";
import { Stats } from "./Stats";

type Tab = "boosterium" | "open" | "collection" | "achievements" | "stats";

const TAB_LABELS: Record<Tab, string> = {
  boosterium: "Boosterium",
  open: "Open boosters",
  collection: "Collection",
  achievements: "Achievements",
  stats: "Stats",
};

/** How long a notification stays on screen. */
const TOAST_MS = 8000;

function WalletBar({ wallet }: { wallet: WalletView }) {
  const money = useMoney();
  return (
    <div className="wallet" title="Your balance">
      {money(wallet.balanceCents)}
    </div>
  );
}

const savePointName = (game: string) =>
  `${game} — ${new Date().toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}`;

/** The game being played: Boosterium, openings, collection, achievements and stats. */
export function GameScreen({ save, onMenu }: { save: SaveInfo; onMenu: () => void }) {
  const [notice, setNotice] = useState<string | null>(null);
  const [sets, setSets] = useState<SetSummary[]>([]);
  const [shop, setShop] = useState<ShopItem[]>([]);
  const [wallet, setWallet] = useState<WalletView | null>(null);
  const [cards, setCards] = useState<OpenedCard[] | null>(null);
  const [batch, setBatch] = useState<OpenedCard[][] | null>(null);
  // The booster kind shown in the opening view, to offer the next one.
  const [last, setLast] = useState<{ setCode: string; kind: string } | null>(null);
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

  const loadAll = useCallback(() => {
    Promise.all([listSets(), getShop(), getWallet()])
      .then(([sets, shop, wallet]) => {
        setSets(sets);
        setShop(shop);
        setWallet(wallet);
        setOpenings((n) => n + 1);
      })
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    loadAll();
  }, [loadAll]);

  // The backend refreshes the card data in the background; reload everything once it is done.
  const dataStatus = useDataStatus(loadAll);

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

  async function openMany(setCode: string, kind: string, quantity: number) {
    setCards(null);
    setBatch(await openBoosters(setCode, kind, quantity));
    setLast({ setCode, kind });
    setOpenings((n) => n + 1);
    setTab("open");
  }

  async function open(setCode: string, kind: string) {
    setBatch(null);
    setCards(await openBooster(setCode, kind));
    setLast({ setCode, kind });
    setOpenings((n) => n + 1);
    setTab("open");
  }

  /** Leaves the opening view: an opening is shown once, never replayed. */
  function closeOpening() {
    setCards(null);
    setBatch(null);
  }

  function switchTab(t: Tab) {
    if (t !== tab) closeOpening();
    setTab(t);
  }

  async function saveNow() {
    try {
      const point = await createSavePoint(savePointName(save.name));
      setNotice(`Save point “${point.name}” created. Load it any time from the menu.`);
    } catch (e) {
      setError(String(e));
    }
  }

  const remaining = last && wallet?.sealed.find((s) => s.setCode === last.setCode && s.kind === last.kind);

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
          <span className="game-name" title="The game being played (saved automatically)">
            {save.name}
          </span>
          {wallet && <WalletBar wallet={wallet} />}
          <button onClick={() => void saveNow()} title="Every action is saved automatically. This keeps a copy to come back to.">
            Save point
          </button>
          <button onClick={onMenu}>☰ Menu</button>
        </header>

        <nav className="tabs">
          {(Object.keys(TAB_LABELS) as Tab[]).map((t) => (
            <button key={t} className={tab === t ? "active" : ""} onClick={() => switchTab(t)}>
              {TAB_LABELS[t]}
              {t === "open" && wallet && wallet.sealed.length > 0 && (
                <span className="tab-count">{wallet.sealed.reduce((n, s) => n + s.quantity, 0)}</span>
              )}
            </button>
          ))}
        </nav>

        <DataStatusBar status={dataStatus} />
        {error && <p className="error">{error}</p>}
        {notice && <p className="notice">{notice}</p>}
        {!wallet && !error && <Loading label="Loading your game…" />}
        {wallet &&
          !error &&
          sets.length === 0 &&
          dataStatus.state !== "updating" &&
          dataStatus.state !== "checking" && (
            <p className="hint">No card data yet. It is downloaded automatically when the app starts with a connection.</p>
          )}

        {tab === "boosterium" && wallet && (
          <Boosterium
            shop={shop}
            sets={sets}
            wallet={wallet}
            onBuy={buy}
            onBuyAndOpen={buyAndOpen}
            onClaimCharity={() => run(() => claimCharity())}
            busy={busy}
          />
        )}

        {tab === "open" && wallet && (
          <section>
            {cards || batch ? (
              <>
                <div className="controls">
                  <button onClick={closeOpening}>← Back to my boosters</button>
                  {remaining && (
                    <button
                      className="primary"
                      disabled={busy}
                      onClick={() => run(() => open(remaining.setCode, remaining.kind))}
                    >
                      Open the next one ({remaining.quantity} left)
                    </button>
                  )}
                </div>
                {cards && <BoosterOpening cards={cards} />}
                {batch && <BatchOpening boosters={batch} />}
              </>
            ) : (
              <SealedBoosters
                sealed={wallet.sealed}
                sets={sets}
                busy={busy}
                onOpen={(s) => run(() => open(s.setCode, s.kind))}
                onOpenMany={(s, n) => run(() => openMany(s.setCode, s.kind, n))}
                onSell={(s) =>
                  run(async () => {
                    const sale = await sellBoosters(s.setCode, s.kind, 1);
                    setNotice(
                      `Sold 1 ${s.setCode} booster for ${money(sale.totalCents)} (${
                        sale.profitCents >= 0 ? "profit" : "loss"
                      } ${money(Math.abs(sale.profitCents))}).`,
                    );
                  })
                }
                onGoShopping={() => switchTab("boosterium")}
              />
            )}
          </section>
        )}

        {tab === "collection" && <Collection sets={sets} refreshKey={openings} onChanged={refreshWallet} />}

        {tab === "achievements" && <Achievements refreshKey={unlockCount} />}

        {tab === "stats" && <Stats refreshKey={`${openings}-${wallet?.balanceCents}`} />}

        <footer>{FOOTER_NOTICE}</footer>

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
