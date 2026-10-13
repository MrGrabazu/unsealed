import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { getDataStatus, type DataStatus } from "./api";
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

/** Progress of the background data refresh, or its failure (never blocking). The step
 * itself comes from the data builder, in English. */
export function DataStatusBar({ status }: { status: DataStatus }) {
  const { t } = useI18n();
  if (status.state === "updating") {
    return (
      <div className="data-status">
        <div className="progress">
          <div className="progress-bar" style={{ width: `${status.percent}%` }} />
        </div>
        <p className="hint">{t("data.updating", { percent: status.percent, step: status.message })}</p>
      </div>
    );
  }
  if (status.state === "failed") {
    return <p className="hint data-status">{t("data.failed", { reason: status.message })}</p>;
  }
  return null;
}
