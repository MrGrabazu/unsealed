import { useCallback, useEffect, useState } from "react";
import { getDataInfo, type SaveInfo } from "./api";
import { About } from "./About";
import { BackToTop } from "./BackToTop";
import { useDataStatus } from "./DataStatus";
import { Preparing } from "./Preparing";
import { getFullscreen, setFullscreen, toggleFullscreen } from "./fullscreen";
import { GameScreen } from "./GameScreen";
import { Home } from "./Home";
import { Options } from "./Options";

type Screen = "home" | "game" | "options" | "about";

/** Navigates between the home screen, the game being played, options and about. */
export function App() {
  const [screen, setScreen] = useState<Screen>("home");
  // The game being played; it stays open while the player visits the menu.
  const [save, setSave] = useState<SaveInfo | null>(null);
  // Whether the game has its data (sets, cards, prices): not on the first launch, until the
  // background build is done. Unknown for an instant at start.
  const [hasData, setHasData] = useState<boolean | null>(null);
  const checkData = useCallback(() => {
    getDataInfo()
      .then((info) => setHasData(info.setCount > 0))
      .catch(() => setHasData(true));
  }, []);
  useEffect(checkData, [checkData]);
  const dataStatus = useDataStatus(checkData);

  // The window opens in full screen: back in a window if the player chose so. F11 switches.
  useEffect(() => {
    if (!getFullscreen()) void setFullscreen(false);
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "F11") return;
      e.preventDefault();
      void toggleFullscreen();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Nothing to play without data: the first launch shows its preparation until it is done
  // (with no data and no refresh coming, the home screen says so itself).
  if (hasData === null) return null;
  if (!hasData && dataStatus.state !== "upToDate") return <Preparing status={dataStatus} />;

  return (
    <>
      {screen === "game" && save ? (
        // A different game remounts the screen, so nothing of the previous one is shown.
        <GameScreen key={save.id} save={save} onMenu={() => setScreen("home")} />
      ) : screen === "options" ? (
        <Options onBack={() => setScreen("home")} />
      ) : screen === "about" ? (
        <About onBack={() => setScreen("home")} />
      ) : (
        <Home
          current={save}
          onPlay={(s) => {
            setSave(s);
            setScreen("game");
          }}
          onCurrentChanged={setSave}
          onOptions={() => setScreen("options")}
          onAbout={() => setScreen("about")}
        />
      )}
      {/* Whatever the screen, a long page can be scrolled back up at once. */}
      <BackToTop />
    </>
  );
}
