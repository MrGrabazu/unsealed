import { Ambience } from "./Ambience";
import type { DataStatus } from "./api";
import { DataPreparing } from "./DataStatus";
import { useI18n } from "./i18n";
import { Logo } from "./Logo";
import { FanNotice } from "./Footer";
import { quitApp } from "./fullscreen";
import { UpdateNotice } from "./UpdateNotice";

/** The first launch: the game's data (sets, cards, prices) is being built and nothing can be
 * played yet. Shown instead of the home screen, which appears once it is ready. */
export function Preparing({ status }: { status: DataStatus }) {
  const { t } = useI18n();
  return (
    <div className="home preparing">
      <Ambience />
      <div className="home-hero">
        <h1 className="home-title">
          <Logo />
        </h1>
      </div>
      <section className="frame preparing-frame">
        <div className="cartouche">{t("data.prepareCartouche")}</div>
        <DataPreparing status={status} />
      </section>
      {/* Full screen, the window has no title bar: the way out while the game cannot start. */}
      <button className="preparing-quit" onClick={() => void quitApp()}>
        {t("home.quit")}
      </button>
      {/* A newer version may be what fixes the preparation: offer it here too. */}
      <UpdateNotice />
      <p className="legal preparing-legal">
        <FanNotice />
      </p>
    </div>
  );
}
