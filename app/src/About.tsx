import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { FAN_CONTENT_NOTICE } from "./legal";

/** What the game is, what it is not, and the notices its sources require. */
export function About({ onBack }: { onBack: () => void }) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  return (
    <div className="page about">
      <div className="controls">
        <button onClick={onBack}>← Back</button>
      </div>
      <h2>About Unsealed</h2>
      <p className="tagline">Crack &amp; Collect{version && <span className="hint"> · version {version}</span>}</p>

      <h3>The game</h3>
      <p>
        Unsealed recreates the thrill of opening trading card boosters, without spending a cent. Buy virtual boosters
        in the Boosterium, crack them open card by card, build your collection, complete sets and unlock achievements.
      </p>
      <p>
        Pull rates follow the real contents of each booster, and card values follow the real market every day. Like in
        real life, most boosters are worth less than they cost: the big pulls are what makes them exciting.
      </p>

      <h3>Free and virtual, for good</h3>
      <ul>
        <li>Unsealed is entirely free: no purchase, no microtransaction, no advertising.</li>
        <li>
          Its currency, boosters and cards are virtual. They cannot be bought, sold or exchanged for anything real, and
          they have no monetary value.
        </li>
        <li>Market prices are shown for entertainment only.</li>
      </ul>

      <h3>Fan Content Policy</h3>
      <p className="legal">{FAN_CONTENT_NOTICE}</p>
      <p>
        Unsealed is not affiliated with, endorsed, sponsored or approved by Wizards of the Coast. Magic: The Gathering,
        its card names, card images, mana and expansion symbols are trademarks and copyrighted materials of Wizards of
        the Coast LLC, used here under the Fan Content Policy. Card art belongs to Wizards of the Coast and to its
        artists, who are credited wherever art is shown on its own.
      </p>
      <p>
        The achievement cards are original creations of Unsealed and do not reproduce any Wizards of the Coast design.
      </p>

      <h3>Data sources</h3>
      <ul>
        <li>
          Card images, set symbols and prices are provided by Scryfall (scryfall.com). Unsealed is not produced by or
          endorsed by Scryfall.
        </li>
        <li>Booster contents and pull rates come from MTGJSON (mtgjson.com).</li>
      </ul>
    </div>
  );
}
