import { useEffect, useState } from "react";
import { useI18n } from "./i18n";
import { findUpdate, installUpdate, type Update } from "./updates";

/** On the home screen, a small frame in the corner when a newer version is published: it
 * installs it and restarts the app. Nothing shows when the app is up to date or offline. */
export function UpdateNotice() {
  const { t } = useI18n();
  const [update, setUpdate] = useState<Update | null>(null);
  // While installing: how far the download is (null while its size is unknown).
  const [progress, setProgress] = useState<number | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void findUpdate().then(setUpdate);
  }, []);

  if (!update) return null;
  const installing = progress !== undefined;
  return (
    <aside className="update-notice frame" role="status">
      <div className="cartouche">{t("update.title")}</div>
      <p>{t("update.available", { version: update.version })}</p>
      {installing ? (
        <div className="bar" aria-label={t("update.installing")}>
          <i style={{ width: `${Math.round((progress ?? 0.1) * 100)}%` }} />
        </div>
      ) : (
        <button
          className="big"
          onClick={() => {
            setError(null);
            setProgress(null);
            installUpdate(update, setProgress).catch((e) => {
              setProgress(undefined);
              setError(String(e));
            });
          }}
        >
          {t("update.install")}
        </button>
      )}
      {installing && <p className="hint">{t("update.installing")}</p>}
      {error && <p className="error">{error}</p>}
    </aside>
  );
}
