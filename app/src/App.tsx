import { useEffect, useState } from "react";
import type { SaveInfo } from "./api";
import { About } from "./About";
import { BackToTop } from "./BackToTop";
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
