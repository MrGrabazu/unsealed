import { useCallback, useEffect, useState } from "react";
import { Ambience } from "./Ambience";
import { deleteSave, listSaves, listSets, loadGame, newGame, renameSave, type SaveInfo } from "./api";
import { DataStatusBar, useDataStatus } from "./DataStatus";
import { Footer } from "./Footer";
import { quitApp } from "./fullscreen";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { Logo } from "./Logo";

type Panel = "menu" | "new" | "load";

/** How the cards behind the logo fan out, in degrees (the newest set's in the middle). */
const FAN_ANGLES = [0, -15, 15, -30, 30];

interface Props {
  /** The game being played, if one is open (back from the game screen). */
  current: SaveInfo | null;
  onPlay: (save: SaveInfo) => void;
  /** The open game was renamed (its new description) or deleted (`null`). */
  onCurrentChanged: (save: SaveInfo | null) => void;
  onOptions: () => void;
  onAbout: () => void;
}

/** The home screen: continue, start or load a game, options and about. */
export function Home({ current, onPlay, onCurrentChanged, onOptions, onAbout }: Props) {
  const { t, plural, formatDate } = useI18n();
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

  // The fan behind the logo: the featured cards of the newest sets with one.
  const [fan, setFan] = useState<string[]>([]);
  useEffect(() => {
    listSets()
      .then((sets) =>
        setFan(
          sets
            .flatMap((s) => (s.art ? [s.art.imageUrl.replace("/art_crop/", "/normal/")] : []))
            .slice(0, FAN_ANGLES.length),
        ),
      )
      .catch(() => {});
  }, []);

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
  const date = (seconds: number) => (seconds > 0 ? formatDate(seconds) : t("common.none"));

  // "Continue" resumes the open game, or else the last played one.
  const latest = current ?? saves?.[0] ?? null;

  return (
    <div className="home">
      <div className="home-hero">
        {/* A fan of the newest sets' featured cards, faint, behind the logo. Whole cards, so
            their artist and copyright stay in view (Scryfall's terms). */}
        <div className="home-fan" aria-hidden="true">
          {fan.map((url, i) => (
            <img key={url} src={url} alt="" draggable={false} style={{ transform: `rotate(${FAN_ANGLES[i]}deg)`, zIndex: FAN_ANGLES[i] === 0 ? 2 : 1 }} />
          ))}
        </div>
        <h1 className="home-title">
          <Logo />
        </h1>
      </div>

      <DataStatusBar status={dataStatus} />
      {error && <p className="error">{error}</p>}
      {saves === null && !error && <Loading />}

      {saves !== null && panel === "menu" && (
        <nav className="home-menu frame">
          <div className="cartouche">{t("home.menuTitle")}</div>
          {latest && (
            <button
              className="primary"
              disabled={busy}
              onClick={() => (current ? onPlay(current) : void play(latest.id))}
            >
              {t("home.continue")}
              <span className="home-sub">{latest.name}</span>
            </button>
          )}
          <button
            className={latest ? "" : "primary"}
            disabled={busy}
            onClick={() => {
              setName(t("home.defaultName", { number: saves.length + 1 }));
              setPanel("new");
            }}
          >
            {t("home.newGame")}
          </button>
          {saves.length > 0 && (
            <button disabled={busy} onClick={() => setPanel("load")}>
              {t("home.loadGame")}
              <span className="home-sub">{plural("home.saveCount", saves.length)}</span>
            </button>
          )}
          <button onClick={onOptions}>{t("home.options")}</button>
          <button onClick={onAbout}>{t("home.about")}</button>
          <button onClick={() => void quitApp()}>{t("home.quit")}</button>
        </nav>
      )}

      {panel === "new" && (
        <form
          className="home-panel frame"
          onSubmit={(e) => {
            e.preventDefault();
            void act(async () => onPlay(await newGame(name)));
          }}
        >
          <h2>{t("home.newTitle")}</h2>
          <p className="hint">{t("home.newHint")}</p>
          <input
            autoFocus
            value={name}
            maxLength={60}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("home.namePlaceholder")}
          />
          <div className="controls">
            <button type="button" onClick={() => setPanel("menu")}>
              {t("common.back")}
            </button>
            <button className="primary" type="submit" disabled={busy}>
              {t("home.start")}
            </button>
          </div>
        </form>
      )}

      {panel === "load" && saves && (
        <div className="home-panel frame">
          <h2>{t("home.loadTitle")}</h2>
          <p className="hint">{t("home.loadHint")}</p>
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
                    <button type="submit">{t("common.ok")}</button>
                    <button type="button" onClick={() => setRenaming(null)}>
                      {t("common.cancel")}
                    </button>
                  </form>
                ) : (
                  <div className="save-name">
                    <strong>{s.name}</strong>
                    <span className="hint">
                      {[
                        current?.id === s.id ? t("home.playingNow") : null,
                        t("home.lastPlayed", { date: date(s.lastPlayedAt) }),
                        t("home.created", { date: date(s.createdAt) }),
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
                  </div>
                )}
                <div className="save-actions">
                  <button className="primary" disabled={busy} onClick={() => void play(s.id)}>
                    {t("home.load")}
                  </button>
                  <button disabled={busy} onClick={() => setRenaming({ id: s.id, name: s.name })}>
                    {t("home.rename")}
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
                    {confirmDelete === s.id ? t("home.deleteConfirm") : t("home.delete")}
                  </button>
                </div>
              </div>
            ))}
          </div>
          <div className="controls">
            <button onClick={() => setPanel("menu")}>{t("common.back")}</button>
          </div>
        </div>
      )}

      <Footer />
      <Ambience />
    </div>
  );
}
