import { useState } from "react";
import type { SaveInfo } from "./api";
import { About } from "./About";
import { GameScreen } from "./GameScreen";
import { Home } from "./Home";
import { Options } from "./Options";

type Screen = "home" | "game" | "options" | "about";

/** Navigates between the home screen, the game being played, options and about. */
export function App() {
  const [screen, setScreen] = useState<Screen>("home");
  // The game being played; it stays open while the player visits the menu.
  const [save, setSave] = useState<SaveInfo | null>(null);

  if (screen === "game" && save) {
    // A different game remounts the screen, so nothing of the previous one is shown.
    return <GameScreen key={save.id} save={save} onMenu={() => setScreen("home")} />;
  }
  if (screen === "options") return <Options onBack={() => setScreen("home")} />;
  if (screen === "about") return <About onBack={() => setScreen("home")} />;
  return (
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
  );
}
