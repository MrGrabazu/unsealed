/** Shown while a screen waits for its data, so the app never looks frozen. */
export function Loading({ label = "Loading…" }: { label?: string }) {
  return (
    <div className="loading" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      <span>{label}</span>
    </div>
  );
}
