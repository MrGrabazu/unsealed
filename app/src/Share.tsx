import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { Backdrop } from "./Backdrop";
import { useI18n } from "./i18n";
import { Loading } from "./Loading";
import { copyPicture, savePicture } from "./sharePicture";

/** A button that opens the sharing of a pull: its picture, to copy or save. `make` draws it.
 * `round`: a rounded pill, floating over a corner rather than taking a line. */
export function ShareButton({
  make,
  fileName,
  className,
  round = false,
}: {
  make: () => Promise<Blob>;
  fileName: string;
  className?: string;
  round?: boolean;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        className={`${round ? "share-round" : "share-button"} ${className ?? ""}`}
        onClick={() => setOpen(true)}
        title={t("share.hint")}
      >
        {t("share.button")}
      </button>
      {open && <SharePicture make={make} fileName={fileName} onClose={() => setOpen(false)} />}
    </>
  );
}

/** The picture of a pull, shown as it will be shared, with copying it (to paste it in
 * Discord, WhatsApp…) and saving it. */
function SharePicture({ make, fileName, onClose }: { make: () => Promise<Blob>; fileName: string; onClose: () => void }) {
  const { t } = useI18n();
  const [blob, setBlob] = useState<Blob | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [message, setMessage] = useState<{ text: string; error?: boolean } | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  // Drawn once, when the window opens (the pull it shows does not change).
  useEffect(() => {
    let live = true;
    let made: string | null = null;
    make()
      .then((b) => {
        if (!live) return;
        made = URL.createObjectURL(b);
        setBlob(b);
        setUrl(made);
      })
      .catch((e) => live && setMessage({ text: String(e), error: true }));
    return () => {
      live = false;
      if (made) URL.revokeObjectURL(made);
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  async function copy() {
    if (!blob) return;
    try {
      await copyPicture(blob);
      setMessage({ text: t("share.copied") });
    } catch {
      setMessage({ text: t("share.copyFailed"), error: true });
    }
  }

  async function save() {
    if (!blob) return;
    try {
      const path = await savePicture(blob, fileName);
      setSaved(path);
      setMessage({ text: t("share.saved", { path }) });
    } catch (e) {
      setMessage({ text: String(e), error: true });
    }
  }

  return (
    <Backdrop onClose={onClose}>
      <div className="modal share-modal" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <button className="modal-close" onClick={onClose} aria-label={t("details.close")}>
          ×
        </button>
        <h2>{t("share.title")}</h2>
        <div className="share-preview">{url ? <img src={url} alt="" /> : !message?.error && <Loading label={t("share.making")} />}</div>
        <p className="hint">{t("share.hint")}</p>
        {message && <p className={message.error ? "error" : "notice"}>{message.text}</p>}
        <div className="share-actions">
          <button className="big" disabled={!blob} onClick={() => void copy()}>
            {t("share.copy")}
          </button>
          <button disabled={!blob} onClick={() => void save()}>
            {t("share.save")}
          </button>
          {saved && <button onClick={() => void revealItemInDir(saved)}>{t("share.showFolder")}</button>}
        </div>
      </div>
    </Backdrop>
  );
}
