/** Mana and other symbols ({2}{G}, {T}, {W/U}...) as Scryfall's official SVGs, shown
 * unaltered (allowed by the maintainer, AGENTS.md section 11). */
const SYMBOL = /\{([^}]+)\}/g;
const symbolUrl = (symbol: string) => `https://svgs.scryfall.io/card-symbols/${symbol.replace(/\//g, "")}.svg`;

export function ManaText({ text }: { text: string }) {
  const parts: (string | { symbol: string })[] = [];
  let last = 0;
  for (const match of text.matchAll(SYMBOL)) {
    if (match.index! > last) parts.push(text.slice(last, match.index));
    parts.push({ symbol: match[1] });
    last = match.index! + match[0].length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return (
    <>
      {parts.map((part, i) =>
        typeof part === "string" ? (
          <span key={i}>{part}</span>
        ) : (
          <img key={i} className="mana" src={symbolUrl(part.symbol)} alt={`{${part.symbol}}`} draggable={false} />
        ),
      )}
    </>
  );
}
