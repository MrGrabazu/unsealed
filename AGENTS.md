# AGENTS.md — Unsealed

> **Unsealed — Crack & Collect**
> Jeu desktop gratuit qui reproduit les sensations d'ouverture de boosters Magic: The Gathering, avec des taux de tirage fidèles à chaque set et une économie virtuelle basée sur les prix réels du marché.

## 1. Vision du projet

- Reproduire le plaisir de l'ouverture de boosters **sans aucun coût réel** pour le joueur.
- Satisfaire la « collectionnite » : collection, complétion de sets, trophées exclusifs.
- Tout repose sur des **données réelles** : contenu des sets, taux de tirage, prix du marché.

### Identité

| Élément | Valeur |
|---|---|
| Nom du jeu | **Unsealed** |
| Accroche | **Crack & Collect** |
| Boutique in-game | **Boosterium** : lieu où l'on achète des boosters et revend ses cartes |

## 2. Contraintes non négociables

### Légal
- **100 % gratuit et 100 % virtuel.** Aucune microtransaction, aucun paywall, aucune publicité qui conditionnerait l'accès au contenu.
- **Aucune conversion** de la monnaie virtuelle ou des cartes vers quoi que ce soit de réel (pas d'échange monétaire, pas de cash-out). C'est ce qui garde le jeu hors du champ des réglementations sur les loot boxes.
- Respect de la **Fan Content Policy de Wizards of the Coast** :
  - afficher le disclaimer officiel de non-affiliation (écran « À propos » et page de lancement) ;
  - ne pas utiliser les logos Wizards / Magic ni de termes déposés (« Magic », « MTG », « Planeswalker »…) dans le nom ou l'identité de l'app ;
  - ne jamais laisser penser que l'app est officielle.
- Respect des **conditions d'utilisation de Scryfall** :
  - ne pas recadrer les images de cartes en masquant l'artiste ou le copyright ;
  - ne pas placer leurs données derrière un paywall ;
  - respecter leurs limites de requêtes et envoyer les en-têtes `User-Agent` et `Accept` requis.

### Cartes d'achievement (non-Magic)
- Même **format** que les cartes Magic (ratio 63×88 mm, mêmes interactions dans la collection).
- **Cadre et identité visuelle 100 % originaux** : ne jamais reproduire le cadre ou la mise en page des cartes Magic.
- Illustrations originales uniquement (créées, commandées, ou sous licence libre compatible).

## 3. Stack technique

| Couche | Techno | Rôle |
|---|---|---|
| Shell desktop | **Tauri** | Packaging, plugin updater pour les mises à jour |
| Backend | **Rust** | Tirage aléatoire, parsing des données, persistance, moteur d'achievements |
| Frontend | **React** | UI, animations d'ouverture, collection, Boosterium |
| Persistance | **SQLite** (`rusqlite` ou `sqlx`) | Collection, portefeuille, historique, progression des achievements |
| RNG | `rand` + `WeightedIndex` | Tirage pondéré, **RNG à seed** pour rejouer et tester |
| Animations | framer-motion (ou équivalent) | Révélation carte par carte, effets foil en CSS |

Règle générale : **la logique métier vit en Rust** (tirage, économie, achievements). React affiche et anime, il ne calcule pas les tirages.

## 4. Sources de données

### MTGJSON : configuration des boosters
- Les fichiers de set contiennent une clé `booster` qui décrit chaque type de booster (draft, set, play, collector…) :
  - les **sheets** (common, uncommon, rare/mythic, foil, land…), avec le poids de chaque carte ;
  - les **variantes de contenu** d'un booster et leurs poids respectifs.
- C'est la source principale des taux de tirage. **Vérifier le schéma exact dans la documentation MTGJSON actuelle** avant d'écrire le parser.
- Des fichiers de configuration locaux peuvent **surcharger ou corriger** la config MTGJSON pour un set donné.

### Scryfall : cartes, images, prix
- Métadonnées des cartes, images (CDN Scryfall, mises en cache localement côté client).
- Prix quotidiens (USD/EUR, foil / non-foil).

### Prix des boosters scellés
- Pas de source gratuite fiable : **décision actuelle = fichier de configuration édité à la main** par le mainteneur (un prix par set et par type de booster). Calcul à partir de l'**espérance de valeur (EV)** plus une marge = piste future, non retenue pour le moment.

## 5. Architecture de distribution des données

Le jeu est un client lourd : **il n'y a pas de serveur à héberger**. L'app construit elle-même ses données sur la machine du joueur, en tâche de fond.

```
[Lancement de l'app]
   └── thread de fond (updater.rs)
         ├── lit le manifest local ; refresh_reason() décide : aucune donnée, set manquant, config changée (sets, types, boosters), ou données plus vieilles que maxAgeHours (24 h)
         ├── si oui : core::builder::build()
         │     ├── télécharge AllPrintings.json.gz de MTGJSON (~180 Mo, cache 7 jours) et le bulk Scryfall default_cards (cache par nom de fichier)
         │     ├── sélectionne les sets (liste explicite, ou "all" : types de setTypes, hors sets en ligne, avec un booster voulu)
         │     ├── valide chaque config de booster par simulation (sauf si déjà validée à l'identique : cache validated.json)
         │     ├── met de côté les sets invalides (`skipped` du manifest) au lieu d'échouer
         │     └── écrit sets/, prices.json, manifest.json (rename atomique, manifest en dernier)
         └── émet l'événement `data-status` (message + pourcentage estimé) ; l'UI affiche une barre de progression et se recharge à la fin
[Client Unsealed]
   └── reste jouable sur les anciennes données pendant la mise à jour (ou affiche une erreur non bloquante hors ligne)
   └── charge les images depuis le CDN Scryfall
```

- Les sets proposés et la fraîcheur maximale vivent dans `config/data.json` (surchargeable par un `data.json` du dossier de données).
- Les gros fichiers bruts sont gardés dans le dossier de cache de l'app ; les anciens bulks Scryfall (et les anciens fichiers MTGJSON par set) sont supprimés après chaque build.
- En debug, le core et ses dépendances lourdes sont compilés optimisés (`Cargo.toml` racine) : sinon le build lancé par l'app serait des dizaines de fois plus lent.
- `UNSEALED_NO_UPDATE=1` désactive le job (développement).
- Le CLI `data-builder` appelle le même code (`core/src/builder.rs`).

## 6. Moteur de tirage

- Implémentation du tirage pondéré en Rust à partir des sheets et variantes de booster.
- **RNG injectable avec seed** pour des tests reproductibles.
- **Mode simulation** : ouvrir N boosters (ex. 100 000) côté Rust et comparer les taux obtenus aux taux attendus.
  - Sert de test de validation de chaque config de set.
  - Permet d'afficher l'**EV réelle** de chaque booster dans le Boosterium.
- **Ouverture en lot** (×10, ×100) avec animation accélérée qui ne ralentit que sur les grosses cartes. Indispensable pour rendre atteignables les gros paliers d'achievements. *Implémenté* : `Game::open_boosters` (une transaction par booster, s'arrête proprement si le stock s'épuise) et `BatchOpening.tsx` (carte « grosse » = mythique, rare foil ou prix ≥ 5 ; résumé final avec valeur totale, nouveautés, meilleurs tirages).

## 7. Économie (Boosterium)

- **Monnaie du jeu : les Sceaux** (*Seals*, « Siegel », « Sellos », « Sigilli »), fictive, pour marquer que rien n'a de valeur réelle. En interne, tout reste en centimes de la **devise du marché** (`currency.market` de `economy.json` : USD ou EUR, source des prix Scryfall) ; l'interface affiche `centimes × sealsPerCent` (1 par défaut : 7,61 $ → 761 Sceaux), en nombre entier, avec une **pièce en forme de sceau** bronze, argent (`silverFrom`, 100) ou or (`goldFrom`, 10 000) selon le montant (`app/src/money.tsx`). Là où un élément est impossible (listes déroulantes, infobulles, notifications), le montant s'écrit en toutes lettres.
- Les textes de configuration écrivent les montants en jetons `{cents:N}` (ex. descriptions d'achievements), convertis à l'affichage : ils restent justes si le ratio change. Un test interdit le « $ » dans les achievements et exige les mêmes jetons dans chaque traduction.

- **Achat** des boosters au prix marché (ou prix configuré).
- **Revente** des cartes à un **pourcentage du prix marché** (type buylist). **Valeur actuelle : prix du marché actuel − 20 % (soit 80 %)**, à équilibrer et à garder configurable.
  - Objectif : comme dans la réalité, la plupart des ouvertures sont perdantes, et un gros tirage procure un vrai plaisir.
- **Présentation** : une ligne par set (filtres par année et budget, prix « à partir de »), qui se déplie sur la liste de ses produits (types de booster, triés Play → Draft → Set → Collector). La vente de **boîtes de boosters** est prévue plus tard : elles s'ajouteront à cette liste de produits.
- **Illustration des sets** : l'*art crop* Scryfall de la carte phare (la plus chère des boosters principaux, choisie au build : `featured` du manifest). Règles Scryfall : afficher l'artiste et le copyright à côté, ne jamais flouter, désaturer ni recolorer l'image. Dans le Boosterium, elle sert de fond à la moitié droite de chaque ligne de set, en pleine opacité ; un dégradé de la couleur du panneau recouvre son bord gauche pour garder le texte lisible (on ne baisse jamais l'opacité de l'image elle-même).
- **Symboles d'extension** : autorisés par décision du mainteneur (la Fan Content Policy ne les vise pas explicitement). Source : `icon_svg_uri` de l'API Scryfall des sets, enregistré au build dans le manifest. Affichés tels quels, sans modification ; le SVG étant noir, il est posé sur une pastille claire.
- **Onglet d'ouverture** : grille des boosters possédés (vide avec un lien vers le Boosterium s'il n'y en a plus) ; une ouverture s'affiche une seule fois, quitter l'onglet la referme.
- **Historique des prix des boosters** : à chaque nouveau `prices.json`, le prix de chaque booster est enregistré en SQLite (table `price_history`, un point par snapshot `updatedAt`). Le Boosterium et les boosters scellés affichent les 30 derniers points en sparkline, avec la variation depuis la mise à jour précédente (`PriceTrend.tsx`).
- **Capital de départ** et **petit revenu quotidien** pour éviter un game over définitif.
- Tous les paramètres économiques sont **configurables** (pas de valeurs en dur dans le code).

## 8. Achievements

### Moteur
- **Définis dans des fichiers de configuration** (condition, seuils, récompense) : ajouter un achievement ne doit nécessiter aucune modification de code.
- **Moteur événementiel en Rust** : le jeu émet des événements, et chaque achievement écoute ceux qui le concernent.
  - Exemples d'événements : `BoosterOpened`, `CardAcquired`, `CardSold`, `SetCompleted`.
- Progression et compteurs persistés dans SQLite.

### Catégories
- **Paliers de volume** : ouvrir 10 / 100 / 1 000 / 10 000 / 100 000… boosters ; cumuler X de valeur ouverte ; revendre X cartes.
- **Complétion** : set complet, toutes les rares d'un set, set entièrement foil, bloc ou année complète.
- **Tirages remarquables** : mythique foil, deux mythiques dans un même booster, carte au-dessus d'une valeur seuil.
- **Cachés / humoristiques** : booster le moins rentable de l'historique, longue série sans mythique, revente par erreur de sa carte la plus chère…

### Récompenses
- **Bonus d'argent virtuel** : à calibrer sur l'économie. Préférer des récompenses proportionnelles ou des **boosters offerts** à des montants fixes.
- **Cartes d'achievement exclusives** (voir section 2) :
  - obtenables **uniquement** via les achievements ;
  - rareté proportionnelle à la difficulté de l'achievement ;
  - forment leur propre collection « méta », dont la complétion est l'achievement ultime.

### 8 bis. État actuel de l'implémentation

- **Définitions** : `config/achievements.json` (51 achievements, dont 8 secrets). Un `achievements.json` dans le dossier de données le remplace.
- **Moteur** : `core/src/achievements.rs`. Des *métriques* (`sum`, `max`, `streak`, `distinct`) sont alimentées par les événements ; une *condition* débloque sur seuil de métrique, sur un événement filtré, ou `allOthers` (l'ultime). Un déblocage émet `AchievementUnlocked`, ce qui permet des méta-achievements.
- **Événements émis par le jeu** (`app/src-tauri/src/game/achievements.rs`) : `BoosterOpened`, `CardAcquired`, `SetProgress` (complétion globale, rares, mythiques, foil, collector extras), `YearProgress` (sets complets parmi ceux proposés de la même année), `CardSold`, `SaleCompleted`, `BoosterBought`, `BoosterSold`, `CharityClaimed`, `BalanceChanged`. Ajouter un champ ou un événement demande du code ; ajouter un achievement n'en demande pas.
- **Récompenses** : `cashCents` et/ou boosters offerts (`setCode` explicite ou `"latest"`). Payées une seule fois, dans la même transaction que le déblocage.
- **Cartes** : dessinées en SVG par `app/src/AchievementCard.tsx` (cadre, médaillon à fond procédural selon l'id, ruban). Look volontairement différent de toute mise en page de carte à jouer. Les listes `EMBLEMS` et `PALETTES` de `core/src/achievements.rs` doivent rester synchronisées avec celles du composant.
- **Secrets** : tant qu'ils sont verrouillés, seuls la rareté et le numéro sont exposés (nom, objectif, récompense et visuel restent cachés, côté Rust).
- **Complétion foil** : ne compte que les cartes imprimées en foil (`finishes`). **Année complète** : tous les sets proposés de l'année, avec au moins 2 sets pour l'achievement livré.
- **Pas encore fait** : complétion par bloc (aucun set proposé n'appartient à un bloc, les blocs ont disparu en 2019), illustrations personnalisées par achievement.

## 8 ter. Parties, sauvegardes et écrans

- **Accueil** au lancement (titre, accroche, menu) : Continuer (la partie ouverte, sinon la dernière jouée), Nouvelle partie, Charger une partie (renommer, supprimer), Options, À propos. Pendant le jeu, « Menu » revient à l'accueil sans fermer la partie.
- **Une partie = un fichier SQLite** dans `saves/` (`app/src-tauri/src/saves.rs`), qui se décrit lui-même dans sa table `save_meta` (nom, dates). Plusieurs joueurs d'une même machine ont chacun leur partie ; pas de notion de profil pour l'instant.
- **Sauvegarde automatique** : chaque action est enregistrée aussitôt. **Sauvegarde manuelle** = « Save point » : une copie de la partie (`VACUUM INTO`), chargeable plus tard ; elle compte comme jouée juste avant l'originale, pour que « Continuer » reprenne l'originale.
- L'ancienne sauvegarde unique (`unsealed.sqlite`) est adoptée comme première partie au premier lancement, puis renommée en `.migrated`.
- **Chargement** : écran de démarrage inline dans `index.html` et fond de fenêtre sombre (`backgroundColor`) pour éviter tout écran blanc ; composant `Loading` sur chaque écran qui attend des données.
- **À propos** : description, gratuité et absence de valeur réelle, notice de la Fan Content Policy mot pour mot (`app/src/legal.ts`), crédits Scryfall et MTGJSON.

## 8 quater. Langues

- **Interface** en anglais, français, allemand, espagnol et italien. Module maison sans dépendance : `app/src/i18n/` (`en.ts` = référence ; `fr.ts`, `de.ts`, `es.ts`, `it.ts` typés `Messages`, donc **TypeScript refuse une langue incomplète**). `t(clé, { variables })`, `plural(base, n)` (clés `_one` / `_other` via `Intl.PluralRules`), dates et nombres au format de la langue.
- **Choix** dans Options, mémorisé dans le `localStorage` ; par défaut la langue du système si elle est proposée, sinon l'anglais.
- **Achievements** : l'anglais vit dans `config/achievements.json`, les traductions dans `config/achievements.<langue>.json` (id → nom et description), surchargeables depuis le dossier de données. Un test vérifie que chaque fichier couvre exactement les achievements définis : **ajouter un achievement oblige à le traduire**. Les récompenses sont renvoyées en données et formulées par l'interface.
- **Langue des cartes et des sets** : réglage séparé (Options), « comme l'interface » par défaut, en anglais, français, allemand, espagnol, italien ou japonais (`FOREIGN_LANGUAGES` du core). Au build, chaque carte garde ses impressions traduites (`foreign` : nom et `scryfallId` de l'impression, depuis le `foreignData` de MTGJSON) et chaque set ses noms traduits (`names`, depuis `translations`, rempli pour ~146 sets seulement). Le jeu affiche nom et image traduits quand ils existent, sinon l'anglais, carte par carte (les vieux sets n'ont été imprimés qu'en anglais). Les prix restent ceux de l'impression anglaise. Le réglage vit dans la session Rust (`set_card_language`), envoyé par l'interface au démarrage et à chaque changement.
- **Format des données** : `DATA_FORMAT_VERSION` (core) est écrit dans le manifest ; s'il change, l'app reconstruit ses données au lancement suivant.
- **Restent en anglais** : les noms « Unsealed », « Crack & Collect » et « Boosterium », la notice de la Fan Content Policy (texte officiel, affiché tel quel, suivi d'une explication traduite), les étapes détaillées du téléchargement des données et les messages d'erreur venant du Rust.
- **Ajouter une langue** : un fichier `app/src/i18n/<code>.ts`, une entrée dans `LANGUAGES` et `MESSAGES` (`i18n/index.tsx`), un `config/achievements.<code>.json`, et une ligne dans `ACHIEVEMENT_TRANSLATIONS` (`lib.rs`) et dans le test des traductions.

## 8 quinquies. Fiche détaillée d'une carte

- **Ouverture** : clic sur une carte de la Collection. La fiche montre la grande image, le texte de chaque face (coût en symboles de mana, type, texte, texte d'ambiance, force/endurance ou loyauté), l'artiste, les prix du marché normal et foil, ce que le Boosterium en donne, les exemplaires possédés, et **toutes les impressions** de la carte (set, numéro, année, prix du jour, « dans le jeu », exemplaires possédés).
- **Source** : la partie locale vient des données du jeu ; le texte et les impressions viennent de l'**API Scryfall à la demande** (`core/src/scryfall.rs`), avec cache mémoire pour la session et au moins 100 ms entre deux requêtes (conditions de Scryfall). La commande Tauri est asynchrone et n'appelle le réseau ni sur le fil principal ni en tenant la session. Hors ligne, seule la partie locale s'affiche.
- **Langue** : le texte vient de l'impression dans la langue des cartes quand elle existe (`printed_*`), sinon du texte Oracle anglais.
- **Cartes double face** (transform, modal double face, réversible, bataille…) : bouton « Retourner » sur la fiche, dans la Collection et à l'ouverture (une fois la carte révélée). L'image du dos vient du CDN Scryfall (`/back/`).
- Test réseau à la demande : `cargo test -p unsealed-core --test scryfall_live -- --ignored`.

## 9. Fonctionnalités de collection

- Vue par set avec **pourcentage de complétion** et cartes manquantes affichées en grisé.
- **Collector Boosters** : `data.json` liste les types de booster proposés (`play`, `collector`) ; chaque set garde ceux qu'il possède (le build n'échoue que si un set n'en a aucun). Changer cette liste déclenche un rebuild au lancement (`booster_kinds` du manifest). La complétion ne compte que les cartes des boosters principaux ; les cartes exclusives aux collectors (showcase, borderless, foils spéciaux) forment une section « Collector extras » à part.
- **Sets proposés** : `"sets": "all"` → tous les sets MTGJSON des types `core`, `expansion`, `masters`, `draft_innovation`, `funny` et `starter` (hors sets en ligne) qui ont un booster voulu (`play`, `draft`, `set`, `default` = booster unique des sets d'avant 1996, `collector`). Environ 178 sets, d'Alpha (1993) à aujourd'hui. Un type de booster qui échoue à la simulation est retiré seul (le set reste proposé) ; la raison est listée dans `skipped` du manifest.
- **Taux attendus sans doublon** : sur une feuille aux poids inégaux (vieux sets imprimés sur planche, une carte pouvant occuper 1 à 5 cases), la probabilité qu'une carte soit tirée est calculée exactement (`sim::inclusion_probabilities`), pas en n × poids / total.
- Historique des ouvertures et des **meilleurs tirages**.
- Statistiques personnelles (nombre de boosters ouverts, meilleure carte et sa valeur, etc.).
- *Implémenté* : onglet **Stats** (`app/src/Stats.tsx`, `Game::stats`). Totaux d'ouverture, argent dépensé et gagné (agrégé depuis le `ledger`), rendement des boosters, valeur de la collection et patrimoine, meilleur et pire booster, 12 meilleurs tirages de tous les temps (au prix du tirage), tableau par booster et 20 dernières ouvertures. Tout est calculé à la demande depuis `openings` / `opening_cards` (index sur les prix, migration 6).

## 10. Roadmap MVP

1. **Fondations données** : parser la clé `booster` de MTGJSON pour un set récent ; valider en Rust, par simulation, que les taux obtenus collent aux taux attendus.
2. **Import** MTGJSON + Scryfall pour 2 ou 3 sets récents en Play Booster.
3. **Ouverture animée** et ajout à la collection.
4. **Boosterium** : portefeuille, achat de boosters, revente de cartes.
5. **Achievements** : moteur événementiel et premiers paliers.
6. Ensuite : Collector Boosters, sets plus anciens, statistiques, cartes d'achievement, pipeline de distribution des données.

## 10 bis. Structure du dépôt et commandes

| Dossier | Rôle |
|---|---|
| `core/` | Crate `unsealed-core` : parsing MTGJSON, tirage à seed, simulation, import, construction des données (`builder`), décision de mise à jour (`update`) |
| `data-builder/` | CLI fin autour de `core::builder` : produit `dist/` (sets compacts, `prices.json`, `manifest.json`) ; chaque config de booster doit passer la simulation. L'app fait la même chose seule au lancement |
| `config/` | `economy.json` : devise, capital de départ, prix des boosters (suivent la valeur espérée × `evMarkupPercent`, surcharges fixes possibles), taux de revente (cartes et boosters scellés), aide quotidienne. `achievements.json` : voir 8 bis. `data.json` : sets à télécharger, types de boosters, âge maximal des données. Embarqués dans l'app ; un fichier du même nom dans le dossier de données les remplace |
| `app/` | Frontend React/Vite ; `app/src-tauri/` = shell Tauri 2 (logique de jeu dans `game.rs`, testable sans Tauri) |

- Données : `cargo run --release -p data-builder -- --sets all` (ou une liste : `--sets BLB,FDN`) ; écrit `dist/`, cache dans `.cache/`, et liste les sets mis de côté avec la raison
- App en dev : `cd app && npm install && npm run tauri dev` (en debug elle lit `dist/` et range les parties dans `.dev-data/saves/` ; `UNSEALED_DATA_DIR` et `UNSEALED_SAVES_DIR` surchargent)
- Tests : `cargo test --workspace` ; build du front : `cd app && npm run build`
- Node : Vite 6 est utilisé car Node 21 est installé (Vite 7+ exige Node ≥ 20.19 / 22.12) ; passer à Node 22 LTS permettra de monter Vite.

## 11. Règles pour l'agent

- Ne jamais introduire de mécanique liée à de l'argent réel, de la publicité conditionnant l'accès, ou un échange de valeur réelle.
- Ne jamais utiliser les logos ou marques de Wizards dans l'UI, les assets ou les noms. Seules exceptions, par décision du mainteneur (usage courant des outils de deck) : les symboles d'extension des sets et les symboles de mana, non modifiés (voir section 7). Les logos Magic: The Gathering et Wizards of the Coast restent strictement interdits.
- Garder la logique de tirage, d'économie et d'achievements en Rust, testée, avec RNG à seed.
- Tout ce qui est équilibrage (taux, prix, pourcentage de revente, récompenses) doit vivre dans des fichiers de configuration.
- Toute nouvelle config de set doit passer le test de simulation des taux avant d'être considérée comme valide.
- **Tout le code est en anglais** : commentaires, noms de fonctions/méthodes/variables/types, messages d'erreur, noms de tests. Aucun français dans le code (la documentation du projet, comme ce fichier, reste en français).
- **Commandes shell** : rester à la racine du dépôt et n'utiliser que des chemins relatifs à celle-ci (`app/src/...`, `core/src/...`, `npm --prefix app run build`). Jamais de `cd`, de `../`, de chemins `/c/...`, ni d'écriture hors du dépôt : cela déclenche des demandes de validation. Modifier les fichiers avec l'outil d'édition, jamais avec `sed`/`perl`/heredoc : le shell ne sait pas analyser ces commandes et demande une validation. Le shell sert aux commandes simples (`cargo`, `npm`, `git`).
- En cas de doute sur un schéma de données externe (MTGJSON, Scryfall), consulter leur documentation actuelle plutôt que de supposer.
