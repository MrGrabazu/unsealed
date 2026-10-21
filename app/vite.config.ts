import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { LOGO_SVG } from "./src/logoSvg";

/** Puts the game's logo in the splash of `index.html` (shown before any script runs), from the
 * same drawing as the `Logo` component: never a copy to keep in step. */
const splashLogo = (): Plugin => ({
  name: "splash-logo",
  transformIndexHtml: (html) => html.replace("<!-- splash-logo -->", LOGO_SVG.replaceAll("{{id}}", "splash-")),
});

// Tauri expects a fixed dev server port.
export default defineConfig({
  plugins: [react(), splashLogo()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
