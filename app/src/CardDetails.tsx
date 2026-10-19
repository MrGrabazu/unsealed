import { useEffect, useRef, useState } from "react";
import { getCardDetails, type CardDetailsView } from "./api";
import { BackToTop } from "./BackToTop";
import { CardImage } from "./CardImage";
import { largeImage } from "./images";
import { CARD_LANGUAGES, useI18n, type I18n } from "./i18n";
import { Loading } from "./Loading";
import { ManaText } from "./ManaText";
import { Money, useMoney } from "./money";

interface Props {
  setCode: string;
  uuid: string;
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
export function CardDetails({ setCode, uuid, onClose }: Props) {
  const { t, tx, cardLang } = useI18n();
  const money = useMoney();
  const [details, setDetails] = useState<CardDetailsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
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

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div ref={box} className="modal details" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <button className="modal-close" onClick={onClose} aria-label={t("details.close")}>
          ×
        </button>
        {error && <p className="error">{error}</p>}
        {!details && !error && <Loading label={t("details.loading")} />}
        {details && (
          <div className="details-layout">
            <div className="details-image">
              {/* Shown large: the sharper image. */}
              <CardImage front={largeImage(details.imageUrl)} back={largeImage(details.backImageUrl)} alt={details.name} />
            </div>

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
                    <dt>{t("details.marketFoil")}</dt>
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
                <div className="printing-grid">
                  {details.printings.map((p) => (
                    <div key={`${p.setCode}-${p.number}`} className={`printing ${p.current ? "current" : ""}`}>
                      {p.imageUrl && <img src={p.imageUrl} alt={p.setName} loading="lazy" draggable={false} />}
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
              </div>
            )}
          </div>
        )}
        <BackToTop target={box} />
      </div>
    </div>
  );
}
