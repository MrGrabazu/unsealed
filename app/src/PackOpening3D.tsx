import { useEffect, useMemo, useState } from "react";
import type { OpenedCard, SetSummary } from "./api";
import { isBigCard } from "./BatchOpening";
import { kindLabel } from "./boosterKinds";
import { CardImage } from "./CardImage";
import { useI18n } from "./i18n";
import { Money, useMoney } from "./money";
import { PackScene } from "./scene/PackScene";

interface Props {
  setCode: string;
  set: SetSummary | undefined;
  kind: string;
  cards: OpenedCard[] | null;
  onTear: () => Promise<boolean>;
  onReveal: (count: number) => void;
  onDetails: (card: OpenedCard) => void;
}

/** A colour per set, the same as the classic wrapper's. */
function hueOf(code: string): number {
  let h = 0;
  for (const c of code) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

const hitOf = (card: OpenedCard) =>
  card.rarity === "mythic" ? "hit-mythic" : isBigCard(card) ? "hit-rare" : card.foil ? "hit-foil" : "";

/** Opening one booster in the 3D scene: tear the pack, then slide the cards off the pile one
 * by one. Once the last card has left, the scene gives way to the list of the cards, and a
 * click on one shows its details. */
export function PackOpening3D({ setCode, set, kind, cards, onTear, onReveal, onDetails }: Props) {
  const { t, tx } = useI18n();
  const money = useMoney();
  const [top, setTop] = useState(0);
  const [finished, setFinished] = useState(false);
  const [slideRequest, setSlideRequest] = useState(0);
  const [tearing, setTearing] = useState(false);

  const look = useMemo(
    () => ({
      hue: hueOf(setCode),
      setName: set?.name ?? setCode,
      setCode,
      kindLabel: kindLabel(t, kind),
      artUrl: set?.art?.imageUrl ?? null,
      iconUrl: set?.iconUrl ?? null,
    }),
    [setCode, set, kind, t],
  );

  useEffect(() => {
    setTop(0);
    setFinished(false);
    setSlideRequest(0);
    setTearing(false);
  }, [setCode, kind, cards]);

  const total = cards?.length ?? 0;

  // The card on top of the pile is seen: its achievements happen now.
  useEffect(() => {
    if (cards) onReveal(finished ? total : top + 1);
  }, [cards, top, finished]);

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
          </>
        ) : (
          <span className="hint">{t("scene.tearHint")}</span>
        )}
      </div>

      {!finished && (
        <>
          <div className="scene-frame">
            <PackScene
              look={look}
              cards={cards}
              top={top}
              slideRequest={slideRequest}
              onNext={() => setTop((n) => Math.min(n + 1, total - 1))}
              onFinished={() => setFinished(true)}
              onTorn={() => {
                if (tearing) return;
                setTearing(true);
                void onTear().then((ok) => !ok && setTearing(false));
              }}
            />
          </div>
          <p className="hint scene-hint">{!cards ? t("scene.shakeHint") : t("scene.peelHint")}</p>
          {set?.art && !cards && (
            <p className="shop-credit">{t("art.credit", { card: set.art.cardName, artist: set.art.artist })}</p>
          )}
        </>
      )}
      {finished && <p className="hint scene-hint">{t("open.details")}</p>}

      {seen.length > 0 && (
        <div className={`revealed-row ${finished ? "final" : ""}`}>
          {seen.map((card, index) => (
            <div
              key={index}
              role="button"
              tabIndex={0}
              className={`revealed-card ${hitOf(card)}`}
              style={{ "--i": index } as React.CSSProperties}
              onClick={() => onDetails(card)}
              onKeyDown={(e) => e.key === "Enter" && onDetails(card)}
              title={t("details.open")}
            >
              <div className="thumb">
                <CardImage front={card.imageUrl} back={card.backImageUrl} alt={card.name} />
                {card.foil && <div className="foil-sheen" />}
                {card.isNew && <span className="badge-new">{t("open.new")}</span>}
              </div>
              <span className="card-caption visible">
                <span className="name">{card.name}</span>
                <span className="price">{card.priceCents == null ? t("common.none") : <Money cents={card.priceCents} estimated={card.priceEstimated} />}</span>
              </span>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
