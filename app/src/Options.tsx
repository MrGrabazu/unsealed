import { useEffect, useState } from "react";
import { getDataInfo, type DataInfo } from "./api";
import { DataStatusBar, useDataStatus } from "./DataStatus";
import { CARD_LANGUAGES, LANGUAGES, useI18n, type CardLangSetting, type Lang } from "./i18n";
import { Loading } from "./Loading";
import { getFullscreen, setFullscreen } from "./fullscreen";
import { getOpening3d, setOpening3d } from "./settings";
import { getMusicOn, getMusicVolume, refreshMusic, setMusicOn, setMusicVolume } from "./music";
import { getSoundOn, getVolume, setSoundOn, setVolume, sounds } from "./sound";

/** Settings: the language, and what the downloaded game data contains. */
export function Options({ onBack }: { onBack: () => void }) {
  const { t, lang, setLang, cardLangSetting, setCardLangSetting, formatDate, formatNumber } = useI18n();
  const [info, setInfo] = useState<DataInfo | null>(null);
  const [opening3d, setOpening3dState] = useState(getOpening3d);
  const [fullscreen, setFullscreenState] = useState(getFullscreen);
  const [soundOn, setSoundOnState] = useState(getSoundOn);
  const [volume, setVolumeState] = useState(getVolume);
  const [musicOn, setMusicOnState] = useState(getMusicOn);
  const [musicVolume, setMusicVolumeState] = useState(getMusicVolume);
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

      <h3>{t("options.cardLanguage")}</h3>
      <select
        value={cardLangSetting}
        onChange={(e) => setCardLangSetting(e.target.value as CardLangSetting)}
        aria-label={t("options.cardLanguage")}
      >
        <option value="auto">{t("options.cardLanguageAuto")}</option>
        {CARD_LANGUAGES.map((l) => (
          <option key={l.code} value={l.code}>
            {l.name}
          </option>
        ))}
      </select>
      <p className="hint">{t("options.cardLanguageHint")}</p>

      <h3>{t("options.display")}</h3>
      <select
        value={fullscreen ? "fullscreen" : "windowed"}
        onChange={(e) => {
          const on = e.target.value === "fullscreen";
          setFullscreenState(on);
          void setFullscreen(on);
        }}
        aria-label={t("options.display")}
      >
        <option value="fullscreen">{t("options.fullscreen")}</option>
        <option value="windowed">{t("options.windowed")}</option>
      </select>
      <p className="hint">{t("options.displayHint")}</p>

      <h3>{t("options.opening")}</h3>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={opening3d}
          onChange={(e) => {
            setOpening3d(e.target.checked);
            setOpening3dState(e.target.checked);
          }}
        />
        {t("options.opening3d")}
      </label>
      <p className="hint">{t("options.opening3dHint")}</p>

      <h3>{t("options.sound")}</h3>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={soundOn}
          onChange={(e) => {
            setSoundOn(e.target.checked);
            setSoundOnState(e.target.checked);
            refreshMusic();
            if (e.target.checked) sounds.coins();
          }}
        />
        {t("options.soundOn")}
      </label>
      <label className="checkbox volume">
        {t("options.volume")}
        <input
          type="range"
          min={0}
          max={100}
          value={Math.round(volume * 100)}
          disabled={!soundOn}
          onChange={(e) => setVolumeState(Number(e.target.value) / 100)}
          // Played once the slider is let go, at the new volume.
          onPointerUp={() => {
            setVolume(volume);
            sounds.coins();
          }}
          onKeyUp={() => setVolume(volume)}
        />
      </label>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={musicOn}
          disabled={!soundOn}
          onChange={(e) => {
            setMusicOn(e.target.checked);
            setMusicOnState(e.target.checked);
          }}
        />
        {t("options.musicOn")}
      </label>
      <label className="checkbox volume">
        {t("options.musicVolume")}
        <input
          type="range"
          min={0}
          max={100}
          value={Math.round(musicVolume * 100)}
          disabled={!soundOn || !musicOn}
          onChange={(e) => {
            const v = Number(e.target.value) / 100;
            setMusicVolumeState(v);
            setMusicVolume(v);
          }}
        />
      </label>
      <p className="hint">{t("options.musicHint")}</p>

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
