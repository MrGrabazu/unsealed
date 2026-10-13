import { useEffect, useState } from "react";
import { getDataInfo, type DataInfo } from "./api";
import { DataStatusBar, useDataStatus } from "./DataStatus";
import { LANGUAGES, useI18n, type Lang } from "./i18n";
import { Loading } from "./Loading";

/** Settings: the language, and what the downloaded game data contains. */
export function Options({ onBack }: { onBack: () => void }) {
  const { t, lang, setLang, formatDate, formatNumber } = useI18n();
  const [info, setInfo] = useState<DataInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const reload = () =>
    getDataInfo()
      .then(setInfo)
      .catch((e) => setError(String(e)));
  const dataStatus = useDataStatus(reload);

  useEffect(() => {
    void reload();
  }, []);

  return (
    <div className="page">
      <div className="controls">
        <button onClick={onBack}>{t("common.back")}</button>
      </div>
      <h2>{t("options.title")}</h2>

      <h3>{t("options.language")}</h3>
      <select value={lang} onChange={(e) => setLang(e.target.value as Lang)} aria-label={t("options.language")}>
        {LANGUAGES.map((l) => (
          <option key={l.code} value={l.code}>
            {l.name}
          </option>
        ))}
      </select>
      <p className="hint">{t("options.languageHint")}</p>

      <h3>{t("options.data")}</h3>
      <DataStatusBar status={dataStatus} />
      {error && <p className="error">{error}</p>}
      {!info && !error && <Loading />}
      {info && (
        <dl className="info-list">
          <dt>{t("options.sets")}</dt>
          <dd>{formatNumber(info.setCount)}</dd>
          <dt>{t("options.boosterTypes")}</dt>
          <dd>{formatNumber(info.boosterCount)}</dd>
          <dt>{t("options.pricesFrom")}</dt>
          <dd>
            {info.pricesUpdatedAt ? formatDate(Date.parse(info.pricesUpdatedAt) / 1000) : t("common.none")}
          </dd>
          <dt>{t("options.lastUpdate")}</dt>
          <dd>{info.builtAt ? formatDate(info.builtAt) : t("options.never")}</dd>
        </dl>
      )}
      <p className="hint">{t("options.dataHint")}</p>

      <h3>{t("options.more")}</h3>
      <p className="hint">{t("options.moreHint")}</p>
    </div>
  );
}
