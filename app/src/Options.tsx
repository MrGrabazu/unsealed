import { useEffect, useState } from "react";
import { getDataInfo, type DataInfo } from "./api";
import { DataStatusBar, useDataStatus } from "./DataStatus";
import { Loading } from "./Loading";

/** Settings. For now, what the downloaded game data contains; options will come here. */
export function Options({ onBack }: { onBack: () => void }) {
  const [info, setInfo] = useState<DataInfo | null>(null);
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
        <button onClick={onBack}>← Back</button>
      </div>
      <h2>Options</h2>

      <h3>Game data</h3>
      <DataStatusBar status={dataStatus} />
      {error && <p className="error">{error}</p>}
      {!info && !error && <Loading />}
      {info && (
        <dl className="info-list">
          <dt>Sets</dt>
          <dd>{info.setCount.toLocaleString()}</dd>
          <dt>Booster types</dt>
          <dd>{info.boosterCount.toLocaleString()}</dd>
          <dt>Card prices from</dt>
          <dd>{info.pricesUpdatedAt ? new Date(info.pricesUpdatedAt).toLocaleString() : "—"}</dd>
          <dt>Last update</dt>
          <dd>{info.builtAt ? new Date(info.builtAt * 1000).toLocaleString() : "never"}</dd>
        </dl>
      )}
      <p className="hint">
        Card data and prices are downloaded in the background at launch and refreshed every day. Nothing to do on your
        side.
      </p>

      <h3>More options</h3>
      <p className="hint">More settings will appear here in future versions.</p>
    </div>
  );
}
