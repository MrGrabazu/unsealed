import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import {
  buyBoosters,
  checkSetImages,
  claimCharity,
  createSavePoint,
  collectIncome,
  getKeptBets,
  getShop,
  getWallet,
  listSets,
  openBooster,
  openBoosters,
  revealCards,
  sellBoosters,
  setKeptBets as saveKeptBets,
  takeUnlocks,
  type AchievementView,
  type IncomeView,
  type SkippedBet,
  type KeptBet,
  type BetRequest,
  type OpenedCard,
  type SettledBet,
  type SaveInfo,
  type SetSummary,
  type ShopItem,
  type WalletView,
} from "./api";
import { AchievementCard } from "./AchievementCard";
import { Achievements } from "./Achievements";
import { BatchOpening } from "./BatchOpening";
import { BetResults } from "./BetResults";
import { activeBets, keptFor, namedCard } from "./bets";
import { BetSlip } from "./BetSlip";
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
import { CurrencyContext, DEFAULT_CURRENCY, useMoney, useMoneyText } from "./money";
import { SealedBoosters } from "./SealedBoosters";
import { Stats } from "./Stats";

type Tab = "boosterium" | "open" | "collection" | "achievements" | "stats" | "history";

const TABS: Tab[] = ["boosterium", "open", "collection", "achievements", "stats", "history"];

/** How long a notification stays on screen. */
const TOAST_MS = 8000;

/** The collection's income is paid about each time a whole Seal is earned, within these bounds. */
const INCOME_MIN_MS = 1_000;
const INCOME_MAX_MS = 60_000;
/** How often the income also refreshes the whole wallet (and shows the achievements it unlocked). */
const INCOME_REFRESH_MS = 60_000;
/** How long a "+N" floats up from the balance. */
const INCOME_FLOAT_MS = 1_400;

interface IncomeFloat {
  id: number;
  cents: number;
}

function WalletBar({ wallet, income, floats }: { wallet: WalletView; income: IncomeView | null; floats: IncomeFloat[] }) {
  const money = useMoney();
  const moneyText = useMoneyText();
  const { t, formatNumber } = useI18n();
  const toSeals = (cents: number) => Math.round(cents * wallet.currency.sealsPerCent);
  const perMinute = income ? income.perMinuteCents * wallet.currency.sealsPerCent : 0;
  return (
    <>
      <div className="wallet" title={t("game.balanceTitle")}>
        {/* Keyed on the amount: it pulses each time it changes. */}
        <span key={wallet.balanceCents} className="wallet-amount">
          {money(wallet.balanceCents)}
        </span>
        {floats.map((f) => (
          <span key={f.id} className="income-float" aria-hidden="true">
            +{formatNumber(toSeals(f.cents))}
          </span>
        ))}
      </div>
      {income && (
        <span
          className="income-rate"
          title={t("income.rateTitle", {
            value: moneyText(income.collectionValueCents),
            percent: formatNumber(income.percentPerMinute),
            hours: formatNumber(income.offlineCapMinutes / 60),
          })}
        >
          {/* Whole Seals only: there is no such thing as a fraction of one. */}
          {t("income.rate", { rate: formatNumber(Math.ceil(perMinute)) })}
        </span>
      )}
    </>
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
  // What the collection earns, and what it earned while the player was away.
  const [income, setIncome] = useState<IncomeView | null>(null);
  const [awayIncome, setAwayIncome] = useState<IncomeView | null>(null);
  // The "+N" floating up from the balance, one per payment.
  const [incomeFloats, setIncomeFloats] = useState<IncomeFloat[]>([]);
  const floatId = useRef(0);
  // A set to show open in the Boosterium (asked for from its collection).
  const [shopFocus, setShopFocus] = useState<{ setCode: string; at: number } | null>(null);
  // Bumped after each opening so the collection view reloads.
  const [openings, setOpenings] = useState(0);
  // Bets: those kept from one opening to the next, those on the pack about to be torn, and
  // what the last opening settled (shown once every card is seen).
  const [kept, setKept] = useState<KeptBet[]>([]);
  const [slip, setSlip] = useState<BetRequest[]>([]);
  const [betResults, setBetResults] = useState<SettledBet[]>([]);
  // Bets left out of the last opening (no stake taken), and why.
  const [betSkipped, setBetSkipped] = useState<SkippedBet[]>([]);
  const [allSeen, setAllSeen] = useState(false);
  // Names of the cards bet on (the results name them).
  const betCardNames = useRef(new Map<string, string>());

  useEffect(() => {
    getKeptBets()
      .then(setKept)
      .catch(() => {});
  }, []);

  function changeKept(next: KeptBet[]) {
    setKept(next);
    saveKeptBets(next).catch((e) => setError(String(e)));
  }

  const betCardName = (uuid: string) =>
    betCardNames.current.get(uuid) ??
    kept.find((b) => namedCard(b.bet) === uuid)?.cardName ??
    cards?.find((c) => c.uuid === uuid)?.name ??
    "?";

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

  // The Seals per cent, for the income's pace (read from a ref: it must not restart the timer).
  const sealsPerCent = useRef(currency.sealsPerCent);
  sealsPerCent.current = currency.sealsPerCent;
  // When the wallet was last fully refreshed by the income (the achievements it unlocked with it).
  const incomeRefreshedAt = useRef(0);

  /** Pays what the collection earned since the last time: a "+N" floats up from the balance,
   * which grows at once. After a while away, a banner says how much it earned. */
  const payIncome = useCallback(
    async (onArrival: boolean): Promise<IncomeView | null> => {
      try {
        const paid = await collectIncome();
        setIncome(paid);
        if (onArrival && paid.earnedCents > 0 && paid.minutes >= 5) setAwayIncome(paid);
        if (paid.earnedCents > 0) {
          const id = ++floatId.current;
          setIncomeFloats((current) => [...current, { id, cents: paid.earnedCents }]);
          setTimeout(() => setIncomeFloats((current) => current.filter((f) => f.id !== id)), INCOME_FLOAT_MS);
          // The balance follows at once; the full refresh (and its achievements) once a minute.
          if (Date.now() - incomeRefreshedAt.current >= INCOME_REFRESH_MS) {
            incomeRefreshedAt.current = Date.now();
            await refreshWallet();
          } else {
            setWallet((w) => w && { ...w, balanceCents: w.balanceCents + paid.earnedCents });
          }
        }
        return paid;
      } catch (e) {
        setError(String(e));
        return null;
      }
    },
    [refreshWallet],
  );

  // Paid about each time a whole Seal is earned (at most every second, at least every minute),
  // so that it can be seen coming in; on arrival, for the time away.
  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const tick = async (onArrival: boolean) => {
      const paid = await payIncome(onArrival);
      if (stopped) return;
      const perMinute = paid ? paid.perMinuteCents * sealsPerCent.current : 0;
      const delay = perMinute > 0 ? Math.min(Math.max(60_000 / perMinute, INCOME_MIN_MS), INCOME_MAX_MS) : INCOME_MAX_MS;
      timer = setTimeout(() => void tick(false), delay);
    };
    void tick(true);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [payIncome]);

  /** Cards of the pack were turned over: their achievements show up now. */
  async function reveal(count: number) {
    // Every card is seen: the bets' results (and their winnings) show up.
    if (cards && count >= cards.length) setAllSeen(true);
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

  /** Forgets the bets of the last opening, for a new one. */
  function resetBets() {
    setBetResults([]);
    setBetSkipped([]);
    setAllSeen(false);
  }

  async function openMany(setCode: string, kind: string, quantity: number) {
    holdingToasts.current = true;
    setPack(null);
    setCards(null);
    resetBets();
    const opened = await openBoosters(setCode, kind, quantity, activeBets(keptFor(kept, setCode)));
    setBatch(opened.map((b) => b.cards));
    setBetResults(opened.flatMap((b) => b.bets));
    // The same bets on every booster: those left out of any booster, once each.
    const skipped = new Map(opened.flatMap((b) => b.skipped).map((s) => [`${JSON.stringify(s.bet)}:${s.reason}`, s]));
    setBetSkipped([...skipped.values()]);
    setLast({ setCode, kind });
    setOpenings((n) => n + 1);
    setTab("open");
  }

  /** Shows the sealed pack. Nothing is opened (or used up) until the player tears it. */
  function open(setCode: string, kind: string) {
    setBatch(null);
    setCards(null);
    resetBets();
    // The kept bets come with every booster (the player can still change them); the others
    // were for the last one.
    setSlip(keptFor(kept, setCode));
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
    resetBets();
    setPack({ setCode, kind, quick: true });
    setLast({ setCode, kind });
    setTab("open");
    setBusy(true);
    setError(null);
    try {
      await checkSetImages(setCode).catch(() => {});
      const opened = await openBooster(setCode, kind, false, activeBets(keptFor(kept, setCode)));
      setCards(opened.cards);
      setBetResults(opened.bets);
      setBetSkipped(opened.skipped);
      setOpenings((n) => n + 1);
      await revealCards(Number.MAX_SAFE_INTEGER);
      setAllSeen(true);
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
      const opened = await openBooster(pack.setCode, pack.kind, true, activeBets(slip));
      setCards(opened.cards);
      setBetResults(opened.bets);
      setBetSkipped(opened.skipped);
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
    resetBets();
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
          {wallet && <WalletBar wallet={wallet} income={income} floats={incomeFloats} />}
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

        {awayIncome && (
          <div className="income-away">
            <p>
              {t("income.away", {
                amount: money(awayIncome.earnedCents),
                time:
                  awayIncome.minutes < 90
                    ? i18n.plural("income.minutes", Math.round(awayIncome.minutes))
                    : i18n.plural("income.hours", Math.round(awayIncome.minutes / 60)),
              })}
            </p>
            <button onClick={() => setAwayIncome(null)}>{t("income.dismiss")}</button>
          </div>
        )}

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
                {/* Bets on the sealed pack, until it is torn. */}
                {pack && !pack.quick && !cards && (
                  <BetSlip
                    key={`${pack.setCode}/${pack.kind}`}
                    setCode={pack.setCode}
                    kind={pack.kind}
                    bets={slip}
                    onBets={setSlip}
                    kept={kept}
                    onKept={changeKept}
                    onCardName={(uuid, name) => betCardNames.current.set(uuid, name)}
                    balanceCents={wallet.balanceCents}
                  />
                )}
                {allSeen && <BetResults bets={betResults} skipped={betSkipped} cardName={betCardName} />}
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
                {batch && (
                  <BatchOpening
                    boosters={batch}
                    onFinished={() => {
                      releaseToasts();
                      setAllSeen(true);
                    }}
                  />
                )}
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
