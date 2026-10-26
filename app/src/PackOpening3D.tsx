import { useEffect, useMemo, useState } from "react";
import type { OpenedCard, SetSummary } from "./api";
import { SellBoosterButton, useBoosterSale } from "./BoosterSale";
import { useI18n } from "./i18n";
import { CardImage } from "./CardImage";
import { Money, useMoney } from "./money";
import { FoilSheen } from "./foils";
import { smallImage } from "./images";
import { RevealedCards } from "./RevealedCards";
import { packLookOf } from "./scene/packArt";
import { PackScene } from "./scene/PackScene";

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
  const { t, tx } = useI18n();
  const money = useMoney();
  const sale = useBoosterSale(cards, onSold);
  const [top, setTop] = useState(0);
  const [finished, setFinished] = useState(false);
  const [slideRequest, setSlideRequest] = useState(0);
  const [tearing, setTearing] = useState(false);
  // Bumped when tearing failed: the scene starts over, the pack sealed again.
  const [attempt, setAttempt] = useState(0);

  const look = useMemo(() => packLookOf(setCode, set, kind, t), [setCode, set, kind, t]);

  useEffect(() => {
    setTop(0);
    setFinished(false);
    setSlideRequest(0);
    setTearing(false);
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
    <section className="pack-opening">
      <div className="opening-bar">
        {cards ? (
          <>
            <span>{t("open.revealed", { revealed: finished ? total : Math.min(top + 1, total), total })}</span>
            {!finished && <button onClick={() => setFinished(true)}>{t("open.revealAll")}</button>}
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
              tearRequest={tearRequest}
              onNext={() => setTop((n) => Math.min(n + 1, total - 1))}
              onFinished={() => setFinished(true)}
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
          {seen.length > 0 && (
            <div className="seen-strip" aria-label={t("open.revealed", { revealed: seen.length, total })}>
              {[...seen].reverse().map((card, i) => (
                <button key={seen.length - i} className={`seen-card ${card.rarity}`} onClick={() => onDetails(card)} title={card.name}>
                  <span className="thumb">
                    <CardImage front={smallImage(card.imageUrl)} back={smallImage(card.backImageUrl)} alt={card.name} />
                    {card.foil && <FoilSheen rarity={card.rarity} kind={card.foilKind} badge={false} />}
                    {card.isNew && <span className="badge-new">{t("open.new")}</span>}
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
