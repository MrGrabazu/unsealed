import type { ReactNode } from "react";
import { createPortal } from "react-dom";

/** The dimmed backdrop of a dialog, rendered at the root of the page: inside the game's
 * stage it would stay under the strap at the top (and the dialog with it). A click on it
 * closes the dialog. */
export function Backdrop({ onClose, children }: { onClose: () => void; children: ReactNode }) {
  return createPortal(
    <div className="modal-backdrop" onClick={onClose}>
      {children}
    </div>,
    document.body,
  );
}
