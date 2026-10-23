import { Ambience } from "./Ambience";
import type { DataStatus } from "./api";
import { DataPreparing } from "./DataStatus";
import { useI18n } from "./i18n";
import { Logo } from "./Logo";
import { FanNotice } from "./Footer";

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
      <p className="legal preparing-legal">
        <FanNotice />
      </p>
    </div>
  );
}
