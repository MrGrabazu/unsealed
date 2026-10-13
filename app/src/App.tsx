import { useEffect, useState } from "react";
import { listSets, openBooster, type OpenedCard, type SetSummary } from "./api";
import { BoosterOpening } from "./BoosterOpening";
import { Collection } from "./Collection";

type Tab = "open" | "collection";

// Fan Content Policy notice (AGENTS.md, section 2). Check the wording against the current policy.
const DISCLAIMER =
  "Unsealed is unofficial Fan Content permitted under the Fan Content Policy. " +
  "Not approved/endorsed by Wizards. Portions of the materials used are property of " +
  "Wizards of the Coast. ©Wizards of the Coast LLC. Card images and prices are provided by Scryfall.";

export function App() {
  const [sets, setSets] = useState<SetSummary[]>([]);
  const [setCode, setSetCode] = useState("");
  const [kind, setKind] = useState("");
  const [cards, setCards] = useState<OpenedCard[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<Tab>("open");
  // Bumped after each opening so the collection view reloads.
  const [openings, setOpenings] = useState(0);

  useEffect(() => {
    listSets()
      .then((list) => {
        setSets(list);
        if (list.length > 0) {
          setSetCode(list[0].code);
          setKind(list[0].boosters[0] ?? "");
        }
      })
      .catch((e) => setError(String(e)));
  }, []);

  const current = sets.find((s) => s.code === setCode);

  function chooseSet(code: string) {
    setSetCode(code);
    setKind(sets.find((s) => s.code === code)?.boosters[0] ?? "");
  }

  async function open() {
    setBusy(true);
    setError(null);
    try {
      setCards(await openBooster(setCode, kind));
      setOpenings((n) => n + 1);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app">
      <header>
        <h1>Unsealed</h1>
        <p className="tagline">Crack &amp; Collect</p>
      </header>

      <nav className="tabs">
        <button className={tab === "open" ? "active" : ""} onClick={() => setTab("open")}>
          Open boosters
        </button>
        <button className={tab === "collection" ? "active" : ""} onClick={() => setTab("collection")}>
          Collection
        </button>
      </nav>

      {tab === "collection" && <Collection sets={sets} refreshKey={openings} />}

      {tab === "open" && (
      <div className="controls">
        <select value={setCode} onChange={(e) => chooseSet(e.target.value)} disabled={sets.length === 0}>
          {sets.map((s) => (
            <option key={s.code} value={s.code}>
              {s.name} ({s.code})
            </option>
          ))}
        </select>
        <select value={kind} onChange={(e) => setKind(e.target.value)} disabled={!current}>
          {current?.boosters.map((b) => (
            <option key={b} value={b}>
              {b}
            </option>
          ))}
        </select>
        <button className="primary" onClick={open} disabled={busy || !setCode || !kind}>
          {busy ? "Opening…" : "Open a booster"}
        </button>
      </div>
      )}

      {error && <p className="error">{error}</p>}
      {!error && sets.length === 0 && (
        <p className="hint">
          No data found. Run <code>cargo run --release -p data-builder -- --sets BLB,DSK,FDN</code> first.
        </p>
      )}
      {tab === "open" && cards && <BoosterOpening cards={cards} />}

      <footer>{DISCLAIMER}</footer>
    </div>
  );
}
