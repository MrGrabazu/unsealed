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
- En debug, le core et ses dépendances lourdes sont compilés optimisés (`Cargo.toml` racine) : sinon le build lancé par l'app serait des dizaines de fois plus lent. Le core y est compilé sans incrémental : optimisé et incrémental, rustc laissait des symboles `anon.*.llvm.*` non résolus (erreurs de link jusqu'à un `cargo clean -p unsealed-core`).
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

- **Monnaie du jeu : les Sceaux** (*Seals*, « Siegel », « Sellos », « Sigilli »), fictive, pour marquer que rien n'a de valeur réelle. En interne, tout reste en centimes de la **devise du marché** (`currency.market` de `economy.json` : USD ou EUR, source des prix Scryfall) ; l'interface affiche `centimes × sealsPerCent` (1 par défaut : 7,61 $ → 761 Sceaux), en nombre entier, toujours avec la même **pièce dorée en forme de sceau** (`app/src/money.tsx`) : une couleur variant avec le montant faisait croire à plusieurs monnaies. Là où un élément est impossible (listes déroulantes, infobulles, notifications), le montant s'écrit en toutes lettres.
- Les textes de configuration écrivent les montants en jetons `{cents:N}` (ex. descriptions d'achievements), convertis à l'affichage : ils restent justes si le ratio change. Un test interdit le « $ » dans les achievements et exige les mêmes jetons dans chaque traduction.

- **Achat** des boosters au prix marché (ou prix configuré).
- **Revente** des cartes à un **pourcentage du prix marché** (type buylist). **Valeur actuelle : prix du marché actuel − 20 % (soit 80 %)**, à équilibrer et à garder configurable.
  - Objectif : comme dans la réalité, la plupart des ouvertures sont perdantes, et un gros tirage procure un vrai plaisir.
- **Présentation** : une ligne par set (filtres par année et budget, prix « à partir de »), qui se déplie sur la liste de ses produits (types de booster, triés Play → Draft → Set → Collector). La vente de **boîtes de boosters** est prévue plus tard : elles s'ajouteront à cette liste de produits.
- **Illustration des sets** : l'*art crop* Scryfall de la carte phare (la plus chère des boosters principaux, choisie au build : `featured` du manifest). Règles Scryfall : afficher l'artiste et le copyright à côté, ne jamais flouter, désaturer ni recolorer l'image. Dans le Boosterium, elle sert de fond à la moitié droite de chaque ligne de set, en pleine opacité ; un dégradé de la couleur du panneau recouvre son bord gauche pour garder le texte lisible (on ne baisse jamais l'opacité de l'image elle-même).
- **Symboles d'extension** : autorisés par décision du mainteneur (la Fan Content Policy ne les vise pas explicitement). Source : `icon_svg_uri` de l'API Scryfall des sets, enregistré au build dans le manifest. Affichés tels quels, sans modification ; le SVG étant noir, il est posé sur une pastille claire.
- **Onglet d'ouverture** : grille des boosters possédés (vide avec un lien vers le Boosterium s'il n'y en a plus) ; une ouverture s'affiche une seule fois, quitter l'onglet la referme.
- **Ouvrir un booster, comme en vrai** (`PackOpening.tsx`) : le booster scellé s'affiche d'abord (`Pack.tsx`, emballage **original** : sachet métallisé à la teinte du set, bords dentelés, fenêtre sur l'art de la carte phare ; les vrais emballages portent le logo Magic et ne sont pas dans les sources). Il n'est **consommé qu'à la déchirure** (le tirage a lieu à ce moment-là). Les cartes forment une pile face cachée : un clic (ou Espace) retourne la carte du dessus, un autre la pose dans la rangée du dessous ; les cartes viennent dans l'ordre d'un vrai booster de leur type (MTGJSON n'en donne aucun : `reveal_rank`, d'après la rareté et le nom de la feuille tirée), le token toujours en dernier. **Draft** et vieux boosters : communes, peu communes, rare/mythique, foil, terrain de base. **Play** (emplacements n°1 à 15 du wiki MTG) : communes, Special Guest / The List, peu communes, wildcard, rare/mythique, terrain, wildcard foil. **Set** (« chapitres » du wiki) : terrain d'abord, communes/peu communes, showcase commune/peu commune, wildcards, rare/mythique, foil, puis The List avant le token. Pages du wiki recopiées (format wiki) dans `.cache/wiki/` (draft, play, set, collector, theme), non commitées. **Collector** (presque tout est foil) : par rareté seulement — aucune source trouvée sur leur ordre physique, à corriger si on en trouve une. Les rares, mythiques et foils ont un effet lumineux. À la fin, un clic sur une carte ouvre sa fiche. L'ouverture par lot (×10, ×100) reste accélérée.
- **Ouverture rapide** (« ⚡ Ouverture rapide », sur chaque booster scellé et à côté de « Suivant ») : pas de déchirure ni de pile, le booster est tiré aussitôt et ses cartes distribuées d'un coup en grille, dans l'ordre d'un vrai booster, le son final étant celui de la meilleure carte (`bestCard`) (`QuickOpening.tsx`, liste commune `RevealedCards.tsx`) ; tout est vu d'un coup (achievements compris). Le backend distingue les ouvertures regardées des autres : `open_booster` (commande Tauri) prend `animated`, l'événement `BoosterOpened` porte `animated` (faux pour l'ouverture rapide et par lot), d'où les métriques `boosters_watched` / `boosters_rushed` et 4 achievements secrets (500 et 1 000 de chaque).
- **Ouverture en 3D** (par défaut, désactivable dans Options : `app/src/settings.ts`, repli sur l'ouverture classique sans WebGL2) : scène three.js / react-three-fiber chargée à la demande (`PackOpening3D.tsx`, `app/src/scene/`). Le booster s'incline vers la souris et se secoue quand on le tient. On déchire la bande du haut en glissant le long d'elle (ou d'un simple clic sur le booster : elle se déchire seule depuis le côté cliqué) : elle se décolle d'un seul tenant depuis le côté de départ, puis tombe. Une fois déchiré, la pile (cachée derrière la face de l'emballage) sort par le haut, puis l'emballage vide tombe pendant que la pile se pose au centre. Sans art crop, la fenêtre du booster montre un fond rayonnant aux couleurs du set autour de son symbole d'extension (ou de son code). Les cartes sont **rigides et épaisses** (`cardMaterial.ts` : dalle extrudée à coins arrondis, tranche, foil irisé suivant la souris). Quand l'emballage tombe, la pile est rapprochée du joueur (×1,5, `CARD_ZOOM`, glissement corrigé de ce facteur) pour mieux lire les cartes ; le cadre (680 px) et le champ de la caméra (40°) leur laissent une marge en haut et en bas. Les cartes vues en grand (scène, fiche de détail) utilisent l'image « large » de Scryfall (672 px, `largeImage` de `images.ts`), les listes la « normal » (488 px). La pile s'incline d'un bloc (survoler ne dévoile rien) ; on fait glisser la carte du dessus dans n'importe quelle direction (au-delà de ~1,3 unité ou d'un geste vif, elle part ; sinon elle revient), un clic ou Espace l'envoie. Une carte compte comme vue (achievements) quand elle arrive sur le dessus ; ses effets (halo des grosses cartes, halo arc-en-ciel et étincelles des foils) n'apparaissent qu'à ce moment, jamais pour les cartes encore cachées. La scène tient elle-même l'index de la carte du dessus (`current`), sans attendre React, pour qu'une carte partie ne réapparaisse pas une image sur la pile. La dernière reste ~2 s puis s'envole ; la scène laisse alors place à la liste des cartes avec la valeur du booster.
- **Effet foil, le même partout**, voulu réaliste (la carte reste lisible) : un arc-en-ciel discret (bandes inclinées de 18° par rapport à la verticale, qui défilent lentement) pleinement visible dans une large bande de lumière qui balaie la carte, plus discret ailleurs, avec un reflet blanc diffus. En CSS, « overlay » à 85 % (60 % hors de la bande) : « soft-light » est invisible sur une vignette. Le shader 3D, vu en grand, reste en « soft-light » (40 % hors de la bande). En 3D, la bande suit la souris, comme quand on incline une vraie carte. En CSS (`.foil-sheen` de `styles.css` : ouvertures, listes, Collection dès qu'un exemplaire foil est possédé) et dans le shader des cartes 3D (`cardMaterial.ts`), qui reprend les mêmes valeurs : les modifier ensemble. Dans les listes, un halo léger qui parcourt l'arc-en-ciel entoure en plus chaque foil.
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

- **Définitions** : `config/achievements.json` (85 achievements, dont 14 secrets ; un test exige moins de mythiques que de communs). Aucun achievement quasi impossible (tirer l'Anneau unique…) : l'ultime « Unsealed » demande tous les autres.
- **Fiche de détail** : un clic sur une carte d'achievement ouvre son objectif en clair, sa progression (barre et pourcentage), sa récompense et sa date de déblocage ; un secret verrouillé n'y montre que sa rareté. Elle est mise en page comme la fiche d'une carte (`.details-layout` : carte à gauche, texte à droite). Rien n'est écrit sous les cartes de la grille : la carte elle-même porte la progression (compteur en petit au-dessus de sa barre, la description passant alors sur 2 lignes). Un `achievements.json` dans le dossier de données le remplace.
- **Moteur** : `core/src/achievements.rs`. Des *métriques* (`sum`, `max`, `streak`, `distinct`) sont alimentées par les événements ; une *condition* débloque sur seuil de métrique, sur un événement filtré, ou `allOthers` (l'ultime). Un déblocage émet `AchievementUnlocked`, ce qui permet des méta-achievements.
- **Événements émis par le jeu** (`app/src-tauri/src/game/achievements.rs`) : `BoosterOpened`, `CardAcquired`, `SetProgress` (complétion globale, rares, mythiques, foil, collector extras), `YearProgress` (sets complets parmi ceux proposés de la même année), `CardSold`, `SaleCompleted`, `BoosterBought`, `BoosterSold`, `CharityClaimed`, `BalanceChanged`. Ajouter un champ ou un événement demande du code ; ajouter un achievement n'en demande pas.
- **Pas de spoiler à l'ouverture** : le tirage a lieu à la déchirure, mais ses événements attendent (`Game::reveal_cards`). Chaque `CardAcquired` est émis quand sa carte est retournée ; les événements du booster entier (`BoosterOpened`, `SetProgress`, `YearProgress`, `BalanceChanged`) avec la dernière carte. Toute autre action, la fermeture de l'ouverture ou de la partie (`Drop`) émet ce qui reste. L'ouverture par lot émet tout d'un coup, et l'interface retient les notifications jusqu'au résumé.
- **Récompenses** : `cashCents` et/ou boosters offerts (`setCode` explicite ou `"latest"` = le set le plus récent qui propose ce type de booster, pas forcément le plus récent de tous : MBC n'a que du `draft`). Payées une seule fois, dans la même transaction que le déblocage.
- **Cartes** : dessinées en SVG par `app/src/AchievementCard.tsx` (cadre, médaillon à fond procédural selon l'id, ruban). Look volontairement différent de toute mise en page de carte à jouer. Les listes `EMBLEMS` et `PALETTES` de `core/src/achievements.rs` doivent rester synchronisées avec celles du composant.
- **Secrets** : tant qu'ils sont verrouillés, seuls la rareté et le numéro sont exposés (nom, objectif, récompense et visuel restent cachés, côté Rust).
- **Complétion foil** : ne compte que les cartes imprimées en foil (`finishes`). **Année complète** : tous les sets proposés de l'année, avec au moins 2 sets pour l'achievement livré.
- **Pas encore fait** : complétion par bloc (aucun set proposé n'appartient à un bloc, les blocs ont disparu en 2019), illustrations personnalisées par achievement.

## 7 bis. Historique et annulation

- **Onglet Historique** : tous les mouvements du portefeuille (table `ledger`), les achats et ventes avec leur détail (table `transactions`, migration 8 : articles en JSON, `TxItem`). Les ventes antérieures à cette version n'ont pas de détail et ne s'annulent pas.
- **Annuler une vente** : on récupère cartes ou boosters (avec leur coût d'achat) en rendant la somme reçue **plus une pénalité qui croît avec le temps** (`cancellation.saleSteps` de `economy.json` : 5 % ≤ 5 min, 10 % ≤ 1 h, 25 % ≤ 1 jour, 50 % ≤ 3 jours, 75 % ≤ 2 semaines, 90 % au-delà). Il faut avoir les Sceaux ; une seule fois.
- **Annuler un achat** : seulement contre le clic malheureux, dans les 5 minutes et si tous ses boosters sont encore scellés, remboursé moins 5 %. Au-delà, la revente au Boosterium existe ; un remboursement au prix payé plus tardif serait une assurance gratuite contre la baisse du marché.
- Les compteurs d'achievements (ex. cartes vendues) ne sont pas décomptés par une annulation.

## 8 ter. Parties, sauvegardes et écrans

- **Accueil** au lancement (titre, accroche, menu) : Continuer (la partie ouverte, sinon la dernière jouée), Nouvelle partie, Charger une partie (renommer, supprimer), Options, À propos. Pendant le jeu, « Menu » revient à l'accueil sans fermer la partie.
- **Une partie = un fichier SQLite** dans `saves/` (`app/src-tauri/src/saves.rs`), qui se décrit lui-même dans sa table `save_meta` (nom, dates). Plusieurs joueurs d'une même machine ont chacun leur partie ; pas de notion de profil pour l'instant.
- **Sauvegarde automatique** : chaque action est enregistrée aussitôt. **Sauvegarde manuelle** = « Save point » : une copie de la partie (`VACUUM INTO`), chargeable plus tard ; elle compte comme jouée juste avant l'originale, pour que « Continuer » reprenne l'originale.
- L'ancienne sauvegarde unique (`unsealed.sqlite`) est adoptée comme première partie au premier lancement, puis renommée en `.migrated`.
- **Chargement** : écran de démarrage inline dans `index.html` et fond de fenêtre sombre (`backgroundColor`) pour éviter tout écran blanc ; composant `Loading` sur chaque écran qui attend des données.
- **À propos** : description, gratuité et absence de valeur réelle, notice de la Fan Content Policy mot pour mot (`app/src/legal.ts`), crédits Scryfall et MTGJSON.

## 8 ter bis. Identité visuelle

- **Thème « Grimoire »** (`app/src/theme.css`, chargé après `styles.css`, qui garde la mise en page) : cuir sombre grainé, panneaux de vélin sombre bordés d'un filet doré, boutons en plaques de laiton gravées, action principale dorée, actions risquées en cire rouge, onglets en marque-pages de cuir. Palette et polices en variables CSS (`--gold-hi`, `--wax`, `--font-title`…). L'écran de chargement inline (`index.html`) et le fond de la fenêtre (`tauri.conf.json`) reprennent ses couleurs.
- **Polices** embarquées (hors ligne) sous licence SIL OFL, avec leur licence : `app/src/assets/fonts/` (Cinzel pour les titres et les boutons, Alegreya pour le texte).
- **Logo** (`logoSvg.ts`, composant `Logo.tsx`) : « UNSEALED » en capitales Cinzel dorées et biseautées, U et D plus grands, le A remplacé par un sceau de cire rouge fendu frappé d'un A ; « Crack & Collect » entre deux lames. Création originale : rien du logo ni de la typographie de Magic (Fan Content Policy). `node .cache/logo-preview.js` en fait une capture.
- **Ornements** : filigranes dorés aux coins des panneaux (SVG en data URI, `--corner-*` de `theme.css`), filet doré sous les titres de section.
- **Emballage 3D** (`scene/textures.ts`) : foil à la teinte du set dans un cadre doré à filigranes, nom du set en Cinzel doré (sur deux lignes s'il est long), illustration sertie d'or, type de booster sur un ruban de cuir, sceau du logo frappé d'un U (`SEAL_PATH`, tiré du logo : un seul dessin). La prévisualisation (`make-preview.js`) écrit aussi `app/preview-pack.html`, la texture dessinée à plat (le headless n'a pas de WebGL).
- **Icône de l'application** : le sceau fendu frappé d'un U doré. `node .cache/make-icon.js` dessine `.cache/icon.png` (1024 px), puis `npm --prefix app run tauri icon ../.cache/icon.png` régénère `app/src-tauri/icons/` (supprimer les dossiers `android/` et `ios/` qu'il crée : jeu de bureau). L'icône est intégrée à l'exe à la compilation et Cargo ne voit pas que les fichiers ont changé : `cargo clean -p unsealed-app` avant de relancer, sinon l'ancienne reste.
- **Sons** (`app/src/sound.ts`) : synthétisés par la Web Audio API (aucun fichier, aucune licence, légère variation à chaque fois) : déchirure, carte qui glisse ou se retourne, carillon d'un foil, cloche et accord d'une grosse carte ou d'une mythique (`revealSound`), pièces à l'achat et à la vente, sceau qui se brise au déblocage d'un succès, page de classeur. Activables et volume réglable dans Options (`localStorage`) ; un bouton 🔊/🔇 dans l'en-tête du jeu les coupe sans passer par le menu.
- **Prévisualiser l'interface hors Tauri** : `node .cache/make-preview.js` écrit un faux backend temporaire (`app/preview.html`, `app/preview-mock.js`, de vraies cartes de `dist/`), puis `npm --prefix app run dev` et `node .cache/shot.js "http://localhost:1420/preview.html?go=Continuer,Collection" out.png` ; `--clean` supprime ces fichiers (à ne jamais commiter). Arrêter le `npm run dev` laisse parfois Vite écouter le port 1420 (« Port 1420 is already in use » au lancement de l'app) : `node .cache/free-port.js 1420`.

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
- **Revente des doublons** : une fenêtre « Vente conclue » (`SaleModal.tsx`) montre le total en grand, le nombre de cartes et le détail (vignette, nom, foil, quantité, prix unitaire, montant), du plus rentable au moins rentable. Le détail est calculé côté interface depuis la collection affichée juste avant (`duplicatesOf`, même règle que `Game::sell_duplicates` : chaque copie au-delà de la première d'une carte rachetable). La vente d'une seule carte garde une simple ligne de notification.
- **Deux affichages** au choix (mémorisé dans le `localStorage`, `settings.ts`) : **Classeur** par défaut, ou **Grille** en second choix (filtres, vente carte par carte). Le classeur (`Binder.tsx`) a une couverture propre à chaque set, couleur et matière (cuir grainé surpiqué, toile, vinyle brillant, suédine) tirées de son code : toujours la même, rien à stocker (`coverOf`, `.binder-spread[data-cover]`). Il montre des pages de 9 pochettes (3×3), deux pages en vis-à-vis, chaque carte à sa place dans l'ordre des numéros. Une carte manquante est un bout de papier découpé à la va-vite glissé dans la pochette, avec son nom (langue des cartes) et son numéro écrits à la main (polices manuscrites du système : Ink Free, Segoe Print, Bradley Hand…), découpe, inclinaison, encre et taille tirées de l'uuid (une carte garde toujours le même papier). Hormis ces papiers, rien n'est écrit sur les pages, comme dans un vrai classeur : ni titre de section ni légende, le numéro, le nom et le prix apparaissent sur la carte au survol. Les sections (cartes, extras collector, jetons) commencent chacune sur une nouvelle page, avec un onglet au-dessus du classeur pour y sauter (celui de la section affichée est surligné) ; la barre de complétion suit la section affichée (« Jetons · 5 / 18 »), car jetons et extras se collectionnent aussi ; on tourne avec les deux grandes flèches sur les côtés du classeur ou ←/→, sans animation, le numéro des pages étant rappelé dessous (une animation de feuille a été essayée puis abandonnée). Un clic ouvre la fiche. Les colonnes sont en `minmax(0, 1fr)` : en `1fr`, une image chargée à sa taille réelle élargit sa colonne et casse la mise en page.
- **Collector Boosters** : `data.json` liste les types de booster proposés (`play`, `collector`) ; chaque set garde ceux qu'il possède (le build n'échoue que si un set n'en a aucun). Changer cette liste déclenche un rebuild au lancement (`booster_kinds` du manifest). La complétion ne compte que les cartes des boosters principaux ; les cartes exclusives aux collectors (showcase, borderless, foils spéciaux) forment une section « Collector extras » à part.
- **Sets proposés** : `"sets": "all"` → tous les sets MTGJSON des types `core`, `expansion`, `masters`, `draft_innovation`, `funny` et `starter` (hors sets en ligne) qui ont un booster voulu (`play`, `draft`, `set`, `default` = booster unique des sets d'avant 1996, `collector`). Environ 178 sets, d'Alpha (1993) à aujourd'hui. Un type de booster qui échoue à la simulation est retiré seul (le set reste proposé) ; la raison est listée dans `skipped` du manifest.
- **Taux attendus sans doublon** : sur une feuille aux poids inégaux (vieux sets imprimés sur planche, une carte pouvant occuper 1 à 5 cases ; « wildcard » des Set Boosters aux poids de 1 à 55), la probabilité qu'une carte soit tirée est calculée exactement (`sim::inclusion_probabilities`, intégrale numérique dont le pas suit le poids total de la feuille), pas en n × poids / total. Tout changement de ce calcul incrémente `VALIDATION_VERSION` (builder) pour revalider toutes les configs.
- **Tirages limités** (`config/limited.json`, surchargeable depuis le dossier de données) : certaines impressions n'existent qu'en quelques exemplaires (cartes *serialized*, numérotées). L'Anneau unique LTR #0 n'existe qu'en un seul exemplaire (~1 booster collector sur 3,3 millions). MTGJSON les décrit et les place dans les feuilles des boosters, mais ne donne pas leur tirage. Le fichier fixe donc un tirage par carte (`set` + `number`), et `serializedPrintRun` (500) pour toute impression serialized non listée (`serialized` du `CardEntry`, tiré du `promoTypes` de MTGJSON). Dans une partie, la table `limited_pulls` (migration 9) compte les exemplaires tirés. Ce compteur ne baisse **jamais**, ni à la revente ni à l'annulation d'une vente. Une carte épuisée sort de sa feuille, et une variante de booster qui ne peut plus être remplie disparaît (`open_booster_excluding`) : une fois l'Anneau trouvé, les boosters qui l'auraient contenu n'existent plus. Ces cartes se vendent comme les autres (pas de carte trophée : c'est le choix du joueur).
- **Prix estimés** : une impression sans aucun prix Scryfall (serialized, promo rare) reçoit au build le meilleur prix de ses **autres impressions** (même `oracle_id`, hors impressions serialized), finition par finition, la finition manquante prenant l'autre (`prices_from_scryfall_jsonl`, `PriceSet::estimated`). En jeu, le prix estimé d'une carte limitée est multiplié selon son tirage (`estimateMultipliers` de `limited.json` : ×10 000 pour 1 exemplaire, ×50 jusqu'à 100, ×20 jusqu'à 500). L'interface marque ces prix « ≈ » (infobulle, et une explication sur la fiche). Un prix Scryfall réel l'emporte toujours. L'EV des boosters compte les estimations, sans le multiplicateur.
- **Tokens** : MTGJSON liste les tokens de chaque set à part (`tokens`, set « T » + code : TLTR pour LTR) et aucune feuille de booster n'en contient. La liste du set parent est parfois vide ou partielle : le build y ajoute tout le contenu du sous-set de tokens (`tokenSetCode`, sinon « T » + code ; `add_tokens_from`), sans doublon. Au build, `add_token_slots` ajoute à **chaque booster** un emplacement de token (feuilles `unsealedToken` / `unsealedTokenFoil`, tokens équiprobables) : toujours foil en Collector, sinon foil une fois sur `tokenFoilOneIn` (`data.json`, 15). Chaque variante est dédoublée (token normal / foil), donc les taux restent exacts et passent la simulation. Les tokens ont la rareté `token` (et `token: true` dans le `CardEntry`) et sont révélés en premier. Leur code de set est tantôt celui d'un sous-set (« TLTR »), tantôt **celui du set lui-même** (FRA, LTR dans les données actuelles) : seul le drapeau `token` les distingue, et tout ce qui compte les cartes d'un set doit les exclure explicitement (`obtainable_cards`, carte phare `featured_card`). La Collection les retrouve sous l'un ou l'autre code (`owned_with_tokens`) et en fait une section « Jetons » hors complétion. Les cartes de pub et d'art des vrais boosters ne sont pas reproduites.
- **Masterpieces** (Invocations d'AKH/HOU, Inventions de KLD/AER, Expeditions de BFZ/OGW) et foils premium des années 2000 : déjà décrits par MTGJSON (feuille `foilOrMasterpiece1In129` / `1In144`, variantes à foil), rien à ajouter.
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
| `config/` | `economy.json` : devise, capital de départ, prix des boosters (suivent la valeur espérée × `evMarkupPercent`, surcharges fixes possibles), taux de revente (cartes et boosters scellés), aide quotidienne. `achievements.json` : voir 8 bis. `data.json` : sets à télécharger, types de boosters, âge maximal des données, chance de token foil (`tokenFoilOneIn`, appliquée au prochain build). `limited.json` : tirages limités (voir section 9). Embarqués dans l'app ; un fichier du même nom dans le dossier de données les remplace |
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
- **Commandes shell** : rester à la racine du dépôt et n'utiliser que des chemins relatifs à celle-ci (`app/src/...`, `core/src/...`, `npm --prefix app run build`). Jamais de `cd`, de `../`, de chemins `/c/...`, ni d'écriture hors du dépôt : cela déclenche des demandes de validation. Modifier les fichiers avec l'outil d'édition, jamais avec `sed`/`perl`/heredoc : le shell ne sait pas analyser ces commandes et demande une validation. Le shell sert aux commandes simples (`cargo`, `npm`, `git`). Jamais de code en ligne (`node -e "…"`, `python -c "…"`) : écrire un script dans `.cache/` puis lancer `node .cache/<script>.js`. Pas non plus de boucles, de sous-shells `$(…)` ni de `sed`/`tr` en pipe, ni de chemin absolu même pour un simple `ls` (ex. vérifier qu'un programme est installé) : tout cela passe dans un script `.cache/`. Captures d'écran de l'interface : `node .cache/shot.js <page|url> <out.png>` (Edge headless).
- En cas de doute sur un schéma de données externe (MTGJSON, Scryfall), consulter leur documentation actuelle plutôt que de supposer.
