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

type T = ReturnType<typeof useI18n>["t"];

/** The steps the game sends, in English (core/src/builder.rs, pack.rs, updater.rs). */
const STEPS = {
  Starting: "data.step.starting",
  "Looking for the ready-made game data": "data.step.lookingForPack",
  "Downloading the game data": "data.step.downloadingPack",
  "Installing the game data": "data.step.installingPack",
  "Downloading every set from MTGJSON (large file)": "data.step.downloadingSets",
  "Reading the sets": "data.step.readingSets",
  "Downloading Scryfall prices (large file)": "data.step.downloadingPrices",
  "Reading prices": "data.step.readingPrices",
  "Saving the game data": "data.step.saving",
} as const;

/** A step of the data refresh in the interface's language; an unknown one stays as sent. */
function stepText(step: string, t: T): string {
  const key = STEPS[step as keyof typeof STEPS];
  if (key) return t(key);
  const preparing = /^Preparing (.+) \((\d+)\/(\d+)\)$/.exec(step);
  if (preparing) return t("data.step.preparingSet", { set: preparing[1], index: preparing[2], count: preparing[3] });
  return step;
}

/** A failure, "<step>: <error>": the step translated, the error itself as the system gave it. */
function failureText(message: string, t: T): string {
  const at = message.indexOf(": ");
  if (at < 0) return message;
  const step = message.slice(0, at);
  const translated = stepText(step, t);
  return translated === step ? message : `${translated} — ${message.slice(at + 2)}`;
}

/** Progress of the background data refresh while the game is playable on the data it has,
 * or its failure (never blocking). */
export function DataStatusBar({ status }: { status: DataStatus }) {
  const { t } = useI18n();
  if (status.state === "updating") {
    return (
      <div className="data-status" role="status">
        <span className="data-status-text">{t("data.updating", { percent: status.percent, step: stepText(status.message, t) })}</span>
        <div className="bar">
          <i style={{ width: `${status.percent}%` }} />
        </div>
      </div>
    );
  }
  if (status.state === "failed") {
    return <p className="hint data-status failed">{t("data.failed", { reason: failureText(status.message, t) })}</p>;
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
          <p className="hint">{failureText(status.message, t)}</p>
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
          <p className="prepare-step">{status.state === "updating" ? stepText(status.message, t) : t("data.prepareChecking")}</p>
          <p className="hint">{t("data.prepareHint")}</p>
        </>
      )}
    </div>
  );
}
