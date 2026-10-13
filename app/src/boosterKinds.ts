/** Display order of booster kinds; unknown kinds come last, alphabetically. */
const KIND_ORDER = ["play", "draft", "set", "default", "collector"];

export const kindRank = (kind: string) => {
  const i = KIND_ORDER.indexOf(kind);
  return i === -1 ? KIND_ORDER.length : i;
};

/** "Draft booster", or just "Booster" for the oldest sets, which had a single kind ("default"). */
export const kindLabel = (kind: string) =>
  kind === "default" ? "Booster" : `${kind.charAt(0).toUpperCase()}${kind.slice(1)} booster`;
