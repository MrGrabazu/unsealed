/** A set's expansion symbol. The SVG is black: it sits on a light badge so it stays
 * visible on the dark theme, and the image itself is never altered. */
export function SetIcon({ url, name }: { url: string | null; name: string }) {
  if (!url) return null;
  return (
    <span className="set-icon" title={name}>
      <img src={url} alt="" draggable={false} />
    </span>
  );
}
