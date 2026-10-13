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
         ├── lit le manifest local ; refresh_reason() décide : aucune donnée, set manquant, ou données plus vieilles que maxAgeHours (24 h)
         ├── si oui : core::builder::build()
         │     ├── télécharge les sets MTGJSON voulus (cache 24 h) et le bulk Scryfall default_cards (cache par nom de fichier)
         │     ├── valide chaque config de booster par simulation
         │     └── écrit sets/, prices.json, manifest.json (rename atomique, manifest en dernier)
         └── émet l'événement `data-status` ; l'UI se recharge à la fin
[Client Unsealed]
   └── reste jouable sur les anciennes données pendant la mise à jour (ou affiche une erreur non bloquante hors ligne)
   └── charge les images depuis le CDN Scryfall
```

- Les sets proposés et la fraîcheur maximale vivent dans `config/data.json` (surchargeable par un `data.json` du dossier de données).
- Les gros fichiers bruts sont gardés dans le dossier de cache de l'app ; les anciens bulks Scryfall sont supprimés après chaque build.
- `UNSEALED_NO_UPDATE=1` désactive le job (développement).
- Le CLI `data-builder` appelle le même code (`core/src/builder.rs`).

## 6. Moteur de tirage

- Implémentation du tirage pondéré en Rust à partir des sheets et variantes de booster.
- **RNG injectable avec seed** pour des tests reproductibles.
- **Mode simulation** : ouvrir N boosters (ex. 100 000) côté Rust et comparer les taux obtenus aux taux attendus.
  - Sert de test de validation de chaque config de set.
  - Permet d'afficher l'**EV réelle** de chaque booster dans le Boosterium.
- **Ouverture en lot** (×10, ×100) avec animation accélérée qui ne ralentit que sur les grosses cartes. Indispensable pour rendre atteignables les gros paliers d'achievements.

## 7. Économie (Boosterium)

- **Achat** des boosters au prix marché (ou prix configuré).
- **Revente** des cartes à un **pourcentage du prix marché** (type buylist). **Valeur actuelle : prix du marché actuel − 20 % (soit 80 %)**, à équilibrer et à garder configurable.
  - Objectif : comme dans la réalité, la plupart des ouvertures sont perdantes, et un gros tirage procure un vrai plaisir.
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

- **Définitions** : `config/achievements.json` (44 achievements, dont 8 secrets). Un `achievements.json` dans le dossier de données le remplace.
- **Moteur** : `core/src/achievements.rs`. Des *métriques* (`sum`, `max`, `streak`, `distinct`) sont alimentées par les événements ; une *condition* débloque sur seuil de métrique, sur un événement filtré, ou `allOthers` (l'ultime). Un déblocage émet `AchievementUnlocked`, ce qui permet des méta-achievements.
- **Événements émis par le jeu** (`app/src-tauri/src/game/achievements.rs`) : `BoosterOpened`, `CardAcquired`, `SetProgress`, `CardSold`, `SaleCompleted`, `BoosterBought`, `BoosterSold`, `CharityClaimed`, `BalanceChanged`. Ajouter un champ ou un événement demande du code ; ajouter un achievement n'en demande pas.
- **Récompenses** : `cashCents` et/ou boosters offerts (`setCode` explicite ou `"latest"`). Payées une seule fois, dans la même transaction que le déblocage.
- **Cartes** : dessinées en SVG par `app/src/AchievementCard.tsx` (cadre, médaillon à fond procédural selon l'id, ruban). Look volontairement différent de toute mise en page de carte à jouer. Les listes `EMBLEMS` et `PALETTES` de `core/src/achievements.rs` doivent rester synchronisées avec celles du composant.
- **Secrets** : tant qu'ils sont verrouillés, seuls la rareté et le numéro sont exposés (nom, objectif, récompense et visuel restent cachés, côté Rust).
- **Pas encore fait** : complétion « tout en foil », « bloc ou année complète » (il faut ces données), illustrations personnalisées par achievement.

## 9. Fonctionnalités de collection

- Vue par set avec **pourcentage de complétion** et cartes manquantes affichées en grisé.
- Historique des ouvertures et des **meilleurs tirages**.
- Statistiques personnelles (nombre de boosters ouverts, meilleure carte et sa valeur, etc.).

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

- Données : `cargo run --release -p data-builder -- --sets BLB,DSK,FDN` (écrit `dist/`, cache dans `.cache/`)
- App en dev : `cd app && npm install && npm run tauri dev` (en debug elle lit `dist/` ; `UNSEALED_DATA_DIR` surcharge)
- Tests : `cargo test --workspace` ; build du front : `cd app && npm run build`
- Node : Vite 6 est utilisé car Node 21 est installé (Vite 7+ exige Node ≥ 20.19 / 22.12) ; passer à Node 22 LTS permettra de monter Vite.

## 11. Règles pour l'agent

- Ne jamais introduire de mécanique liée à de l'argent réel, de la publicité conditionnant l'accès, ou un échange de valeur réelle.
- Ne jamais utiliser les logos ou marques de Wizards dans l'UI, les assets ou les noms.
- Garder la logique de tirage, d'économie et d'achievements en Rust, testée, avec RNG à seed.
- Tout ce qui est équilibrage (taux, prix, pourcentage de revente, récompenses) doit vivre dans des fichiers de configuration.
- Toute nouvelle config de set doit passer le test de simulation des taux avant d'être considérée comme valide.
- **Tout le code est en anglais** : commentaires, noms de fonctions/méthodes/variables/types, messages d'erreur, noms de tests. Aucun français dans le code (la documentation du projet, comme ce fichier, reste en français).
- En cas de doute sur un schéma de données externe (MTGJSON, Scryfall), consulter leur documentation actuelle plutôt que de supposer.
