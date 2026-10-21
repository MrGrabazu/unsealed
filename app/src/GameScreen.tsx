import { lazy, Suspense, useCallback, useContext, useEffect, useRef, useState } from "react";
import {
  buyArtifact,
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
  takeGivenArtifacts,
  takeUnlocks,
  type AchievementView,
  type ArtifactView,
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
import { refreshMusic } from "./music";
import { getSoundOn, setSoundOn, sounds } from "./sound";
import { PackOpening } from "./PackOpening";
import { OpeningResult } from "./OpeningResult";
import { canUseWebGL, getOpening3d } from "./settings";

// The 3D opening brings three.js along: loaded the first time a booster is opened.
const PackOpening3D = lazy(() => import("./PackOpening3D").then((m) => ({ default: m.PackOpening3D })));
import { Boosterium, CURIOS } from "./Boosterium";
import { Collection } from "./Collection";
import { kindLabel, rewardText } from "./boosterKinds";
import { DataPreparing, DataStatusBar, useDataStatus } from "./DataStatus";
import { Ambience } from "./Ambience";
import { BackToTop } from "./BackToTop";
import { History } from "./History";
import { useI18n } from "./i18n";
import { Icon, type IconName } from "./icons";
import { Loading } from "./Loading";
import { Coin, CurrencyContext, DEFAULT_CURRENCY, useMoneyText } from "./money";
import { SealedBoosters } from "./SealedBoosters";
import { Stats } from "./Stats";
import { ArtifactReveal } from "./ArtifactReveal";
import { Curiosities } from "./Curiosities";

type Tab = "boosterium" | "open" | "collection" | "achievements" | "curiosities" | "stats" | "history";

const TABS: Tab[] = ["boosterium", "open", "collection", "achievements", "curiosities", "stats", "history"];

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

const TAB_ICONS: Record<Tab, IconName> = {
  boosterium: "shop",
  open: "pack",
  collection: "book",
  achievements: "trophy",
  curiosities: "curio",
  stats: "chart",
  history: "scroll",
};

/** The resources on the HUD's strap: the balance (with what the collection earns, and the
 * "+N" floating up from it), the sealed boosters, and what the collection is worth. */
/** What the collection earned while the game was closed: a small frame hanging under the
 * balance (it moves nothing), closed by a click or after a while. */
function AwayIncome({ away, onClose }: { away: IncomeView; onClose: () => void }) {
  const { t, plural, formatNumber } = useI18n();
  const rules = useContext(CurrencyContext);
  useEffect(() => {
    sounds.coins();
    const timer = window.setTimeout(onClose, AWAY_INCOME_MS);
    return () => window.clearTimeout(timer);
  }, []);
  const time =
    away.minutes < 90 ? plural("income.minutes", Math.round(away.minutes)) : plural("income.hours", Math.round(away.minutes / 60));
  return (
    <div className="away-pop frame" role="status" onClick={onClose}>
      <div className="cartouche">{t("income.welcome")}</div>
      <div className="away-amount">
        +{formatNumber(Math.round(away.earnedCents * rules.sealsPerCent))}
        <Coin />
      </div>
      <p>{t("income.awayShort", { time })}</p>
      <button className="big">{t("income.dismiss")}</button>
    </div>
  );
}

/** How long the frame of what was earned away stays, unless closed before. */
const AWAY_INCOME_MS = 20_000;

function Resources({
  wallet,
  income,
  floats,
  away,
  onAwayClosed,
}: {
  wallet: WalletView;
  income: IncomeView | null;
  floats: IncomeFloat[];
  away: IncomeView | null;
  onAwayClosed: () => void;
}) {
  const moneyText = useMoneyText();
  const { t, formatNumber } = useI18n();
  const toSeals = (cents: number) => Math.round(cents * wallet.currency.sealsPerCent);
  const perMinute = income ? income.perMinuteCents * wallet.currency.sealsPerCent : 0;
  const sealed = wallet.sealed.reduce((n, s) => n + s.quantity, 0);
  return (
    <div className="resources">
      {/* With the Crown of the Counter, the balance wears a golden frame. */}
      <div className={`res ${wallet.artifactPerks.crowned ? "crowned" : ""}`} title={t("game.balanceTitle")}>
        {/* The coin in its medallion before the amount, as the other resources have theirs. */}
        <span className="ico coin-ico">
          <Coin />
        </span>
        {/* Keyed on the amount: it pulses each time it changes. */}
        <span key={wallet.balanceCents} className="v wallet-amount">
          {formatNumber(toSeals(wallet.balanceCents))}
        </span>
        {income && (
          <span
            className="s"
            title={t("income.rateTitle", {
              value: moneyText(income.collectionValueCents),
              percent: formatNumber(Number(income.percentPerMinute.toPrecision(2))),
              hours: formatNumber(income.offlineCapMinutes / 60),
            }) + (income.maxPerMinuteCents == null ? "" : " " + t("income.rateMax", { max: moneyText(income.maxPerMinuteCents) }))}
          >
            {/* Whole Seals only: there is no such thing as a fraction of one. */}
            {t("income.rate", { rate: formatNumber(Math.ceil(perMinute)) })}
          </span>
        )}
        {floats.map((f) => (
          <span key={f.id} className="income-float" aria-hidden="true">
            +{formatNumber(toSeals(f.cents))}
          </span>
        ))}
        {away && <AwayIncome away={away} onClose={onAwayClosed} />}
      </div>
      <div className="res" title={t("hud.sealed")}>
        <span className="ico">
          <Icon name="pack" />
        </span>
        <span className="v">{formatNumber(sealed)}</span>
      </div>
      {income && (
        <div className="res" title={t("hud.value")}>
          <span className="ico">
            <Icon name="gem" />
          </span>
          <span className="v">{formatNumber(toSeals(income.collectionValueCents))}</span>
        </div>
      )}
    </div>
  );
}

/** The last bookmark of the rail: hovered (or focused), it shows in a small frame what the
 * keys do on the screen shown. */
function Keys({ hints }: { hints: [string, string][] }) {
  const { t } = useI18n();
  return (
    <div className="rail-keys">
      <button className="rail-tab" aria-label={t("keys.title")}>
        <Icon name="keyboard" />
      </button>
      <div className="keys-pop frame" role="tooltip">
        <div className="cartouche">{t("keys.title")}</div>
        {hints.map(([key, label]) => (
          <div key={key} className="keys-row">
            <kbd>{key}</kbd>
            <span>{label}</span>
          </div>
        ))}
      </div>
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
  // What the collection earns, and what it earned while the player was away.
  const [income, setIncome] = useState<IncomeView | null>(null);
  const [awayIncome, setAwayIncome] = useState<IncomeView | null>(null);
  // The "+N" floating up from the balance, one per payment.
  const [incomeFloats, setIncomeFloats] = useState<IncomeFloat[]>([]);
  // An artifact found in the booster being opened: revealed once its result shows.
  const [foundArtifact, setFoundArtifact] = useState<ArtifactView | null>(null);
  // Artifacts to reveal, one after the other (found, or given by achievements).
  const [artifactQueue, setArtifactQueue] = useState<ArtifactView[]>([]);
  // Bumped when an artifact is got: the cabinet and the shop read them again.
  const [artifactCount, setArtifactCount] = useState(0);
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
  // A single booster's opening is over: its result (hand and tally) is shown.
  const [resultShown, setResultShown] = useState(false);
  // Bumped to tear the sealed pack open from its side panel (or a key), without touching it.
  const [tearRequest, setTearRequest] = useState(0);
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

  // Achievements unlocked since the achievements tab was last looked at (its bookmark's badge).
  const [unseenAchievements, setUnseenAchievements] = useState(0);
  const stage = useRef<HTMLElement>(null);

  const showToasts = useCallback((unlocked: AchievementView[]) => {
    if (unlocked.length === 0) return;
    sounds.seal();
    setToasts((current) => [...current, ...unlocked]);
    setUnlockCount((n) => n + unlocked.length);
    setUnseenAchievements((n) => n + unlocked.length);
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
      // Artifacts these achievements gave: their own moment.
      const given = await takeGivenArtifacts(lang);
      if (given.length > 0) {
        setArtifactQueue((queue) => [...queue, ...given]);
        setArtifactCount((n) => n + 1);
      }
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

  // An artifact got may change prices (a discount): the shop is read again.
  useEffect(() => {
    if (artifactCount > 0) loadAll();
  }, [artifactCount, loadAll]);

  /** Buys an artifact at the Boosterium: it is revealed at once, and the prices it changes
   * (discounts) are read again. */
  async function buyCurio(id: string) {
    setBusy(true);
    setError(null);
    try {
      const bought = await buyArtifact(id, lang);
      sounds.coins();
      setArtifactQueue((queue) => [...queue, bought]);
      setArtifactCount((n) => n + 1);
      await refreshWallet();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  // The backend refreshes the card data in the background; reload everything once it is done.
  const dataStatus = useDataStatus(loadAll);
  // No set yet while the data is being prepared (or failed to be): the screens that need
  // sets show the preparation instead of an empty list.
  const waitingForData =
    (tab === "boosterium" || tab === "open" || tab === "collection") &&
    sets.length === 0 &&
    (dataStatus.state === "updating" || dataStatus.state === "failed");

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

  /** Forgets the bets of the last opening (and its result), for a new one. */
  function resetBets() {
    setBetResults([]);
    setBetSkipped([]);
    setAllSeen(false);
    setResultShown(false);
  }

  async function openMany(setCode: string, kind: string, quantity: number) {
    holdingToasts.current = true;
    setPack(null);
    setCards(null);
    resetBets();
    const opened = await openBoosters(setCode, kind, quantity, activeBets(keptFor(kept, setCode)));
    setBatch(opened.map((b) => b.cards));
    const found = opened.flatMap((b) => (b.artifact ? [b.artifact] : []));
    if (found.length > 0) {
      setArtifactQueue((queue) => [...queue, ...found]);
      setArtifactCount((n) => n + 1);
    }
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
  async function openQuick(setCode: string, kind: string, bets: BetRequest[] = activeBets(keptFor(kept, setCode))) {
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
      const opened = await openBooster(setCode, kind, false, bets);
      setCards(opened.cards);
      setFoundArtifact(opened.artifact);
      setBetResults(opened.bets);
      setBetSkipped(opened.skipped);
      setOpenings((n) => n + 1);
      await revealCards(Number.MAX_SAFE_INTEGER);
      setAllSeen(true);
      setResultShown(true);
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
      setFoundArtifact(opened.artifact);
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

  // The artifact a booster hid comes out once its result shows (or as the opening is left).
  useEffect(() => {
    if (resultShown && foundArtifact) {
      setArtifactQueue((queue) => [...queue, foundArtifact]);
      setArtifactCount((n) => n + 1);
      setFoundArtifact(null);
    }
  }, [resultShown, foundArtifact]);

  /** Leaves the opening view: an opening is shown once, never replayed. */
  function closeOpening() {
    // Leaving before every card is seen: what they unlock shows up now.
    if (cards) void reveal(Number.MAX_SAFE_INTEGER);
    if (foundArtifact) {
      setArtifactQueue((queue) => [...queue, foundArtifact]);
      setArtifactCount((n) => n + 1);
      setFoundArtifact(null);
    }
    releaseToasts();
    setDetailsOf(null);
    setPack(null);
    setCards(null);
    setBatch(null);
    resetBets();
  }

  function switchTab(t: Tab) {
    if (t !== tab) closeOpening();
    if (t === "achievements") setUnseenAchievements(0);
    setTab(t);
  }

  // 1 to 6 go to the tabs, as their bookmarks are ordered (not while typing).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName) || e.ctrlKey || e.altKey || e.metaKey) return;
      const index = Number(e.key) - 1;
      if (Number.isInteger(index) && index >= 0 && index < TABS.length) switchTab(TABS[index]);
      // The sealed pack: Space tears it open, Q opens it at once, Escape goes back to the reserve.
      const sealed = tab === "open" && pack && !pack.quick && !cards && !busy;
      if (sealed && !["BUTTON"].includes(target.tagName)) {
        if (e.key === " ") {
          e.preventDefault();
          setTearRequest((n) => n + 1);
        } else if (e.key.toLowerCase() === "q") void openQuick(pack.setCode, pack.kind, activeBets(slip));
        else if (e.key === "Escape" && !detailsOf) closeOpening();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

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
  // What the sealed pack on show costs today, and what the Boosterium would give for it.
  const packShop = pack ? shop.find((s) => s.setCode === pack.setCode && s.kind === pack.kind) : undefined;

  const buy = (item: ShopItem, quantity: number, openNow: boolean) =>
    run(async () => {
      await buyBoosters(item.setCode, item.kind, quantity);
      sounds.coins();
      if (openNow) open(item.setCode, item.kind);
    });

  // What the keys do on this screen (shown by the last bookmark of the rail).
  const opening = tab === "open" && !!(pack || batch);
  const hints: [string, string][] = [
    ["1–7", t("keys.tabs")],
    ...((tab === "boosterium"
      ? [
          ["⏎", t("keys.buy")],
          ["+ −", t("keys.quantity")],
          ["F", t("keys.favorite")],
        ]
      : opening && resultShown
        ? []
      : opening && pack && !pack.quick && !cards
        ? [
            ["␣", t("keys.tear")],
            ["Q", t("keys.quick")],
            [t("keys.esc"), t("keys.reserve")],
          ]
      : opening
        ? [["␣", t("keys.nextCard")]]
        : tab === "open" && (wallet?.sealed.length ?? 0) > 0
          ? [
              ["⏎", t("keys.open")],
              ["Q", t("keys.quick")],
            ]
        : tab === "collection"
          ? [["← →", t("keys.turnPage")]]
          : []) as [string, string][]),
    ...(detailsOf ? ([[t("keys.esc"), t("keys.close")]] as [string, string][]) : []),
    ["F11", t("keys.fullscreen")],
  ];

  return (
    <CurrencyContext.Provider value={currency}>
      <div className="game">
        <header className="hud">
          <div title={t("game.nameTitle") + " · " + save.name}>
            <Logo />
          </div>
          <div className="hud-title">{t(`tab.${tab}`)}</div>
          {wallet && (
            <Resources
              wallet={wallet}
              income={income}
              floats={incomeFloats}
              away={awayIncome}
              onAwayClosed={() => setAwayIncome(null)}
            />
          )}
          <div className="hud-tools">
            {/* Sound on or off, at hand (also in the options, with the volume). */}
            <button
              className="hud-tool"
              onClick={() => {
                setSoundOn(!soundOn);
                setSoundOnState(!soundOn);
                refreshMusic();
                if (!soundOn) sounds.click();
              }}
              title={t(soundOn ? "game.soundOn" : "game.soundOff")}
              aria-label={t(soundOn ? "game.soundOn" : "game.soundOff")}
            >
              <Icon name={soundOn ? "soundOn" : "soundOff"} />
            </button>
            <button className="hud-tool" onClick={() => void saveNow()} title={t("game.savePointTitle")} aria-label={t("game.savePoint")}>
              <Icon name="save" />
            </button>
            <button className="hud-tool" onClick={onMenu} title={t("game.menu")} aria-label={t("game.menu")}>
              <Icon name="menu" />
            </button>
          </div>
        </header>

        <nav className="rail">
          {TABS.map((each) => {
            const badge =
              each === "open" ? wallet?.sealed.reduce((n, s) => n + s.quantity, 0) ?? 0 : each === "achievements" ? unseenAchievements : 0;
            return (
              <button
                key={each}
                className={`rail-tab ${tab === each ? "on" : ""}`}
                onClick={() => switchTab(each)}
                aria-label={t(`tab.${each}`)}
                aria-current={tab === each ? "page" : undefined}
              >
                <Icon name={TAB_ICONS[each]} />
                {badge > 0 && <span className="badge">{badge}</span>}
                <span className="lbl">{t(`tab.${each}`)}</span>
              </button>
            );
          })}
          <Keys hints={hints} />
        </nav>

        <main className="stage" ref={stage}>
        {!waitingForData && <DataStatusBar status={dataStatus} />}
        {error && <p className="error">{error}</p>}
        {notice && <p className="notice">{notice}</p>}
        {!wallet && !error && <Loading label={t("game.loading")} />}
        {wallet && !error && sets.length === 0 && dataStatus.state === "upToDate" && <p className="hint">{t("data.none")}</p>}
        {/* No set yet, the data being prepared: say so, rather than show empty screens. */}
        {wallet && waitingForData && (
          <section className="frame data-wait">
            <div className="cartouche">{t("data.prepareCartouche")}</div>
            <DataPreparing status={dataStatus} />
          </section>
        )}

        {tab === "boosterium" && wallet && !waitingForData && (
          <Boosterium
            shop={shop}
            sets={sets}
            wallet={wallet}
            onBuy={buy}
            onClaimCharity={() => run(() => claimCharity())}
            busy={busy}
            focus={shopFocus}
            artifactsKey={artifactCount}
            onBuyArtifact={(id) => void buyCurio(id)}
          />
        )}

        {tab === "open" && wallet && !waitingForData && (
          <>
            {pack && cards && resultShown && !batch ? (
              <OpeningResult
                set={sets.find((s) => s.code === pack.setCode)}
                setCode={pack.setCode}
                kind={pack.kind}
                cards={cards}
                priceCents={shop.find((s) => s.setCode === pack.setCode && s.kind === pack.kind)?.priceCents ?? null}
                quick={pack.quick}
                bets={betResults}
                skipped={betSkipped}
                cardName={betCardName}
                onDetails={(card) => setDetailsOf({ setCode: pack.setCode, uuid: card.uuid })}
                onSold={refreshWallet}
                remaining={remaining ?? null}
                busy={busy}
                onNext={() => remaining && open(remaining.setCode, remaining.kind)}
                onQuickNext={() => remaining && void openQuick(remaining.setCode, remaining.kind)}
                onBack={closeOpening}
              />
            ) : pack && !pack.quick && !batch ? (
              // One booster, sealed then revealed: the pack alone in the light on the left, the
              // bets beside it (then the bets placed), with the way to tear it open.
              <div className="split opening-split">
                <section className="frame opening-stage">
                  <div className="cartouche">
                    {sets.find((s) => s.code === pack.setCode)?.name ?? pack.setCode} · {kindLabel(t, pack.kind)}
                  </div>
                  <div className="opening-light" aria-hidden="true" />
                  {opening3d ? (
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
                        onFinished={() => setResultShown(true)}
                        tearRequest={tearRequest}
                      />
                    </Suspense>
                  ) : (
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
                      onFinished={() => setResultShown(true)}
                      tearRequest={tearRequest}
                    />
                  )}
                  {!cards && (
                    <div className="pack-pills">
                      {packShop && <span className="pill">{t("open.priceToday", { amount: money(packShop.priceCents) })}</span>}
                      {packShop && <span className="pill">{t("open.resale", { amount: money(packShop.sellBackCents) })}</span>}
                      {remaining && <span className="pill">{t("open.leftInReserve", { count: remaining.quantity })}</span>}
                    </div>
                  )}
                </section>
                <aside className="frame bet-side">
                  <div className="cartouche">🎲 {t("bet.title")}</div>
                  <div className="scroll">
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
                      panel
                      locked={!!cards}
                    />
                  </div>
                  <div className="buy-bar">
                    {!cards && (
                      <>
                        <button className="big wax" disabled={busy} onClick={() => setTearRequest((n) => n + 1)}>
                          {t("open.tear")}
                        </button>
                        <button disabled={busy} onClick={() => void openQuick(pack.setCode, pack.kind, activeBets(slip))}>
                          {t("open.quick")}
                        </button>
                      </>
                    )}
                    <button onClick={closeOpening}>{t("open.back")}</button>
                  </div>
                </aside>
              </div>
            ) : pack || batch ? (
              <section className="frame">
                <div className="cartouche">
                  {pack ? `${sets.find((s) => s.code === pack.setCode)?.name ?? pack.setCode} · ${kindLabel(t, pack.kind)}` : t("tab.open")}
                </div>
                <div className="controls">
                  <button onClick={closeOpening}>{t("open.back")}</button>
                  {/* Once a batch is open: the next one of the same kind, if any is left (a single
                      booster offers it in its result). */}
                  {remaining && batch && (
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
                  {remaining && batch && (
                    <button disabled={busy} onClick={() => void openQuick(remaining.setCode, remaining.kind)}>
                      {t("open.quick")}
                    </button>
                  )}
                </div>
                {batch && allSeen && <BetResults bets={betResults} skipped={betSkipped} cardName={betCardName} />}
                {/* A quick opening shows its result as soon as it is drawn. */}
                {pack?.quick && <Loading />}
                {batch && (
                  <BatchOpening
                    boosters={batch}
                    onFinished={() => {
                      releaseToasts();
                      setAllSeen(true);
                    }}
                  />
                )}
              </section>
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
          </>
        )}

        {tab === "open" && detailsOf && (
          <CardDetails setCode={detailsOf.setCode} uuid={detailsOf.uuid} onClose={() => setDetailsOf(null)} />
        )}

        {tab === "collection" && !waitingForData && (
          <Collection
            sets={sets}
            refreshKey={openings}
            onChanged={refreshWallet}
            duplicatesBonus={wallet?.artifactPerks.duplicatesPercent ?? 0}
            onShop={(setCode) => {
              setShopFocus({ setCode, at: Date.now() });
              switchTab("boosterium");
            }}
          />
        )}

        {tab === "achievements" && <Achievements refreshKey={unlockCount} />}

        {tab === "curiosities" && (
          <Curiosities
            refreshKey={artifactCount}
            onShop={() => {
              setShopFocus({ setCode: CURIOS, at: Date.now() });
              switchTab("boosterium");
            }}
          />
        )}

        {tab === "stats" && <Stats refreshKey={`${openings}-${wallet?.balanceCents}`} />}

        {tab === "history" && (
          <History
            onChanged={() => {
              setOpenings((n) => n + 1);
              void refreshWallet();
            }}
          />
        )}
        <BackToTop target={stage} />
        </main>

        <Ambience />
        {artifactQueue.length > 0 && (
          <ArtifactReveal item={artifactQueue[0]} onClose={() => setArtifactQueue((queue) => queue.slice(1))} />
        )}

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
