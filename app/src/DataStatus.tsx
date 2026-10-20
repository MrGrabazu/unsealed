import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { getDataStatus, retryDataUpdate, type DataStatus } from "./api";
import { useI18n } from "./i18n";

/** Follows the background data refresh. `onRefreshed` runs when a refresh completes. */
export function useDataStatus(onRefreshed?: () => void): DataStatus {
  const [status, setStatus] = useState<DataStatus>({ state: "checking" });
  const callback = useRef(onRefreshed);
  callback.current = onRefreshed;

  useEffect(() => {
    let previous: DataStatus["state"] = "checking";
    const onStatus = (next: DataStatus) => {
      setStatus(next);
      if (previous === "updating" && next.state === "upToDate") callback.current?.();
      previous = next.state;
    };
    getDataStatus().then(onStatus).catch(() => {});
    const unlisten = listen<DataStatus>("data-status", (e) => onStatus(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return status;
}

/** Progress of the background data refresh while the game is playable on the data it has,
 * or its failure (never blocking). The step itself comes from the data builder, in English. */
export function DataStatusBar({ status }: { status: DataStatus }) {
  const { t } = useI18n();
  if (status.state === "updating") {
    return (
      <div className="data-status" role="status">
        <span className="data-status-text">{t("data.updating", { percent: status.percent, step: status.message })}</span>
        <div className="bar">
          <i style={{ width: `${status.percent}%` }} />
        </div>
      </div>
    );
  }
  if (status.state === "failed") {
    return <p className="hint data-status failed">{t("data.failed", { reason: status.message })}</p>;
  }
  return null;
}

/** The data being prepared while there is none yet (first launch): what happens, how far it
 * is, and a retry if it failed (offline…). Shown in its own screen, or in place of an empty one. */
export function DataPreparing({ status }: { status: DataStatus }) {
  const { t } = useI18n();
  const failed = status.state === "failed";
  const percent = status.state === "updating" ? status.percent : 0;
  return (
    <div className="data-preparing" role="status">
      <h2>{t(failed ? "data.prepareFailedTitle" : "data.prepareTitle")}</h2>
      {failed ? (
        <>
          <p>{t("data.prepareFailed")}</p>
          <p className="hint" lang="en">
            {status.message}
          </p>
          <button className="big" onClick={() => void retryDataUpdate()}>
            {t("data.retry")}
          </button>
        </>
      ) : (
        <>
          <div className="prepare-gauge">
            <div className="bar">
              <i style={{ width: `${Math.max(percent, 2)}%` }} />
            </div>
            <b>{percent} %</b>
          </div>
          {/* The builder's steps are in English ("Downloading Scryfall prices"…). */}
          <p className="prepare-step" lang="en">
            {status.state === "updating" ? status.message : t("data.prepareChecking")}
          </p>
          <p className="hint">{t("data.prepareHint")}</p>
        </>
      )}
    </div>
  );
}
