import { useI18n } from "./i18n";
import { FAN_CONTENT_NOTICE } from "./legal";

/** The official Fan Content Policy notice, word for word in English, then in the interface's
 * language (an English interface has nothing more to show). */
export function FanNotice() {
  const { t } = useI18n();
  const translated = t("legal.noticeTranslated");
  return (
    <>
      <span lang="en">{FAN_CONTENT_NOTICE}</span>
      {translated && <span className="notice-translated">{translated}</span>}
    </>
  );
}

/** The notices shown at the bottom of every screen. */
export function Footer() {
  const { t } = useI18n();
  return (
    <footer>
      <FanNotice /> {t("footer.sources")}
    </footer>
  );
}
