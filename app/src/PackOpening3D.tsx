import { useEffect, useMemo, useState } from "react";
import type { OpenedCard, SetSummary } from "./api";
import { SellBoosterButton, useBoosterSale } from "./BoosterSale";
import { useI18n } from "./i18n";
import { CardImage } from "./CardImage";
import { Money, useMoney } from "./money";
import { FoilSheen } from "./foils";
import { smallImage } from "./images";
import { NewBadge, RevealMention } from "./NewBadge";
import { RevealedCards } from "./RevealedCards";
import { packLookOf } from "./scene/packArt";
import { PackScene, type PassRequest } from "./scene/PackScene";

/** The rarities whose runs can be passed; foils never are. */
const PASSABLE = ["common", "uncommon", "rare", "mythic"] as const;
type Passable = (typeof PASSABLE)[number];

function passableRarity(card: OpenedCard): Passable | null {
  return !card.foil && (PASSABLE as readonly string[]).includes(card.rarity) ? (card.rarity as Passable) : null;
}

interface Props {
  setCode: string;
  set: SetSummary | undefined;
  kind: string;
  cards: OpenedCard[] | null;
  onTear: () => Promise<boolean>;
  onReveal: (count: number) => void;
  onDetails: (card: OpenedCard) => void;
  /** Cards of the booster were sold (the wallet changed). */
  onSold: () => void;
  /** The last card has left the scene: the game shows the booster's result. */
  onFinished?: () => void;
  /** Bumped to tear the pack open without touching it. */
  tearRequest?: number;
}

/** Opening one booster in the 3D scene: tear the pack, then slide the cards off the pile one
 * by one. Once the last card has left, the scene gives way to the list of the cards, and a
 * click on one shows its details. */
export function PackOpening3D({ setCode, set, kind, cards, onTear, onReveal, onDetails, onSold, onFinished, tearRequest = 0 }: Props) {
  const { t, tx, plural } = useI18n();
  const money = useMoney();
  const sale = useBoosterSale(cards, onSold);
  const [top, setTop] = useState(0);
  const [finished, setFinished] = useState(false);
  const [slideRequest, setSlideRequest] = useState(0);
  const [tearing, setTearing] = useState(false);
  // Bumped when tearing failed: the scene starts over, the pack sealed again.
  const [attempt, setAttempt] = useState(0);
  // The card seen on top of the pile, for its mention (none while one leaves).
  const [shown, setShown] = useState<number | null>(null);

  const look = useMemo(() => packLookOf(setCode, set, kind, t), [setCode, set, kind, t]);

  useEffect(() => {
    setTop(0);
    setFinished(false);
    setSlideRequest(0);
    setTearing(false);
    setShown(null);
    setPassRequest(null);
  }, [setCode, kind, cards]);

  // The thumbnails of the cards seen beside the scene, loaded and decoded as soon as the booster
  // is torn: asked for only as each card came, they showed a black frame first. Kept while the
  // booster is shown, so the browser keeps them ready.
  useEffect(() => {
    if (!cards) return;
    const images = cards
      .flatMap((c) => [c.imageUrl, c.backImageUrl])
      .filter((url): url is string => url != null)
      .map((url) => {
        const image = new Image();
        image.src = smallImage(url);
        void image.decode().catch(() => {});
        return image;
      });
    return () => {
      images.length = 0;
    };
  }, [cards]);

  const total = cards?.length ?? 0;

  // The run of cards from the top of the pile that share its rarity, foils aside: in a real
  // booster's order, the commons, then the uncommons… so they can be passed in one go, and the
  // rest seen card by card (a foil is never passed: it may be the one awaited).
  const [passRequest, setPassRequest] = useState<PassRequest | null>(null);
  const run = useMemo(() => {
    if (!cards || finished) return null;
    const rarity = passableRarity(cards[top]);
    if (!rarity) return null;
    let to = top;
    while (to < total && passableRarity(cards[to]) === rarity) to++;
    return { rarity, to, count: to - top };
  }, [cards, top, finished, total]);
  // Hidden while its cards are being passed.
  const passingRun = passRequest != null && top < passRequest.to;

  // The card on top of the pile is seen: its achievements happen now.
  useEffect(() => {
    if (cards) onReveal(finished ? total : top + 1);
  }, [cards, top, finished]);

  useEffect(() => {
    if (cards && finished) onFinished?.();
  }, [cards, finished]);

  // Space slides the top card off.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === " " && cards && !finished) {
        e.preventDefault();
        setSlideRequest((n) => n + 1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cards, finished]);

  const seen = cards ? cards.slice(0, finished ? total : top) : [];
  const value = seen.reduce((sum, c) => sum + (c.priceCents ?? 0), 0);

  return (
    <section className={`pack-opening ${finished ? "" : "scene-live"}`}>
      <div className="opening-bar">
        {cards ? (
          <>
            <span>{t("open.revealed", { revealed: finished ? total : Math.min(top + 1, total), total })}</span>
            {run && !passingRun && (
              <button onClick={() => setPassRequest({ to: run.to })}>
                {t("open.pass", { cards: plural(`rarity.${run.rarity}`, run.count) })}
              </button>
            )}
            {seen.length > 0 && <strong className="total">{tx("open.boosterValue", { amount: money(value) })}</strong>}
            {finished && <SellBoosterButton sale={sale} />}
          </>
        ) : (
          <span className="hint">{t("scene.tearHint")}</span>
        )}
      </div>

      {!finished && (
        <>
          {/* The cards already seen line up in a column over the scene's right edge, the latest
              on top: laid over it, they take no room (below it, they made it shrink card after
              card; beside it, they pushed the pack off centre). */}
          <div className="scene-row">
          <div className="scene-frame">
            <PackScene
              key={attempt}
              look={look}
              cards={cards}
              top={top}
              slideRequest={slideRequest}
              passRequest={passRequest}
              tearRequest={tearRequest}
              onNext={() => setTop((n) => Math.min(n + 1, total - 1))}
              onFinished={() => setFinished(true)}
              onShown={setShown}
              onTorn={() => {
                if (tearing) return;
                setTearing(true);
                void onTear().then((ok) => {
                  if (ok) return;
                  setTearing(false);
                  setAttempt((n) => n + 1);
                });
              }}
            />
          </div>
          <RevealMention card={cards && shown != null ? cards[shown] : null} index={shown ?? -1} />
          {seen.length > 0 && (
            <div className="seen-strip" aria-label={t("open.revealed", { revealed: seen.length, total })}>
              {[...seen].reverse().map((card, i) => (
                <button key={seen.length - i} className={`seen-card ${card.rarity}`} onClick={() => onDetails(card)} title={card.name}>
                  <span className="thumb">
                    <CardImage front={smallImage(card.imageUrl)} back={smallImage(card.backImageUrl)} alt={card.name} />
                    {card.foil && <FoilSheen rarity={card.rarity} kind={card.foilKind} badge={false} />}
                    <NewBadge card={card} />
                  </span>
                  <span className="seen-price">{card.priceCents == null ? "—" : <Money cents={card.priceCents} estimated={card.priceEstimated} />}</span>
                </button>
              ))}
            </div>
          )}
          </div>
          <p className="hint scene-hint">{!cards ? t("scene.shakeHint") : t("scene.peelHint")}</p>
          {set?.art && !cards && (
            <p className="shop-credit">{t("art.credit", { card: set.art.cardName, artist: set.art.artist })}</p>
          )}
        </>
      )}
      {finished && <p className="hint scene-hint">{t("open.details")}</p>}

      {finished && seen.length > 0 && <RevealedCards cards={seen} final onDetails={onDetails} sale={sale} />}
    </section>
  );
}
