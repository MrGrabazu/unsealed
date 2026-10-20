import { LayoutGroup, motion } from "framer-motion";
import { useEffect, useState } from "react";
import type { OpenedCard, SetSummary } from "./api";
import { isBigCard } from "./BatchOpening";
import { SellBoosterButton, SellCardButton, useBoosterSale } from "./BoosterSale";
import { cardBackUrl } from "./canvasArt";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Money, useMoney } from "./money";
import { Pack } from "./Pack";
import { sheenClass } from "./RevealedCards";
import { revealSound, sounds } from "./sound";

interface Props {
  setCode: string;
  set: SetSummary | undefined;
  kind: string;
  /** The cards, once the pack is torn open (the draw happens then). */
  cards: OpenedCard[] | null;
  /** Draws the booster; false when that failed. */
  onTear: () => Promise<boolean>;
  /** `count` cards have been turned over: their achievements happen now. */
  onReveal: (count: number) => void;
  busy: boolean;
  onDetails: (card: OpenedCard) => void;
  /** Cards of the booster were sold (the wallet changed). */
  onSold: () => void;
  /** Every card is set aside: the game shows the booster's result. */
  onFinished?: () => void;
  /** Bumped to tear the pack open without touching it. */
  tearRequest?: number;
}

/** How long the last card is left in the row before the result shows (ms). */
const RESULT_AFTER_MS = 900;

/** How dramatic a card's reveal is. */
const hitOf = (card: OpenedCard) =>
  card.rarity === "mythic" ? "hit-mythic" : isBigCard(card) ? "hit-rare" : card.foil ? "hit-foil" : "";

/** The cards' back (drawn once, see `canvasArt.ts`), as an image URL once ready. */
function useCardBack(): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void cardBackUrl().then((u) => live && setUrl(u));
    return () => {
      live = false;
    };
  }, []);
  return url;
}

/** A card that can be face down (its back) or face up. */
function FlipCard({ card, faceUp }: { card: OpenedCard; faceUp: boolean }) {
  const { t } = useI18n();
  const backUrl = useCardBack();
  return (
    <motion.div
      className="card"
      data-rarity={card.rarity}
      initial={false}
      animate={{ rotateY: faceUp ? 180 : 0 }}
      transition={{ duration: 0.5, ease: "easeInOut" }}
    >
      <div className="face back" style={backUrl ? { backgroundImage: `url(${backUrl})` } : undefined} />
      <div className="face front">
        <CardImage front={card.imageUrl} back={faceUp ? card.backImageUrl : null} alt={card.name} />
        {card.foil && <div className={sheenClass(card.rarity)} />}
        {card.isNew && <span className="badge-new">{t("open.new")}</span>}
      </div>
    </motion.div>
  );
}

/** Opening one booster, like for real: the sealed pack, torn open; the cards in a pile,
 * revealed one by one (in a real booster's order) and set aside in a row; then all of them to
 * look at. */
export function PackOpening({ setCode, set, kind, cards, onTear, onReveal, busy, onDetails, onSold, onFinished, tearRequest }: Props) {
  const { t, tx } = useI18n();
  const money = useMoney();
  const sale = useBoosterSale(cards, onSold);
  const [torn, setTorn] = useState(false);
  // Cards already set aside in the row, and whether the top of the pile is turned over.
  const [aside, setAside] = useState(0);
  const [topFaceUp, setTopFaceUp] = useState(false);

  // A new pack starts sealed.
  useEffect(() => {
    setTorn(false);
    setAside(0);
    setTopFaceUp(false);
  }, [setCode, kind]);
  useEffect(() => {
    setAside(0);
    setTopFaceUp(false);
  }, [cards]);

  const total = cards?.length ?? 0;
  const done = cards != null && aside >= total;
  useEffect(() => {
    if (!done) return;
    const timer = setTimeout(() => onFinished?.(), RESULT_AFTER_MS);
    return () => clearTimeout(timer);
  }, [done]);

  function advance() {
    if (!cards || done) return;
    if (!topFaceUp) {
      setTopFaceUp(true);
      revealSound(cards[aside]);
      onReveal(aside + 1);
    } else {
      setTopFaceUp(false);
      sounds.slide();
      setAside((n) => n + 1);
    }
  }

  // Space or Enter turns the next card over too.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.key === " " || e.key === "Enter") && cards && !done) {
        e.preventDefault();
        advance();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function tearIt() {
    if (torn || busy) return;
    setTorn(true);
    sounds.tear();
    // If the draw fails, the pack is whole again (the error is shown above it).
    void onTear().then((ok) => !ok && setTorn(false));
  }

  // Asked from outside (the tally's button, a key).
  useEffect(() => {
    if (tearRequest && tearRequest > 0 && !cards) tearIt();
  }, [tearRequest]);

  if (!cards) {
    return <Pack setCode={setCode} set={set} kind={kind} torn={torn} disabled={busy} onTear={tearIt} />;
  }

  const pile = cards.slice(aside);
  const value = cards.slice(0, aside).reduce((sum, c) => sum + (c.priceCents ?? 0), 0);

  return (
    <section className="pack-opening">
      <div className="opening-bar">
        <span>{t("open.revealed", { revealed: aside, total })}</span>
        {!done && (
          <button
            onClick={() => {
              setTopFaceUp(false);
              setAside(total);
              onReveal(total);
            }}
          >
            {t("open.revealAll")}
          </button>
        )}
        {aside > 0 && <strong className="total">{tx("open.boosterValue", { amount: money(value) })}</strong>}
        {done && <SellBoosterButton sale={sale} />}
      </div>

      <LayoutGroup>
        {!done && (
          <div className="pile-area">
            {/* Not a <button>: a double-faced card's "turn over" button sits inside. */}
            <div
              role="button"
              tabIndex={0}
              className="pile"
              onClick={advance}
              aria-label={topFaceUp ? t("open.stackHintAside") : t("open.stackHintReveal")}
            >
              {/* Drawn bottom first: the first card to reveal is on top. */}
              {pile
                .map((card, k) => ({ card, k, index: aside + k }))
                .reverse()
                .map(({ card, k, index }) => {
                  const top = k === 0;
                  return (
                    <motion.div
                      key={index}
                      layoutId={`pack-card-${index}`}
                      className={`pile-card ${top && topFaceUp ? `face-up ${hitOf(card)}` : ""}`}
                      style={{ zIndex: total - k }}
                      animate={{
                        x: Math.min(k, 6) * 2,
                        y: Math.min(k, 6) * -2,
                        scale: top && topFaceUp ? 1.08 : 1,
                      }}
                      transition={{ type: "spring", stiffness: 260, damping: 24 }}
                    >
                      <FlipCard card={card} faceUp={top && topFaceUp} />
                    </motion.div>
                  );
                })}
            </div>
            <p className="hint">{topFaceUp ? t("open.stackHintAside") : t("open.stackHintReveal")}</p>
          </div>
        )}

        {aside > 0 && (
          <div className="revealed-row">
            {cards.slice(0, aside).map((card, index) => (
              <motion.div
                role="button"
                tabIndex={0}
                key={index}
                layoutId={`pack-card-${index}`}
                className={`revealed-card ${hitOf(card)} ${sale.sold[index] ? "sold" : ""}`}
                onClick={() => onDetails(card)}
                onKeyDown={(e) => e.key === "Enter" && onDetails(card)}
                title={t("details.open")}
                transition={{ type: "spring", stiffness: 260, damping: 26 }}
              >
                <FlipCard card={card} faceUp />
                <span className="card-caption visible">
                  <span className="name">{card.name}</span>
                  <span className="price">{card.priceCents == null ? t("common.none") : <Money cents={card.priceCents} estimated={card.priceEstimated} />}</span>
                </span>
                {done && <SellCardButton sale={sale} index={index} card={card} />}
              </motion.div>
            ))}
          </div>
        )}
      </LayoutGroup>
      {done && <p className="hint">{t("open.details")}</p>}
    </section>
  );
}
