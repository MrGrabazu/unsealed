import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import {
  buyBoosters,
  checkSetImages,
  claimCharity,
  createSavePoint,
  getShop,
  getWallet,
  listSets,
  openBooster,
  openBoosters,
  revealCards,
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
import { CardDetails } from "./CardDetails";
import { Logo } from "./Logo";
import { getSoundOn, setSoundOn, sounds } from "./sound";
import { PackOpening } from "./PackOpening";
import { QuickOpening } from "./QuickOpening";
import { canUseWebGL, getOpening3d } from "./settings";

// The 3D opening brings three.js along: loaded the first time a booster is opened.
const PackOpening3D = lazy(() => import("./PackOpening3D").then((m) => ({ default: m.PackOpening3D })));
import { Boosterium } from "./Boosterium";
import { Collection } from "./Collection";
import { rewardText } from "./boosterKinds";
import { DataStatusBar, useDataStatus } from "./DataStatus";
import { Footer } from "./Footer";
import { History } from "./History";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { CurrencyContext, DEFAULT_CURRENCY, useMoney } from "./money";
import { SealedBoosters } from "./SealedBoosters";
import { Stats } from "./Stats";

type Tab = "boosterium" | "open" | "collection" | "achievements" | "stats" | "history";

const TABS: Tab[] = ["boosterium", "open", "collection", "achievements", "stats", "history"];

/** How long a notification stays on screen. */
const TOAST_MS = 8000;

function WalletBar({ wallet }: { wallet: WalletView }) {
  const money = useMoney();
  const { t } = useI18n();
  return (
    <div className="wallet" title={t("game.balanceTitle")}>
      {money(wallet.balanceCents)}
    </div>
  );
}

/** The game being played: Boosterium, openings, collection, achievements and stats. */
export function GameScreen({ save, onMenu }: { save: SaveInfo; onMenu: () => void }) {
  const i18n = useI18n();
  const { t, lang } = i18n;
  const [notice, setNotice] = useState<string | null>(null);
  const [sets, setSets] = useState<SetSummary[]>([]);
  const [shop, setShop] = useState<ShopItem[]>([]);
  const [wallet, setWallet] = useState<WalletView | null>(null);
  const [cards, setCards] = useState<OpenedCard[] | null>(null);
  const [batch, setBatch] = useState<OpenedCard[][] | null>(null);
  // The pack being opened: shown sealed until torn (then `cards` holds what it gave).
  const [pack, setPack] = useState<{ setCode: string; kind: string; quick: boolean } | null>(null);
  // The card whose details are open.
  const [detailsOf, setDetailsOf] = useState<{ setCode: string; uuid: string } | null>(null);
  // The 3D opening, unless turned off in the options or impossible on this machine.
  const [opening3d] = useState(() => getOpening3d() && canUseWebGL());
  const [soundOn, setSoundOnState] = useState(getSoundOn);
  // The booster kind shown in the opening view, to offer the next one.
  const [last, setLast] = useState<{ setCode: string; kind: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<Tab>("boosterium");
  // A set to show open in the Boosterium (asked for from its collection).
  const [shopFocus, setShopFocus] = useState<{ setCode: string; at: number } | null>(null);
  // Bumped after each opening so the collection view reloads.
  const [openings, setOpenings] = useState(0);

  // This component renders the currency provider, so it cannot use `useMoneyText` itself.
  const currency = wallet?.currency ?? DEFAULT_CURRENCY;
  const money = (cents: number) => i18n.plural("currency.amount", Math.round(cents * currency.sealsPerCent));

  const [toasts, setToasts] = useState<AchievementView[]>([]);
  // Bumped when achievements unlock so the achievements tab reloads.
  const [unlockCount, setUnlockCount] = useState(0);

  // During a fast batch opening, unlocked achievements wait for its summary (no spoilers).
  const holdingToasts = useRef(false);
  const heldToasts = useRef<AchievementView[]>([]);

  const showToasts = useCallback((unlocked: AchievementView[]) => {
    if (unlocked.length === 0) return;
    sounds.seal();
    setToasts((current) => [...current, ...unlocked]);
    setUnlockCount((n) => n + unlocked.length);
    for (const a of unlocked) {
      setTimeout(() => setToasts((current) => current.filter((t) => t.id !== a.id)), TOAST_MS);
    }
  }, []);

  const releaseToasts = useCallback(() => {
    holdingToasts.current = false;
    showToasts(heldToasts.current.splice(0));
  }, [showToasts]);

  /** Refreshes the wallet and shows the achievements unlocked by the last action. */
  const refreshWallet = useCallback(async () => {
    try {
      setWallet(await getWallet());
      const unlocked = await takeUnlocks(lang);
      if (holdingToasts.current) heldToasts.current.push(...unlocked);
      else showToasts(unlocked);
    } catch (e) {
      setError(String(e));
    }
  }, [lang, showToasts]);

  /** Cards of the pack were turned over: their achievements show up now. */
  async function reveal(count: number) {
    try {
      await revealCards(count);
      await refreshWallet();
    } catch (e) {
      setError(String(e));
    }
  }

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
    holdingToasts.current = true;
    setPack(null);
    setCards(null);
    setBatch(await openBoosters(setCode, kind, quantity));
    setLast({ setCode, kind });
    setOpenings((n) => n + 1);
    setTab("open");
  }

  /** Shows the sealed pack. Nothing is opened (or used up) until the player tears it. */
  function open(setCode: string, kind: string) {
    setBatch(null);
    setCards(null);
    setPack({ setCode, kind, quick: false });
    setLast({ setCode, kind });
    setTab("open");
    // While the pack is sealed: which translated images are good scans (else English ones).
    void checkSetImages(setCode).catch(() => {});
  }

  /** The quick opening: drawn at once, its cards dealt without the animation (and seen at
   * once, achievements included). */
  async function openQuick(setCode: string, kind: string) {
    setBatch(null);
    setCards(null);
    setPack({ setCode, kind, quick: true });
    setLast({ setCode, kind });
    setTab("open");
    setBusy(true);
    setError(null);
    try {
      await checkSetImages(setCode).catch(() => {});
      setCards(await openBooster(setCode, kind, false));
      setOpenings((n) => n + 1);
      await revealCards(Number.MAX_SAFE_INTEGER);
    } catch (e) {
      setError(String(e));
      setPack(null);
    } finally {
      setBusy(false);
      await refreshWallet();
    }
  }

  /** The pack is torn open: the booster is drawn now. False when it failed (the pack
   * then shows sealed again, with the error). */
  async function tear(): Promise<boolean> {
    if (!pack) return false;
    setBusy(true);
    setError(null);
    try {
      setCards(await openBooster(pack.setCode, pack.kind, true));
      setOpenings((n) => n + 1);
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setBusy(false);
      await refreshWallet();
    }
  }

  /** Leaves the opening view: an opening is shown once, never replayed. */
  function closeOpening() {
    // Leaving before every card is seen: what they unlock shows up now.
    if (cards) void reveal(Number.MAX_SAFE_INTEGER);
    releaseToasts();
    setDetailsOf(null);
    setPack(null);
    setCards(null);
    setBatch(null);
  }

  function switchTab(t: Tab) {
    if (t !== tab) closeOpening();
    setTab(t);
  }

  async function saveNow() {
    try {
      const name = t("game.savePointName", { game: save.name, date: i18n.formatDate(Date.now() / 1000) });
      const point = await createSavePoint(name);
      setNotice(t("game.savePointCreated", { name: point.name }));
    } catch (e) {
      setError(String(e));
    }
  }

  const remaining = last && wallet?.sealed.find((s) => s.setCode === last.setCode && s.kind === last.kind);

  const buy = (item: ShopItem, quantity: number) =>
    run(async () => {
      await buyBoosters(item.setCode, item.kind, quantity);
      sounds.coins();
    });
  const buyAndOpen = (item: ShopItem) =>
    run(async () => {
      await buyBoosters(item.setCode, item.kind, 1);
      sounds.coins();
      open(item.setCode, item.kind);
    });

  return (
    <CurrencyContext.Provider value={currency}>
      <div className="app">
        <header>
          <h1 className="header-logo">
            <Logo />
          </h1>
          <span className="game-name" title={t("game.nameTitle")}>
            {save.name}
          </span>
          {wallet && <WalletBar wallet={wallet} />}
          {/* Sound on or off, at hand (also in the options, with the volume). */}
          <button
            className="sound-toggle"
            onClick={() => {
              setSoundOn(!soundOn);
              setSoundOnState(!soundOn);
              if (!soundOn) sounds.click();
            }}
            title={t(soundOn ? "game.soundOn" : "game.soundOff")}
            aria-label={t(soundOn ? "game.soundOn" : "game.soundOff")}
          >
            {soundOn ? "🔊" : "🔇"}
          </button>
          <button onClick={() => void saveNow()} title={t("game.savePointTitle")}>
            {t("game.savePoint")}
          </button>
          <button onClick={onMenu}>{t("game.menu")}</button>
        </header>

        <nav className="tabs">
          {TABS.map((each) => (
            <button key={each} className={tab === each ? "active" : ""} onClick={() => switchTab(each)}>
              {t(`tab.${each}`)}
              {each === "open" && wallet && wallet.sealed.length > 0 && (
                <span className="tab-count">{wallet.sealed.reduce((n, s) => n + s.quantity, 0)}</span>
              )}
            </button>
          ))}
        </nav>

        <DataStatusBar status={dataStatus} />
        {error && <p className="error">{error}</p>}
        {notice && <p className="notice">{notice}</p>}
        {!wallet && !error && <Loading label={t("game.loading")} />}
        {wallet &&
          !error &&
          sets.length === 0 &&
          dataStatus.state !== "updating" &&
          dataStatus.state !== "checking" && <p className="hint">{t("data.none")}</p>}

        {tab === "boosterium" && wallet && (
          <Boosterium
            shop={shop}
            sets={sets}
            wallet={wallet}
            onBuy={buy}
            onBuyAndOpen={buyAndOpen}
            onClaimCharity={() => run(() => claimCharity())}
            busy={busy}
            focus={shopFocus}
          />
        )}

        {tab === "open" && wallet && (
          <section>
            {pack || batch ? (
              <>
                <div className="controls">
                  <button onClick={closeOpening}>{t("open.back")}</button>
                  {/* Once this pack is open: the next one of the same kind, if any is left. */}
                  {remaining && (cards || batch) && (
                    <button
                      className="primary"
                      disabled={busy}
                      onClick={() =>
                        pack?.quick ? void openQuick(remaining.setCode, remaining.kind) : open(remaining.setCode, remaining.kind)
                      }
                    >
                      {t("open.next", { count: remaining.quantity })}
                    </button>
                  )}
                  {/* The quick way, for the impatient, right from an opening. */}
                  {remaining && (cards || batch) && !pack?.quick && (
                    <button disabled={busy} onClick={() => void openQuick(remaining.setCode, remaining.kind)}>
                      {t("open.quick")}
                    </button>
                  )}
                </div>
                {pack?.quick && (
                  <QuickOpening
                    set={sets.find((s) => s.code === pack.setCode)}
                    kind={pack.kind}
                    cards={cards}
                    onDetails={(card) => setDetailsOf({ setCode: pack.setCode, uuid: card.uuid })}
                    onSold={refreshWallet}
                  />
                )}
                {pack && !pack.quick && opening3d && (
                  <Suspense fallback={<Loading />}>
                    <PackOpening3D
                      setCode={pack.setCode}
                      set={sets.find((s) => s.code === pack.setCode)}
                      kind={pack.kind}
                      cards={cards}
                      onTear={tear}
                      onReveal={(count) => void reveal(count)}
                      onDetails={(card) => setDetailsOf({ setCode: pack.setCode, uuid: card.uuid })}
                      onSold={refreshWallet}
                    />
                  </Suspense>
                )}
                {pack && !pack.quick && !opening3d && (
                  <PackOpening
                    setCode={pack.setCode}
                    set={sets.find((s) => s.code === pack.setCode)}
                    kind={pack.kind}
                    cards={cards}
                    onTear={tear}
                    onReveal={(count) => void reveal(count)}
                    busy={busy}
                    // The details live in the booster's set file (it holds cards from other sets too).
                    onDetails={(card) => setDetailsOf({ setCode: pack.setCode, uuid: card.uuid })}
                    onSold={refreshWallet}
                  />
                )}
                {batch && <BatchOpening boosters={batch} onFinished={releaseToasts} />}
                {detailsOf && (
                  <CardDetails setCode={detailsOf.setCode} uuid={detailsOf.uuid} onClose={() => setDetailsOf(null)} />
                )}
              </>
            ) : (
              <SealedBoosters
                sealed={wallet.sealed}
                sets={sets}
                busy={busy}
                onOpen={(s) => open(s.setCode, s.kind)}
                onOpenQuick={(s) => void openQuick(s.setCode, s.kind)}
                onOpenMany={(s, n) => run(() => openMany(s.setCode, s.kind, n))}
                onSell={(s) =>
                  run(async () => {
                    const sale = await sellBoosters(s.setCode, s.kind, 1);
                    sounds.coins();
                    const amount = money(sale.totalCents);
                    setNotice(
                      sale.profitCents >= 0
                        ? t("open.soldBooster", { set: s.setCode, amount, profit: money(sale.profitCents) })
                        : t("open.soldBoosterLoss", { set: s.setCode, amount, loss: money(-sale.profitCents) }),
                    );
                  })
                }
                onGoShopping={() => switchTab("boosterium")}
              />
            )}
          </section>
        )}

        {tab === "collection" && (
          <Collection
            sets={sets}
            refreshKey={openings}
            onChanged={refreshWallet}
            onShop={(setCode) => {
              setShopFocus({ setCode, at: Date.now() });
              switchTab("boosterium");
            }}
          />
        )}

        {tab === "achievements" && <Achievements refreshKey={unlockCount} />}

        {tab === "stats" && <Stats refreshKey={`${openings}-${wallet?.balanceCents}`} />}

        {tab === "history" && (
          <History
            onChanged={() => {
              setOpenings((n) => n + 1);
              void refreshWallet();
            }}
          />
        )}

        <Footer />

        <div className="toasts" aria-live="polite">
          {toasts.map((a) => (
            <button
              key={a.id}
              className="toast"
              onClick={() => setToasts((current) => current.filter((other) => other.id !== a.id))}
            >
              <div className="toast-card">
                <AchievementCard item={a} />
              </div>
              <div className="toast-text">
                <strong>{t("achievement.unlocked")}</strong>
                <span>{a.name}</span>
                {a.reward && (
                  <span className="gain">
                    {t("achievement.reward", { reward: rewardText(i18n, money, a.reward) })}
                  </span>
                )}
              </div>
            </button>
          ))}
        </div>
      </div>
    </CurrencyContext.Provider>
  );
}
