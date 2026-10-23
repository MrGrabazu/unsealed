import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import { startMusicOnFirstGesture } from "./music";
import "./styles.css";
import "./theme.css";
import "./comptoir.css";
import "./foils.css";

// The ambience starts with the first click or key press (browsers allow no sound before).
startMusicOnFirstGesture();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </React.StrictMode>,
);
