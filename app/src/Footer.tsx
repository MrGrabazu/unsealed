import { useI18n } from "./i18n";
import { FAN_CONTENT_NOTICE } from "./legal";

/** The notices shown at the bottom of every screen. */
export function Footer() {
  const { t } = useI18n();
  return (
    <footer>
      {FAN_CONTENT_NOTICE} {t("footer.sources")}
    </footer>
  );
}
