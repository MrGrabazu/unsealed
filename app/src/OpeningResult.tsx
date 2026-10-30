import { useEffect, useRef } from "react";
import type { OpenedCard, SealedStack, SetSummary, SettledBet, SkippedBet } from "./api";
import { BackToTop } from "./BackToTop";
import { BetResults } from "./BetResults";
import { cornerOf } from "./corners";
import { kindLabel } from "./boosterKinds";
import { SellDuplicatesButton, useBoosterSale } from "./BoosterSale";
import { useI18n } from "./i18n";
import { foilLabel } from "./foils";
import { Money, useMoney, useMoneyText } from "./money";
import { RevealedCards } from "./RevealedCards";
import { ShareButton } from "./Share";
import { drawBoosterShare, type BoosterShare } from "./sharePicture";
import { bestCard, revealSound, sounds } from "./sound";

/** Delay between two cards dealt (ms), as in `.revealed-row.final`. */
const DEAL_MS = 30;

interface Props {
  set: SetSummary | undefined;
  setCode: string;
  kind: string;
  cards: OpenedCard[];
  /** What such a booster costs today, to tell a gain from a loss. */
  priceCents: number | null;
  /** A quick opening: its cards are dealt at once, with the rip and the best card's sound. */
  quick: boolean;
  bets: SettledBet[];
  skipped: SkippedBet[];
  cardName: (uuid: string) => string;
  onDetails: (card: OpenedCard) => void;
  /** Cards of the booster were sold (the wallet changed). */
  onSold: () => void;
  /** The next booster of the same kind, if any is left. */
  remaining: SealedStack | null;
  busy: boolean;
  onNext: () => void;
  onQuickNext: () => void;
  onBack: () => void;
}

/** A booster once every card is seen: its hand spread out, and beside it the tally (its value,
 * gain or loss on its price, new cards, the best one, what the Boosterium gives, the bets),
 * with selling it all and the next one. */
export function OpeningResult(props: Props) {
  const { set, setCode, kind, cards, priceCents, quick, bets, skipped, cardName, onDetails, onSold, remaining, busy } = props;
  const { t, tx } = useI18n();
  const money = useMoney();
  const moneyText = useMoneyText();
  const sale = useBoosterSale(cards, onSold);
  const list = useRef<HTMLDivElement>(null);

  // A quick opening: the rip, then the best card's sound once every card has landed.
  useEffect(() => {
    if (!quick) return;
    sounds.tear();
    const timer = setTimeout(() => revealSound(bestCard(cards)), cards.length * DEAL_MS + 350);
    return () => clearTimeout(timer);
  }, [cards, quick]);

  const value = cards.reduce((sum, c) => sum + (c.priceCents ?? 0), 0);
  const resale = cards.reduce((sum, c) => sum + c.sellValueCents, 0);
  const best = cards.reduce<OpenedCard | null>((b, c) => ((c.priceCents ?? 0) > (b?.priceCents ?? -1) ? c : b), null);
  const fresh = cards.filter((c) => c.isNew).length;
  const freshFoils = cards.filter((c) => c.newFoil).length;
  const gain = priceCents == null ? null : value - priceCents;

  /** The picture to share: every card of the booster, its value, its best card large. */
  const boosterShare = (): BoosterShare => ({
    title: `${set?.name ?? setCode} · ${kindLabel(t, kind)}`,
    valueText: t("share.boosterValue", { amount: moneyText(value) }),
    gainText: gain == null ? null : t(gain >= 0 ? "share.gain" : "share.loss", { amount: moneyText(Math.abs(gain)) }),
    gain: (gain ?? 0) >= 0,
    cards: cards.map((c) => ({
      imageUrl: c.imageUrl,
      name: c.name,
      foil: c.foil,
      foilLabel: c.foil ? (foilLabel(c.foilKind) ?? t(c.rarity === "art" ? "collection.signedBadge" : "collection.foilBadge")) : null,
      newLabel: c.isNew ? t("open.new") : c.newFoil ? t("open.newFoil") : null,
      corner: cornerOf(c.setCode),
    })),
    best: best ? cards.indexOf(best) : 0,
    bestLabel: t("open.tallyBest"),
    bestText: best ? `${best.name} · ${moneyText(best.priceCents ?? 0)}` : "",
    tagline: t("share.tagline"),
  });

  return (
    <div className="split result-split">
      <section className="frame">
        <div className="cartouche">
          {set?.name ?? setCode} · {kindLabel(t, kind)}
        </div>
        <div className="scroll" ref={list}>
          <RevealedCards cards={cards} final onDetails={onDetails} sale={sale} />
          <p className="hint scene-hint">{t("open.details")}</p>
          <BackToTop target={list} />
        </div>
      </section>
      <aside className="frame result-side">
        <div className="cartouche">{t("open.tally")}</div>
        <div className="scroll">
          <div className="bigvalue">
            <small>{t("open.tallyValue")}</small>
            <b>
              <Money cents={value} />
            </b>
            {gain != null && priceCents != null && (
              <em className={gain >= 0 ? "gain" : "loss"}>
                {tx(gain >= 0 ? "open.tallyGain" : "open.tallyLoss", { amount: money(Math.abs(gain)), price: money(priceCents) })}
              </em>
            )}
          </div>
          <div className="gild-divider" />
          <div className="ready-line">
            <span>{t("open.tallyNew")}</span>
            <b>{fresh}</b>
          </div>
          {freshFoils > 0 && (
            <div className="ready-line">
              <span>{t("open.tallyNewFoils")}</span>
              <b className="new-foil-count">{freshFoils}</b>
            </div>
          )}
          {best && (
            <div className="ready-line">
              <span>{t("open.tallyBest")}</span>
              <b className="result-best">
                {best.name} · <Money cents={best.priceCents ?? 0} />
              </b>
            </div>
          )}
          <div className="ready-line">
            <span>{t("open.tallyResale")}</span>
            <b>
              <Money cents={resale} />
            </b>
          </div>
          {(bets.length > 0 || skipped.length > 0) && <BetResults bets={bets} skipped={skipped} cardName={cardName} />}
          <ShareButton
            className="result-share"
            make={() => drawBoosterShare(boosterShare())}
            fileName={`${set?.name ?? setCode} ${kindLabel(t, kind)}`}
          />
        </div>
        <div className="buy-bar">
          <div className="result-sell">
            <SellDuplicatesButton setCode={setCode} opened={cards} sale={sale} onSold={onSold} large />
          </div>
          {remaining ? (
            <>
              <button className="big" disabled={busy} onClick={quick ? props.onQuickNext : props.onNext}>
                {t("open.next", { count: remaining.quantity })}
              </button>
              {/* The other way of opening the next one: after a quick opening, the scene again. */}
              {quick ? (
                <button disabled={busy} onClick={props.onNext} title={t("open.animatedHint")}>
                  {t("open.animated")}
                </button>
              ) : (
                <button disabled={busy} onClick={props.onQuickNext} title={t("open.quickHint")}>
                  {t("open.quick")}
                </button>
              )}
            </>
          ) : null}
          <button onClick={props.onBack}>{t("open.back")}</button>
        </div>
      </aside>
    </div>
  );
}
