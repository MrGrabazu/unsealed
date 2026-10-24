import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { BoxProgress, SealedStack, SetSummary } from "./api";
import { BackToTop } from "./BackToTop";
import { kindLabel, wrapperFinish } from "./boosterKinds";
import { useI18n } from "./i18n";
import { Icon } from "./icons";
import { Money, useMoney } from "./money";
import { hueOf } from "./Pack";
import { PriceTrend } from "./PriceTrend";
import { SetIcon } from "./SetIcon";

/** Batch sizes offered next to "Open one". */
const BATCH_SIZES = [10, 100];

interface Props {
  sealed: SealedStack[];
  sets: SetSummary[];
  busy: boolean;
  onOpen: (stack: SealedStack) => void;
  /** Opens one without the animation. */
  onOpenQuick: (stack: SealedStack) => void;
  onOpenMany: (stack: SealedStack, quantity: number) => void;
  onSell: (stack: SealedStack) => void;
  onGoShopping: () => void;
}

const keyOf = (s: SealedStack) => `${s.setCode}/${s.kind}`;

/** A sealed booster as it stands in the reserve: an original wrapper in the set's hue, with
 * the featured art in its window (no real wrapper: they carry the Magic logo). */
export function Wrapper({ stack, set, large = false }: { stack: SealedStack; set: SetSummary | undefined; large?: boolean }) {
  const { t } = useI18n();
  const style = { "--hue": hueOf(stack.setCode) } as CSSProperties;
  return (
    <span className={`wrapper finish-${wrapperFinish(stack.kind)} ${large ? "large" : ""}`} style={style} aria-hidden="true">
      <span className="wrapper-holo" />
      <span className="wrapper-name">{stack.setName}</span>
      <span className="wrapper-window">
        {set?.art ? <img src={set.art.imageUrl} alt="" draggable={false} /> : <SetIcon url={set?.iconUrl ?? null} name={stack.setName} />}
      </span>
      <span className="wrapper-kind">{kindLabel(t, stack.kind)}</span>
      <span className="wrapper-seal">U</span>
    </span>
  );
}

/** How far a booster's box is, when it holds a box topper: a gilded gauge, "Box topper in 14
 * boosters". Every box's worth of boosters opened gives it (toppers.rs of the game). */
export function BoxGauge({ progress }: { progress: BoxProgress }) {
  const { t, plural } = useI18n();
  const left = progress.size - progress.opened;
  return (
    <div className="box-gauge" title={t("topper.gaugeHint", { size: progress.size })}>
      <span className="box-gauge-label">
        <Icon name="pack" />
        {plural("topper.gauge", left, { packs: progress.packs })}
      </span>
      <span className="bar">
        <i style={{ width: `${(progress.opened / progress.size) * 100}%` }} />
      </span>
      <small>
        {progress.opened} / {progress.size}
      </small>
    </div>
  );
}

/** The sealed boosters the player owns, standing in the reserve; the one chosen is ready to
 * open beside them, with what it cost and what it is worth. */
export function SealedBoosters({ sealed, sets, busy, onOpen, onOpenQuick, onOpenMany, onSell, onGoShopping }: Props) {
  const money = useMoney();
  const { t, tx } = useI18n();
  const [chosen, setChosen] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const current = sealed.find((s) => keyOf(s) === chosen) ?? sealed[0] ?? null;

  // Enter opens the one chosen, Q opens it at once (not while typing).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (!current || busy || ["INPUT", "SELECT", "TEXTAREA", "BUTTON"].includes(target.tagName)) return;
      if (e.key === "Enter") onOpen(current);
      else if (e.key.toLowerCase() === "q") onOpenQuick(current);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  if (!current) {
    return (
      <section className="frame empty-state">
        <div className="cartouche">{t("open.reserve")}</div>
        <p>{t("open.noSealed")}</p>
        <button className="big" style={{ maxWidth: 360, margin: "0 auto" }} onClick={onGoShopping}>
          {t("open.goShopping")}
        </button>
      </section>
    );
  }

  const set = sets.find((candidate) => candidate.code === current.setCode);
  // Profit if the whole stack were sold back today.
  const pnl = current.quantity * current.sellValueCents - current.costCents;
  return (
    <div className="split">
      <section className="frame">
        <div className="cartouche">{t("open.reserve")}</div>
        <div className="scroll" ref={list}>
          <div className="reserve">
            {sealed.map((s) => (
              <button key={keyOf(s)} className={`reserve-cell ${s === current ? "sel" : ""}`} onClick={() => setChosen(keyOf(s))}>
                <Wrapper stack={s} set={sets.find((c) => c.code === s.setCode)} />
                <span className="reserve-qty">×{s.quantity}</span>
                <span className="reserve-label">
                  {s.setName}
                  <small>{kindLabel(t, s.kind)}</small>
                </span>
              </button>
            ))}
            <button className="reserve-cell more" onClick={onGoShopping}>
              <span className="wrapper empty" aria-hidden="true" />
              <span className="reserve-label">
                {t("open.moreBoosters")}
                <small>{t("open.goShopping")}</small>
              </span>
            </button>
          </div>
          <BackToTop target={list} />
        </div>
      </section>

      <section className="frame ready">
        <div className="cartouche">{t("open.ready")}</div>
        <div className="scroll">
          <div className="ready-pack">
            <Wrapper stack={current} set={set} large />
          </div>
          <div className="ready-name">
            <span className="medallion">
              <SetIcon url={set?.iconUrl ?? null} name={current.setName} />
            </span>
            {current.setName}
          </div>
          <div className="meta" style={{ textAlign: "center" }}>
            {kindLabel(t, current.kind)} · {t("open.inReserve", { count: current.quantity })}
          </div>
          {/* Scryfall: an art crop is shown unaltered, with its artist credited next to it. */}
          {set?.art && (
            <div className="shop-credit" style={{ textAlign: "center" }}>
              {t("art.credit", { card: set.art.cardName, artist: set.art.artist })}
            </div>
          )}
          <div className="gild-divider" />
          {current.topper ? (
            // Given with a box of boosters: no price, never sold (its cards can be).
            <p className="ready-topper">{t("topper.reserveLine")}</p>
          ) : (
            <>
              <div className="ready-line">
                <span>{tx("open.paid", { amount: money(current.costCents) })}</span>
                <span>
                  {tx("open.worth", { amount: money(current.quantity * current.sellValueCents) })}{" "}
                  <span className={pnl >= 0 ? "gain" : "loss"}>
                    {pnl >= 0 ? "+" : "−"}
                    <Money cents={Math.abs(pnl)} />
                  </span>
                </span>
              </div>
              <div className="ready-line">
                <PriceTrend history={current.history} />
              </div>
            </>
          )}
          {current.boxProgress && <BoxGauge progress={current.boxProgress} />}
        </div>
        <div className="buy-bar">
          <button className="big wax" disabled={busy} onClick={() => onOpen(current)}>
            {t("open.openOne")}
          </button>
          <div className="ready-more">
            <button disabled={busy} onClick={() => onOpenQuick(current)} title={t("open.quickHint")}>
              {t("open.quick")}
            </button>
            {BATCH_SIZES.filter((n) => current.quantity >= n).map((n) => (
              <button key={n} disabled={busy} onClick={() => onOpenMany(current, n)}>
                {t("open.openMany", { count: n })}
              </button>
            ))}
            {!current.topper && (
              <button disabled={busy} onClick={() => onSell(current)}>
                {tx("open.sellOne", { amount: money(current.sellValueCents) })}
              </button>
            )}
          </div>
        </div>
      </section>
    </div>
  );
}
