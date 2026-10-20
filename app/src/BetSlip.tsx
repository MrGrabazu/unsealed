import { useContext, useEffect, useMemo, useState } from "react";
import {
  getBetChance,
  getBetContext,
  getCollection,
  type Bet,
  type BetChance,
  type BetContext,
  type BetRequest,
  type CardFilter,
  type CardKind,
  type Clause,
  type CollectionCard,
  type Compare,
  type Finish,
  type KeptBet,
  type Treatment,
} from "./api";
import { betKey, CARD_TYPES, COLORS, describeBet, formatChance, formatOdds, namedCard, oddsOf, RARITIES } from "./bets";
import { useI18n } from "./i18n";
import type { MessageKey } from "./i18n/en";
import { CurrencyContext, Money, useMoneyText } from "./money";

interface Props {
  setCode: string;
  kind: string;
  /** The bets placed on this booster (taken when it is torn open). */
  bets: BetRequest[];
  onBets: (bets: BetRequest[]) => void;
  /** The bets kept from one opening to the next: a bet of the slip can be kept. */
  kept: KeptBet[];
  onKept: (bets: KeptBet[]) => void;
  /** Names a card the slip knows (for the results of a bet on it). */
  onCardName: (uuid: string, name: string) => void;
  balanceCents: number;
  /** In the opening screen's side panel: always unfolded, a new bet built on demand. */
  panel?: boolean;
  /** The pack is torn: the bets are placed, only shown. */
  locked?: boolean;
}

const newCardsClause = (): Clause => ({ type: "cards", compare: "atLeast", count: 1, filter: {} });

/** Bets on the booster about to be opened. Each is built from conditions that must all hold:
 * on its cards (how many, of which rarity, type, colour, finish, treatment, price, or a given
 * card) or on its value, at odds from the real chances. As many bets as wanted; any can be
 * kept for the next boosters. Folded until opened. */
export function BetSlip({ setCode, kind, bets, onBets, kept, onKept, onCardName, balanceCents, panel = false, locked = false }: Props) {
  const { t, plural, formatNumber } = useI18n();
  const moneyText = useMoneyText();
  // Folded at first: the booster is what the screen shows; the folded slip still says how
  // many bets are placed and what they stake. The side panel is always unfolded.
  const [folded, setFolded] = useState(true);
  const open = panel || !folded;
  // In the panel: the builder of a new bet, shown on demand.
  const [building, setBuilding] = useState(false);
  const [context, setContext] = useState<BetContext | null>(null);
  const [cards, setCards] = useState<CollectionCard[]>([]);
  const [error, setError] = useState<string | null>(null);
  // Chances of the bets of the slip, by key.
  const [chances, setChances] = useState<Record<string, BetChance>>({});

  useEffect(() => {
    if (!open || context) return;
    getBetContext(setCode, kind)
      .then(setContext)
      .catch((e) => setError(String(e)));
    getCollection(setCode)
      .then((v) => setCards([...v.cards, ...v.extras, ...v.tokens, ...v.arts]))
      .catch(() => {});
  }, [open, setCode, kind, context]);

  const byUuid = useMemo(() => new Map(cards.map((c) => [c.uuid, c])), [cards]);
  const cardName = (uuid: string) => {
    const c = byUuid.get(uuid);
    return c ? `${c.name} (#${c.number})` : (kept.find((k) => namedCard(k.bet) === uuid)?.cardName ?? "?");
  };
  const describe = (bet: Bet) => describeBet(bet, { t, plural }, moneyText, cardName);

  // The chances of the bets that came with the slip (kept ones) or were just added.
  useEffect(() => {
    if (!open) return;
    for (const { bet } of bets) {
      const key = betKey(bet);
      if (key in chances) continue;
      getBetChance(setCode, kind, bet)
        .then((c) => setChances((all) => ({ ...all, [key]: c })))
        .catch(() => {});
    }
  }, [open, bets, setCode, kind]);

  const margin = context?.houseMarginPercent ?? 0;
  const isKept = (bet: Bet) => kept.some((k) => betKey(k.bet) === betKey(bet) && (k.setCode == null || k.setCode === setCode));

  function add(bet: Bet, stakeCents: number, keep: boolean, chance: BetChance) {
    if (bets.some((b) => betKey(b.bet) === betKey(bet))) return;
    setChances((all) => ({ ...all, [betKey(bet)]: chance }));
    const card = namedCard(bet);
    if (card) onCardName(card, cardName(card));
    onBets([...bets, { bet, stakeCents }]);
    if (keep) setKeep({ bet, stakeCents }, true);
    setBuilding(false);
  }

  function setKeep(request: BetRequest, on: boolean) {
    const others = kept.filter((k) => !(betKey(k.bet) === betKey(request.bet) && (k.setCode == null || k.setCode === setCode)));
    const card = namedCard(request.bet);
    const keep: KeptBet = {
      bet: request.bet,
      stakeCents: request.stakeCents,
      active: request.active !== false,
      setCode: card ? setCode : null,
      cardName: card ? cardName(card) : null,
    };
    onKept(on ? [...others, keep] : others);
  }

  /** Pauses a bet, or plays it again: kept in the slip (and in the kept bets), its stake too. */
  function setActive(request: BetRequest, active: boolean) {
    const key = betKey(request.bet);
    onBets(bets.map((b) => (betKey(b.bet) === key ? { ...b, active } : b)));
    if (isKept(request.bet)) onKept(kept.map((k) => (betKey(k.bet) === key ? { ...k, active } : k)));
  }

  function setStake(request: BetRequest, stakeCents: number) {
    const key = betKey(request.bet);
    onBets(bets.map((b) => (betKey(b.bet) === key ? { ...b, stakeCents } : b)));
    if (isKept(request.bet)) onKept(kept.map((k) => (betKey(k.bet) === key ? { ...k, stakeCents } : k)));
  }

  // What the active bets stake (the paused ones are not placed).
  const live = bets.filter((b) => b.active !== false);
  const total = live.reduce((sum, b) => sum + b.stakeCents, 0);

  if (panel) {
    const shown = locked ? live : bets;
    return (
      <div className={`bet-panel ${locked ? "locked" : ""}`}>
        {!locked && <p className="hint">{margin === 0 ? t("bet.introFair") : t("bet.introMargin", { percent: margin })}</p>}
        {error && <p className="error">{error}</p>}
        {shown.length === 0 && <p className="hint">{t(locked ? "bet.emptyLocked" : "bet.empty")}</p>}
        <ul className="bet-cards">
          {shown.map((request) => {
            const chance = chances[betKey(request.bet)];
            const odds = chance?.probability ? oddsOf(chance.probability, margin) : null;
            const active = request.active !== false;
            return (
              <li key={betKey(request.bet)} className={active ? "" : "paused"}>
                {!locked && (
                  <label className="bet-switch" title={t(active ? "bet.pauseHint" : "bet.playHint")}>
                    <input type="checkbox" checked={active} onChange={(e) => setActive(request, e.target.checked)} />
                    <span />
                  </label>
                )}
                <span className="bet-card-name">
                  {describe(request.bet)}
                  {locked && isKept(request.bet) && <span className="bet-pin"> 📌</span>}
                </span>
                <span className="bet-odds">
                  {chance === undefined ? "…" : odds == null ? t("bet.unpriced") : formatOdds(odds, formatNumber)}
                </span>
                <span className="bet-card-sub">
                  {locked ? (
                    <>
                      {t("bet.stake")} <Money cents={request.stakeCents} />
                    </>
                  ) : (
                    <StakeInput value={request.stakeCents} onChange={(c) => setStake(request, c)} />
                  )}
                  {odds != null && (
                    <>
                      {" "}
                      → <Money cents={Math.floor(request.stakeCents * odds)} />
                    </>
                  )}
                  {chance?.probability != null && <> · {formatNumber(Math.round(chance.probability * 1000) / 10)} %</>}
                </span>
                {!locked && (
                  <span className="bet-card-tools">
                    <label className="bet-keep" title={t("bet.keepHint")}>
                      <input type="checkbox" checked={isKept(request.bet)} onChange={(e) => setKeep(request, e.target.checked)} />
                      📌
                    </label>
                    <button
                      className="bet-remove"
                      onClick={() => onBets(bets.filter((b) => betKey(b.bet) !== betKey(request.bet)))}
                      aria-label={t("bet.remove")}
                      title={t("bet.remove")}
                    >
                      ×
                    </button>
                  </span>
                )}
              </li>
            );
          })}
        </ul>
        {!locked && total > balanceCents && <p className="hint">{t("bet.tooMuch")}</p>}
        {!locked &&
          (building && context ? (
            <div className="bet-building">
              <BetBuilder setCode={setCode} kind={kind} context={context} cards={cards} cardName={cardName} describe={describe} onAdd={add} />
              <button onClick={() => setBuilding(false)}>{t("common.cancel")}</button>
            </div>
          ) : (
            <button className="bet-add" onClick={() => setBuilding(true)} disabled={!context}>
              + {t("bet.new")}
            </button>
          ))}
        <div className="bet-total">
          <span>{plural("bet.count", live.length)}</span>
          <b>
            <Money cents={total} />
          </b>
        </div>
      </div>
    );
  }

  return (
    <section className={`bet-slip ${open ? "open" : ""}`}>
      <button className="bet-slip-toggle" onClick={() => setFolded((f) => !f)} aria-expanded={open}>
        <span>🎲 {t("bet.title")}</span>
        {bets.length > 0 && (
          <span className="hint">
            {plural("bet.count", live.length)} · <Money cents={total} />
          </span>
        )}
        <span className="shop-chevron">{open ? "▾" : "▸"}</span>
      </button>
      {open && (
        <div className="bet-slip-body">
          <p className="hint">{margin === 0 ? t("bet.introFair") : t("bet.introMargin", { percent: margin })}</p>
          {error && <p className="error">{error}</p>}
          {bets.length > 0 && (
            <ul className="bet-lines">
              {bets.map((request) => {
                const chance = chances[betKey(request.bet)];
                const odds = chance?.probability ? oddsOf(chance.probability, margin) : null;
                const active = request.active !== false;
                return (
                  <li key={betKey(request.bet)} className={active ? "" : "paused"}>
                    <label className="bet-active" title={t(active ? "bet.pauseHint" : "bet.playHint")}>
                      <input type="checkbox" checked={active} onChange={(e) => setActive(request, e.target.checked)} />
                    </label>
                    <span className="bet-line-name">{describe(request.bet)}</span>
                    <span className="hint">
                      {chance === undefined ? "…" : odds == null ? t("bet.unpriced") : formatOdds(odds, formatNumber)}
                    </span>
                    <StakeInput value={request.stakeCents} onChange={(c) => setStake(request, c)} />
                    <span className="hint">{odds != null && <>→ <Money cents={Math.floor(request.stakeCents * odds)} /></>}</span>
                    <label className="bet-keep" title={t("bet.keepHint")}>
                      <input type="checkbox" checked={isKept(request.bet)} onChange={(e) => setKeep(request, e.target.checked)} />
                      📌
                    </label>
                    <button
                      className="bet-remove"
                      onClick={() => onBets(bets.filter((b) => betKey(b.bet) !== betKey(request.bet)))}
                      aria-label={t("bet.remove")}
                      title={t("bet.remove")}
                    >
                      ×
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
          {total > balanceCents && <p className="hint">{t("bet.tooMuch")}</p>}
          {context && (
            <BetBuilder
              setCode={setCode}
              kind={kind}
              context={context}
              cards={cards}
              cardName={cardName}
              describe={describe}
              onAdd={add}
            />
          )}
        </div>
      )}
    </section>
  );
}

/** A stake, typed in Seals. */
function StakeInput({ value, onChange }: { value: number; onChange: (cents: number) => void }) {
  const { t } = useI18n();
  const { sealsPerCent } = useContext(CurrencyContext);
  return (
    <label className="bet-stake">
      {t("bet.stake")}
      <input
        type="number"
        min={1}
        value={Math.round(value * sealsPerCent)}
        onChange={(e) => onChange(Math.max(0, Math.round(Number(e.target.value) / sealsPerCent)))}
      />
    </label>
  );
}

/** Builds one bet, condition by condition, its odds worked out as it is written. */
function BetBuilder({
  setCode,
  kind,
  context,
  cards,
  cardName,
  describe,
  onAdd,
}: {
  setCode: string;
  kind: string;
  context: BetContext;
  cards: CollectionCard[];
  cardName: (uuid: string) => string;
  describe: (bet: Bet) => string;
  onAdd: (bet: Bet, stakeCents: number, keep: boolean, chance: BetChance) => void;
}) {
  const { t, formatNumber } = useI18n();
  const [clauses, setClauses] = useState<Clause[]>([newCardsClause()]);
  // Bumped when a bet is placed: the editors start afresh (the card number field included).
  const [round, setRound] = useState(0);
  const [stake, setStake] = useState(Math.max(context.minStakeCents, 100));
  const [keep, setKeep] = useState(false);
  const [chance, setChance] = useState<BetChance | null>(null);
  const [busy, setBusy] = useState(false);
  const bet: Bet = useMemo(() => ({ clauses }), [clauses]);

  // The odds follow the bet as it is written (a little after the last change).
  useEffect(() => {
    setChance(null);
    setBusy(true);
    let live = true;
    const timer = setTimeout(() => {
      getBetChance(setCode, kind, bet)
        .then((c) => live && setChance(c))
        .catch(() => live && setChance({ probability: null, exact: false }))
        .finally(() => live && setBusy(false));
    }, 250);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [bet, setCode, kind]);

  const update = (i: number, clause: Clause) => setClauses((all) => all.map((c, j) => (j === i ? clause : c)));
  const remove = (i: number) => setClauses((all) => (all.length > 1 ? all.filter((_, j) => j !== i) : all));
  const odds = chance?.probability ? oddsOf(chance.probability, context.houseMarginPercent) : null;

  return (
    <div className="bet-builder">
      <h4>{t("bet.newBet")}</h4>
      {clauses.map((clause, i) => (
        <div key={`${round}-${i}`} className="bet-clause">
          {i > 0 && <span className="bet-and">{t("bet.and")}</span>}
          <ClauseEditor clause={clause} onChange={(c) => update(i, c)} cards={cards} cardName={cardName} context={context} />
          {clauses.length > 1 && (
            <button className="bet-remove" onClick={() => remove(i)} aria-label={t("bet.remove")} title={t("bet.remove")}>
              ×
            </button>
          )}
        </div>
      ))}
      <div className="bet-builder-more">
        <button onClick={() => setClauses((all) => [...all, newCardsClause()])}>{t("bet.addCards")}</button>
        <button
          onClick={() => setClauses((all) => [...all, { type: "value", compare: "atLeast", cents: context.boosterPriceCents }])}
        >
          {t("bet.addValue")}
        </button>
      </div>
      <p className="bet-builder-summary">{describe(bet)}</p>
      <div className="bet-builder-odds">
        <span>
          {busy
            ? t("bet.computing")
            : chance?.probability
              ? `${formatChance(chance.probability, t, formatNumber)} · ${chance.exact ? t("bet.exact") : t("bet.simulated")}`
              : t("bet.unpricedLong")}
        </span>
        {odds != null && <strong>{formatOdds(odds, formatNumber)}</strong>}
        <StakeInput value={stake} onChange={setStake} />
        {odds != null && (
          <span className="hint">
            → <Money cents={Math.floor(stake * odds)} />
          </span>
        )}
        <label className="bet-keep" title={t("bet.keepHint")}>
          <input type="checkbox" checked={keep} onChange={(e) => setKeep(e.target.checked)} />📌 {t("bet.keep")}
        </label>
        <button
          className="primary"
          disabled={!chance?.probability || stake < context.minStakeCents}
          onClick={() => {
            onAdd(bet, stake, keep, chance!);
            setClauses([newCardsClause()]);
            setRound((r) => r + 1);
            setKeep(false);
          }}
        >
          {t("bet.place")}
        </button>
      </div>
    </div>
  );
}

/** Chips to pick any of a few values (none picked: any). */
function Chips<T extends string>({ values, picked, label, onChange }: { values: readonly T[]; picked: T[]; label: (v: T) => string; onChange: (p: T[]) => void }) {
  return (
    <span className="bet-chips">
      {values.map((v) => (
        <button
          key={v}
          className={`bet-chip ${picked.includes(v) ? "active" : ""}`}
          onClick={() => onChange(picked.includes(v) ? picked.filter((x) => x !== v) : [...picked, v])}
        >
          {label(v)}
        </button>
      ))}
    </span>
  );
}

/** One condition: on the booster's cards, or on its value. */
function ClauseEditor({
  clause,
  onChange,
  cards,
  cardName,
  context,
}: {
  clause: Clause;
  onChange: (c: Clause) => void;
  cards: CollectionCard[];
  cardName: (uuid: string) => string;
  context: BetContext;
}) {
  const { t } = useI18n();
  const { sealsPerCent } = useContext(CurrencyContext);
  const toSeals = (cents: number | null | undefined) => (cents == null ? "" : String(Math.round(cents * sealsPerCent)));
  const toCents = (text: string) => (text.trim() === "" ? null : Math.max(0, Math.round(Number(text) / sealsPerCent)));

  if (clause.type === "value") {
    return (
      <div className="bet-clause-body">
        <span>{t("bet.boosterValue")}</span>
        <select value={clause.compare} onChange={(e) => onChange({ ...clause, compare: e.target.value as Compare })}>
          <option value="atLeast">{t("bet.atLeastShort")}</option>
          <option value="atMost">{t("bet.atMostShort")}</option>
        </select>
        <input
          type="number"
          min={0}
          className="bet-amount"
          value={toSeals(clause.cents)}
          onChange={(e) => onChange({ ...clause, cents: toCents(e.target.value) ?? 0 })}
        />
        <button onClick={() => onChange({ ...clause, compare: "atLeast", cents: context.boosterPriceCents })}>{t("bet.profitable")}</button>
        <button onClick={() => onChange({ ...clause, compare: "atMost", cents: Math.max(0, context.boosterPriceCents - 1) })}>
          {t("bet.losing")}
        </button>
      </div>
    );
  }

  const f = clause.filter;
  const setFilter = (patch: Partial<CardFilter>) => onChange({ ...clause, filter: { ...f, ...patch } });
  // "None": at most 0.
  const mode = clause.compare !== "atLeast" && clause.count === 0 ? "none" : clause.compare;
  const named = f.card ? cards.find((c) => c.uuid === f.card) : undefined;

  return (
    <div className="bet-clause-body">
      <div className="bet-row">
        <select
          value={mode}
          onChange={(e) => {
            const v = e.target.value;
            if (v === "none") onChange({ ...clause, compare: "atMost", count: 0 });
            else onChange({ ...clause, compare: v as Compare, count: Math.max(clause.count, v === "atLeast" ? 1 : clause.count) });
          }}
        >
          <option value="atLeast">{t("bet.atLeastShort")}</option>
          <option value="exactly">{t("bet.exactlyShort")}</option>
          <option value="atMost">{t("bet.atMostShort")}</option>
          <option value="none">{t("bet.noneShort")}</option>
        </select>
        {mode !== "none" && (
          <input
            type="number"
            min={0}
            max={30}
            className="bet-count"
            value={clause.count}
            onChange={(e) => onChange({ ...clause, count: Math.max(0, Math.min(30, Math.round(Number(e.target.value)))) })}
          />
        )}
        <select value={f.kind ?? "cards"} onChange={(e) => setFilter({ kind: e.target.value as CardKind })} disabled={!!f.card}>
          {(["cards", "tokens", "artCards", "all"] as CardKind[]).map((k) => (
            <option key={k} value={k}>
              {t(`bet.kind.${k}` as MessageKey)}
            </option>
          ))}
        </select>
        <select value={f.finish ?? "any"} onChange={(e) => setFilter({ finish: e.target.value as Finish })}>
          <option value="any">{t("bet.finishAny")}</option>
          <option value="foil">{t("bet.finish.foil")}</option>
          <option value="normal">{t("bet.finish.normal")}</option>
        </select>
        <select value={f.treatment ?? "any"} onChange={(e) => setFilter({ treatment: e.target.value as Treatment })}>
          <option value="any">{t("bet.treatmentAny")}</option>
          <option value="fullArt">{t("bet.treatment.fullArt")}</option>
          <option value="specialFrame">{t("bet.treatment.specialFrame")}</option>
        </select>
      </div>
      <div className="bet-row">
        <span className="bet-label">{t("bet.rarities")}</span>
        <Chips values={RARITIES} picked={f.rarities ?? []} label={(r) => t(`bet.rarity.${r}` as MessageKey)} onChange={(rarities) => setFilter({ rarities })} />
      </div>
      <div className="bet-row">
        <span className="bet-label">{t("bet.types")}</span>
        <Chips values={CARD_TYPES} picked={f.types ?? []} label={(ty) => t(`bet.type.${ty}` as MessageKey)} onChange={(types) => setFilter({ types })} />
      </div>
      <div className="bet-row">
        <span className="bet-label">{t("bet.colors")}</span>
        <Chips values={COLORS} picked={f.colors ?? []} label={(c) => t(`bet.color.${c}` as MessageKey)} onChange={(colors) => setFilter({ colors })} />
      </div>
      <div className="bet-row">
        <span className="bet-label">{t("bet.price")}</span>
        <input
          type="number"
          min={0}
          className="bet-amount"
          placeholder={t("bet.min")}
          value={toSeals(f.minCents)}
          onChange={(e) => setFilter({ minCents: toCents(e.target.value) })}
        />
        <input
          type="number"
          min={0}
          className="bet-amount"
          placeholder={t("bet.max")}
          value={toSeals(f.maxCents)}
          onChange={(e) => setFilter({ maxCents: toCents(e.target.value) })}
        />
        <span className="bet-label">{t("bet.givenCard")}</span>
        <input
          className="bet-count"
          placeholder={t("bet.cardNumber")}
          defaultValue={named?.number ?? ""}
          onChange={(e) => {
            const number = e.target.value.trim().toLowerCase();
            const card = cards.find((c) => c.number.toLowerCase() === number);
            setFilter({ card: card?.uuid ?? null });
          }}
        />
        {f.card && <span className="hint">{cardName(f.card)}</span>}
      </div>
    </div>
  );
}
