import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useI18n } from "./i18n";
import { FAN_CONTENT_NOTICE } from "./legal";

/** The players' community, "The Boosterium", on Discord. */
const DISCORD_URL = "https://discord.gg/B5P3NJYke";

/** What the game is, what it is not, and the notices its sources require. */
export function About({ onBack }: { onBack: () => void }) {
  const { t } = useI18n();
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  return (
    <div className="page about">
      <div className="controls">
        <button onClick={onBack}>{t("common.back")}</button>
      </div>
      <h2>{t("about.title")}</h2>
      <p className="tagline">
        Crack &amp; Collect{version && <span className="hint"> · {t("about.version", { version })}</span>}
      </p>

      <div className="community">
        <div>
          <strong>The Boosterium</strong>
          <p className="hint">{t("about.community")}</p>
        </div>
        <button className="primary" onClick={() => void openUrl(DISCORD_URL)}>
          {t("about.joinDiscord")}
        </button>
      </div>

      <h3>{t("about.gameTitle")}</h3>
      <p>{t("about.game1")}</p>
      <p>{t("about.game2")}</p>

      <h3>{t("about.freeTitle")}</h3>
      <ul>
        <li>{t("about.free1")}</li>
        <li>{t("about.free2")}</li>
        <li>{t("about.free3")}</li>
      </ul>

      <h3>{t("about.fcpTitle")}</h3>
      <p className="hint">{t("about.fcpOriginal")}</p>
      {/* The official notice is never translated. */}
      <p className="legal" lang="en">
        {FAN_CONTENT_NOTICE}
      </p>
      <p>{t("about.fcp1")}</p>
      <p>{t("about.fcp2")}</p>

      <h3>{t("about.sourcesTitle")}</h3>
      <ul>
        <li>{t("about.sources1")}</li>
        <li>{t("about.sources2")}</li>
      </ul>
    </div>
  );
}
