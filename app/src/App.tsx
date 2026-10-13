import { useCallback, useEffect, useState } from "react";
import {
  buyBoosters,
  claimCharity,
  getShop,
  getWallet,
  listSets,
  openBooster,
  type OpenedCard,
  type SetSummary,
  type ShopItem,
  type WalletView,
} from "./api";
import { BoosterOpening } from "./BoosterOpening";
import { Boosterium } from "./Boosterium";
import { Collection } from "./Collection";
import { CurrencyContext, useMoney } from "./money";

type Tab = "boosterium" | "open" | "collection";

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

export function App() {
  const [sets, setSets] = useState<SetSummary[]>([]);
  const [shop, setShop] = useState<ShopItem[]>([]);
  const [wallet, setWallet] = useState<WalletView | null>(null);
  const [cards, setCards] = useState<OpenedCard[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<Tab>("boosterium");
  // Bumped after each opening so the collection view reloads.
  const [openings, setOpenings] = useState(0);

  const refreshWallet = useCallback(async () => {
    try {
      setWallet(await getWallet());
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
    <CurrencyContext.Provider value={wallet?.currency.symbol ?? "$"}>
      <div className="app">
        <header>
          <h1>Unsealed</h1>
          <p className="tagline">Crack &amp; Collect</p>
          {wallet && <WalletBar wallet={wallet} />}
        </header>

        <nav className="tabs">
          {(["boosterium", "open", "collection"] as Tab[]).map((t) => (
            <button key={t} className={tab === t ? "active" : ""} onClick={() => setTab(t)}>
              {t === "boosterium" ? "Boosterium" : t === "open" ? "Open boosters" : "Collection"}
              {t === "open" && wallet && wallet.sealed.length > 0 && (
                <span className="tab-count">{wallet.sealed.reduce((n, s) => n + s.quantity, 0)}</span>
              )}
            </button>
          ))}
        </nav>

        {error && <p className="error">{error}</p>}
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
                <div key={`${s.setCode}/${s.kind}`} className="inventory-row">
                  <span>
                    {s.setName} · {s.kind} booster
                  </span>
                  <strong>×{s.quantity}</strong>
                  <button className="primary" disabled={busy} onClick={() => run(() => open(s.setCode, s.kind))}>
                    Open one
                  </button>
                </div>
              ))}
            </div>
            {cards && <BoosterOpening cards={cards} />}
          </section>
        )}

        {tab === "collection" && <Collection sets={sets} refreshKey={openings} onChanged={refreshWallet} />}

        <footer>{DISCLAIMER}</footer>
      </div>
    </CurrencyContext.Provider>
  );
}
