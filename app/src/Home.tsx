import { useCallback, useEffect, useState } from "react";
import { deleteSave, listSaves, loadGame, newGame, renameSave, type SaveInfo } from "./api";
import { DataStatusBar, useDataStatus } from "./DataStatus";
import { FOOTER_NOTICE } from "./legal";
import { Loading } from "./Loading";

type Panel = "menu" | "new" | "load";

interface Props {
  /** The game being played, if one is open (back from the game screen). */
  current: SaveInfo | null;
  onPlay: (save: SaveInfo) => void;
  /** The open game was renamed (its new description) or deleted (`null`). */
  onCurrentChanged: (save: SaveInfo | null) => void;
  onOptions: () => void;
  onAbout: () => void;
}

const formatDate = (seconds: number) =>
  seconds > 0 ? new Date(seconds * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "—";

/** The home screen: continue, start or load a game, options and about. */
export function Home({ current, onPlay, onCurrentChanged, onOptions, onAbout }: Props) {
  const [saves, setSaves] = useState<SaveInfo[] | null>(null);
  const [panel, setPanel] = useState<Panel>("menu");
  const [name, setName] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  // Id of a save waiting for a second click to confirm its deletion.
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dataStatus = useDataStatus();

  const reload = useCallback(() => {
    listSaves()
      .then(setSaves)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(reload, [reload]);

  async function act(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const play = (id: string) => act(async () => onPlay(await loadGame(id)));

  // "Continue" resumes the open game, or else the last played one.
  const latest = current ?? saves?.[0] ?? null;

  return (
    <div className="home">
      <div className="home-title">
        <h1>Unsealed</h1>
        <p className="tagline">Crack &amp; Collect</p>
      </div>

      <DataStatusBar status={dataStatus} />
      {error && <p className="error">{error}</p>}
      {saves === null && !error && <Loading />}

      {saves !== null && panel === "menu" && (
        <nav className="home-menu">
          {latest && (
            <button
              className="primary"
              disabled={busy}
              onClick={() => (current ? onPlay(current) : void play(latest.id))}
            >
              Continue
              <span className="home-sub">{latest.name}</span>
            </button>
          )}
          <button
            className={latest ? "" : "primary"}
            disabled={busy}
            onClick={() => {
              setName(`Game ${saves.length + 1}`);
              setPanel("new");
            }}
          >
            New game
          </button>
          {saves.length > 0 && (
            <button disabled={busy} onClick={() => setPanel("load")}>
              Load a game
              <span className="home-sub">
                {saves.length} save{saves.length > 1 ? "s" : ""}
              </span>
            </button>
          )}
          <button onClick={onOptions}>Options</button>
          <button onClick={onAbout}>About</button>
        </nav>
      )}

      {panel === "new" && (
        <form
          className="home-panel"
          onSubmit={(e) => {
            e.preventDefault();
            void act(async () => onPlay(await newGame(name)));
          }}
        >
          <h2>New game</h2>
          <p className="hint">
            A new collection and wallet, starting from scratch. Your other games are kept: each player on this computer
            can have their own.
          </p>
          <input autoFocus value={name} maxLength={60} onChange={(e) => setName(e.target.value)} placeholder="Name" />
          <div className="controls">
            <button type="button" onClick={() => setPanel("menu")}>
              ← Back
            </button>
            <button className="primary" type="submit" disabled={busy}>
              Start
            </button>
          </div>
        </form>
      )}

      {panel === "load" && saves && (
        <div className="home-panel">
          <h2>Load a game</h2>
          <p className="hint">Games are saved automatically. Save points are copies you can come back to.</p>
          <div className="save-list">
            {saves.map((s) => (
              <div key={s.id} className={`save-row ${current?.id === s.id ? "current" : ""}`}>
                {renaming?.id === s.id ? (
                  <form
                    className="save-rename"
                    onSubmit={(e) => {
                      e.preventDefault();
                      void act(async () => {
                        const renamed = await renameSave(s.id, renaming.name);
                        if (current?.id === s.id) onCurrentChanged(renamed);
                        setRenaming(null);
                        reload();
                      });
                    }}
                  >
                    <input
                      autoFocus
                      value={renaming.name}
                      maxLength={60}
                      onChange={(e) => setRenaming({ id: s.id, name: e.target.value })}
                    />
                    <button type="submit">OK</button>
                    <button type="button" onClick={() => setRenaming(null)}>
                      Cancel
                    </button>
                  </form>
                ) : (
                  <div className="save-name">
                    <strong>{s.name}</strong>
                    <span className="hint">
                      {current?.id === s.id ? "playing now · " : ""}last played {formatDate(s.lastPlayedAt)} · created{" "}
                      {formatDate(s.createdAt)}
                    </span>
                  </div>
                )}
                <div className="save-actions">
                  <button className="primary" disabled={busy} onClick={() => void play(s.id)}>
                    Load
                  </button>
                  <button disabled={busy} onClick={() => setRenaming({ id: s.id, name: s.name })}>
                    Rename
                  </button>
                  <button
                    className={`sell ${confirmDelete === s.id ? "confirm" : ""}`}
                    disabled={busy}
                    onMouseLeave={() => confirmDelete === s.id && setConfirmDelete(null)}
                    onClick={() => {
                      if (confirmDelete !== s.id) {
                        setConfirmDelete(s.id);
                        return;
                      }
                      setConfirmDelete(null);
                      void act(async () => {
                        await deleteSave(s.id);
                        if (current?.id === s.id) onCurrentChanged(null);
                        reload();
                      });
                    }}
                  >
                    {confirmDelete === s.id ? "Delete for good?" : "Delete"}
                  </button>
                </div>
              </div>
            ))}
          </div>
          <div className="controls">
            <button onClick={() => setPanel("menu")}>← Back</button>
          </div>
        </div>
      )}

      <footer>{FOOTER_NOTICE}</footer>
    </div>
  );
}
