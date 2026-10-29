import { useEffect, useRef, useState } from "react";
import { getCardDetails, type CardDetailsView } from "./api";
import { Backdrop } from "./Backdrop";
import { BackToTop } from "./BackToTop";
import { CardImage, NearImage } from "./CardImage";
import { foilLabel } from "./foils";
import { largeImage } from "./images";
import { CARD_LANGUAGES, useI18n, type I18n } from "./i18n";
import { Loading } from "./Loading";
import { ManaText } from "./ManaText";
import { Money, useMoney, useMoneyText } from "./money";
import { drawCardShare, type CardShare } from "./sharePicture";
import { ShareButton } from "./Share";

interface Props {
  setCode: string;
  uuid: string;
  /** The finish it was pulled in, opened from a booster: the one its shared picture shows
   * (otherwise foil when a foil copy is owned). */
  foil?: boolean;
  onClose: () => void;
}

const rarityName = (t: I18n["t"], rarity: string) => {
  switch (rarity) {
    case "common":
      return t("rarityName.common");
    case "uncommon":
      return t("rarityName.uncommon");
    case "rare":
      return t("rarityName.rare");
    case "mythic":
      return t("rarityName.mythic");
    case "token":
      return t("rarityName.token");
    case "art":
      return t("rarityName.art");
    default:
      return t("rarityName.special");
  }
};

/** A card in detail: large image (turned over for double-faced cards), rules text, prices,
 * copies owned and every other printing. */
export function CardDetails({ setCode, uuid, foil, onClose }: Props) {
  const { t, tx, cardLang } = useI18n();
  const money = useMoney();
  const moneyText = useMoneyText();
  const [details, setDetails] = useState<CardDetailsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const printingList = useRef<HTMLDivElement>(null);
  // A card printed sideways, turned to be read (its front only: a battle's back is upright).
  const [turned, setTurned] = useState(false);
  const [backShown, setBackShown] = useState(false);
  const reading = backShown ? undefined : details?.reading;

  useEffect(() => {
    setTurned(false);
    setBackShown(false);
    getCardDetails(setCode, uuid)
      .then(setDetails)
      .catch((e) => setError(String(e)));
  }, [setCode, uuid]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const price = (cents: number | null) =>
    cents == null ? t("common.none") : <Money cents={cents} estimated={details?.priceEstimated} />;

  /** The picture to share: the card in the finish it was pulled in (or the finest owned). */
  const cardShare = (d: CardDetailsView): CardShare => {
    const shownFoil = d.canBeFoil && (foil ?? (d.ownedFoil > 0 || !d.canBeNonfoil));
    const cents = shownFoil ? (d.foilPriceCents ?? d.priceCents) : (d.priceCents ?? d.foilPriceCents);
    return {
      card: {
        imageUrl: largeImage(d.imageUrl),
        name: d.name,
        foil: shownFoil,
        foilLabel: shownFoil ? (foilLabel(d.foilKind) ?? t(d.rarity === "art" ? "collection.signedBadge" : "collection.foilBadge")) : null,
        newLabel: null,
      },
      subtitle: t("details.set", { set: d.setName, number: d.number, rarity: rarityName(t, d.rarity) }),
      priceLabel: t("share.marketValue"),
      priceText: cents == null ? null : `${d.priceEstimated ? "≈ " : ""}${moneyText(cents)}`,
      tagline: t("share.tagline"),
    };
  };

  return (
    <Backdrop onClose={onClose}>
      <div className="modal details" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <button className="modal-close" onClick={onClose} aria-label={t("details.close")}>
          ×
        </button>
        {error && <p className="error">{error}</p>}
        {!details && !error && <Loading label={t("details.loading")} />}
        {details && (
          <div className={`details-layout ${reading && turned ? `turned-${reading}` : ""}`}>
            <div className="details-image-col">
            <div className="details-image">
              {/* Shown large: the sharper image. */}
              <CardImage
                front={largeImage(details.imageUrl)}
                back={largeImage(details.backImageUrl)}
                alt={details.name}
                onTurned={(back) => {
                  setBackShown(back);
                  setTurned(false);
                }}
              />
              {reading && (
                <button
                  type="button"
                  className={`rotate-button ${turned ? "active" : ""}`}
                  title={t(turned ? "details.unrotate" : "details.rotate")}
                  aria-label={t(turned ? "details.unrotate" : "details.rotate")}
                  aria-pressed={turned}
                  onClick={() => setTurned((on) => !on)}
                >
                  {reading === "flip" ? "↻" : reading === "left" ? "⤺" : "⤻"}
                </button>
              )}
            </div>
            {/* Under the image, in the room it leaves. */}
            <ShareButton make={() => drawCardShare(cardShare(details))} fileName={details.name} />
            </div>

            {/* The text, then the printings, which scroll on their own: the sheet never
                goes beyond the window, even for a basic land and its hundreds of printings. */}
            <div className="details-side">
            <div className="details-info">
              <h2>{details.name}</h2>
              <p className="hint">
                {t("details.set", {
                  set: details.setName,
                  number: details.number,
                  rarity: rarityName(t, details.rarity),
                })}
              </p>

              {details.shownLanguage !== cardLang && (
                <p className="hint">
                  {t("details.notPrintedIn", {
                    language: CARD_LANGUAGES.find((l) => l.code === cardLang)?.name ?? cardLang,
                  })}
                </p>
              )}
              {!details.online && <p className="hint">{t("details.offline")}</p>}
              {details.faces.map((face, i) => (
                <div key={i} className="details-face">
                  {details.faces.length > 1 && (
                    <h3>
                      {face.name}
                      {face.manaCost && (
                        <span className="details-cost">
                          <ManaText text={face.manaCost} />
                        </span>
                      )}
                    </h3>
                  )}
                  {details.faces.length === 1 && face.manaCost && (
                    <p className="details-cost">
                      <ManaText text={face.manaCost} />
                    </p>
                  )}
                  {face.typeLine && <p className="details-type">{face.typeLine}</p>}
                  {face.text?.split("\n").map((line, j) => (
                    <p key={j} className="details-text">
                      <ManaText text={line} />
                    </p>
                  ))}
                  {face.flavorText && <p className="details-flavor">{face.flavorText}</p>}
                  {face.stats && <p className="details-stats">{face.stats}</p>}
                </div>
              ))}
              {details.artist && <p className="hint">{t("details.artist", { artist: details.artist })}</p>}
              {details.priceEstimated && <p className="hint">{t("details.estimated")}</p>}

              <dl className="info-list">
                {details.canBeNonfoil && (
                  <>
                    <dt>{t("details.market")}</dt>
                    <dd>
                      {price(details.priceCents)}
                      {details.sellValueCents > 0 && (
                        <span className="hint"> · {tx("details.buys", { amount: money(details.sellValueCents) })}</span>
                      )}
                    </dd>
                  </>
                )}
                {details.canBeFoil && (
                  <>
                    {/* A special foil is named (galaxy, etched…): it is the only foil of this printing. */}
                    <dt>{foilLabel(details.foilKind) ?? t("details.marketFoil")}</dt>
                    <dd>
                      {price(details.foilPriceCents)}
                      {details.foilSellValueCents > 0 && (
                        <span className="hint">
                          {" "}
                          · {tx("details.buys", { amount: money(details.foilSellValueCents) })}
                        </span>
                      )}
                    </dd>
                  </>
                )}
                <dt>{t("details.owned")}</dt>
                <dd>{t("details.ownedCount", { normal: details.owned, foil: details.ownedFoil })}</dd>
              </dl>
            </div>

            {details.printings.length > 1 && (
              <div className="details-printings">
                <h3>{t("details.printings")}</h3>
                <p className="hint">{t("details.printingsHint")}</p>
                <div className="printing-list" ref={printingList}>
                <div className="printing-grid">
                  {details.printings.map((p) => (
                    <div key={`${p.setCode}-${p.number}`} className={`printing ${p.current ? "current" : ""}`}>
                      {p.imageUrl && <NearImage src={p.imageUrl} alt={p.setName} />}
                      <div className="printing-info">
                        <strong>{p.setName}</strong>
                        <span className="hint">
                          {p.setCode} · #{p.number} · {p.releasedAt?.slice(0, 4)}
                        </span>
                        <span>
                          {price(p.priceCents)}
                          {p.foilPriceCents != null && (
                            <span className="hint"> · {tx("details.foilPrice", { amount: money(p.foilPriceCents) })}</span>
                          )}
                        </span>
                        <span className="printing-tags">
                          {p.current && <span className="tag accent">{t("details.thisOne")}</span>}
                          {p.inGame && <span className="tag">{t("details.inGame")}</span>}
                          {p.owned + p.ownedFoil > 0 && (
                            <span className="tag gain">{t("details.ownedShort", { count: p.owned + p.ownedFoil })}</span>
                          )}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
                <BackToTop target={printingList} />
                </div>
              </div>
            )}
            </div>
          </div>
        )}
      </div>
    </Backdrop>
  );
}
