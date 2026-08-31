# Brief pour Claude Design — site des voyages de la famille

Document destiné à une session de travail sur l'interface. Il décrit ce qui
existe, pourquoi, et ce qui reste ouvert. Il ne décrit pas le pipeline de
données, sauf là où il contraint l'affichage.

## 1. Ce que c'est

Un carnet de voyage familial, en ligne, privé. Un premier voyage y est publié,
le **Tour des Alpes** : 23 journées, du 24 juillet au 15 août 2026, Paris à
Paris par l'Allemagne, l'Autriche, la Slovénie et l'Italie. 705 photos et
128 vidéos, 3 800 km de trace.

Public : la famille proche, une quinzaine de personnes. Le site est derrière
une authentification par code à usage unique, non indexé, sans compte à créer.
Il n'y a ni recherche, ni commentaire, ni partage social, ni analytics.

L'usage réel est la lecture longue et le feuilletage : on ouvre une journée,
on lit le récit, on regarde les photos, on suit le tracé sur la carte, on passe
au jour suivant. Sur ordinateur le soir, sur téléphone dans les transports.

D'autres voyages suivront — Japon, Polynésie, Tunisie — avec une contrainte
importante : **leurs photos n'ont pas de coordonnées GPS**. L'interface doit
donc supporter un voyage sans carte, ou à carte très pauvre.

## 2. Les pages

Cinq gabarits, tous statiques.

**Accueil `/`.** Liste des voyages. Aujourd'hui une seule entrée. C'est la page
la plus pauvre et la moins réfléchie du site.

**Vue d'ensemble d'un voyage `/voyages/2026-alpes/`.** Titre, sous-titre,
quatre chiffres clés (journées, kilomètres, nuits, campings), une carte du
voyage entier avec sa légende, puis la frise verticale des 23 journées —
chacune une ligne avec sa date, son titre, son nombre de médias et ses
kilomètres.

**Journée `/voyages/2026-alpes/jours/2026-08-08/`.** La page la plus lue.
De haut en bas : la frise horizontale des 23 jours, puis deux colonnes
au-delà de 1024 px — récit à gauche sur 55 %, carte fixe à droite sur 45 %.
Le récit est un texte Markdown de 200 à 600 mots, dans lequel sont posées une
à trois photos ou galeries. Sous le récit, une galerie des autres photos du
jour, puis les liens vers la veille et le lendemain. Sous 1024 px, une seule
colonne, la carte devenant un bandeau collant en haut de l'écran sur 30 % de
la hauteur.

**Lecteur jour par jour `/voyages/2026-alpes/carte/`.** Une carte sur 58 % de
la hauteur, un curseur horizontal qui fait défiler les 23 journées, deux
boutons ronds de navigation, et le titre de la journée active entre les deux.
Le zéro du curseur montre le voyage entier.

**Toutes les photos `/voyages/2026-alpes/photos/`.** Une grille de 705
vignettes. Page volontairement brute.

## 3. Les composants

**La carte** (MapLibre GL, fond OpenFreeMap Positron, clair et pâle).
Elle porte : les tracés colorés par mode de déplacement, des pastilles pour
les médias, des marqueurs noirs nommés pour les campings, un contrôle de zoom
et un bouton plein écran. Au survol d'une pastille, une étiquette apparaît en
bas à gauche avec la vignette de la photo (240 px de large) et le titre de la
journée ; au survol d'un tracé, le titre seul. Un clic sur une pastille, sur
la page d'une journée, fait défiler le récit jusqu'à la photo et l'entoure ;
inversement, la photo au centre de l'écran s'entoure d'un anneau sur la carte.

**La légende de carte.** Sous la carte : une ligne de modes présents avec leur
couleur et leurs kilomètres, puis une ligne de repères — camping, photo,
vidéo, position reconstituée.

**La frise des journées.** 23 pastilles carrées de 44 px, le numéro du jour,
le mois rappelé à sa bascule, la journée courante en accent plein. Défilement
horizontal sur téléphone.

**La visionneuse.** Un `<dialog>` natif qui ouvre une photo en plein écran,
avec les flèches du clavier, l'échappement et le balayage au doigt.

## 4. Le langage visuel actuel

Choisi pour que le sujet soit les photos et le texte, pas l'interface.

- Fond `#fbf9f6`, encre `#2b2723`, gris discret `#6f665e`, traits `#e3ddd5`,
  accent terre cuite `#c0562a`. Mode sombre : fond `#16130f`, encre `#eae4dc`.
  Les deux modes suivent la préférence système, sans bascule manuelle.
- Récit en serif système, interface en sans-serif système. Aucune fonte
  chargée : c'est un choix de performance qu'on peut rediscuter.
- Colonne de lecture à 46 rem, page large à 76 rem.
- Aucune animation en dehors du recentrage de la carte.

**La carte ne suit pas cette palette**, délibérément : six modes de
déplacement rabattus vers le beige ne se distinguaient plus sur un fond gris.
Route `#4a3b2e`, marche `#d1491f`, vélo `#2e7d32`, bateau `#1565c0`, train
`#8e24aa`, téléphérique `#b08300`. Pastille photo blanche cerclée de sombre,
pastille vidéo prune, pastille creuse pour une position reconstituée.

## 5. Les contraintes techniques

Elles sont fermes et une proposition qui les ignore ne pourra pas être
retenue.

- **Site statique**, Astro, sans framework d'interface. Pas de React, Vue ou
  Svelte : le JavaScript se compte en centaines d'octets hors carte.
- **Budget de poids transféré**, en brotli : moins de 50 Ko de JavaScript sur
  une page sans carte, moins de 320 Ko avec. MapLibre en consomme 309 à lui
  seul. La visionneuse tient en 986 octets.
- **Les images** sont servies en trois largeurs (320, 1024, 2048) depuis un
  stockage objet, avec un aperçu flou en fond le temps du chargement.
- **Français** partout, typographie française : apostrophes typographiques,
  guillemets français, espaces insécables. Jamais de tiret cadratin.
- **Accessibilité** : navigation au clavier, cibles tactiles de 44 px, la
  couleur ne porte jamais seule une information.

## 6. Ce qui me paraît faible, et sur quoi j'attends des idées

Par ordre décroissant d'importance.

**La page d'accueil** n'existe qu'administrativement. Avec plusieurs voyages,
que montre-t-elle ? Une carte du monde avec les voyages posés dessus ? Des
cartes-vignettes ? Une frise chronologique ?

**La galerie « les autres photos du jour »** est une grille sans hiérarchie de
20 à 100 vignettes. Elle écrase le récit qui la précède. Faut-il la replier,
la paginer, la remplacer par une mosaïque orientée, la supprimer au profit
d'un placement complet dans le récit ?

**La lecture à deux colonnes** fonctionne mais la carte fixe reste passive :
elle montre la journée entière et ne réagit qu'à la photo survolée. Comment
faire vivre le lien entre le texte et la carte sans transformer la page en
tableau de bord ?

**La frise des jours** est fonctionnelle et sans grâce. 23 carrés numérotés.
Elle pourrait porter davantage : le mode dominant du jour, sa distance, une
vignette.

**Le lecteur jour par jour** double la vue d'ensemble sans que la frontière
soit claire. Peut-être les fusionner.

**Les chiffres clés** sont quatre nombres alignés, sans mise en scène. Le
voyage a de belles données : 3 800 km, 6 modes de déplacement, 9 campings,
23 jours, 4 pays.

**Le mode sombre** est un simple échange de variables, jamais retravaillé.

**La page des photos** est une grille brute de 705 vignettes.

## 7. Ce que je ne veux pas

- Une interface qui se remarque avant les photos.
- Des animations d'apparition au défilement.
- Un menu, une barre latérale, un fil d'Ariane complexe : le site a cinq
  pages et un fil de navigation qui tient en une ligne.
- Des icônes décoratives.
- Un ton « produit » : c'est un carnet de famille, pas une application.

## 8. Captures à fournir

Prendre chaque capture en **pleine page** (Chrome : F12 → Ctrl+Maj+P →
« Capture full size screenshot »), en clair et, pour les deux premières, aussi
en sombre.

1. `/voyages/2026-alpes/jours/2026-08-08/` en 1440 px de large — la page la
   plus représentative : récit, photos posées, carte, galerie.
2. La même en 390 px de large (mode téléphone) — pour montrer le bandeau de
   carte collant et la frise qui défile.
3. `/voyages/2026-alpes/` en 1440 px — chiffres clés, carte d'ensemble,
   frise verticale des journées.
4. `/voyages/2026-alpes/carte/` en 1440 px — le lecteur et son curseur.
5. `/voyages/2026-alpes/photos/` en 1440 px, le haut de page suffit.
6. `/` en 1440 px — la page d'accueil, pour montrer ce qu'elle n'est pas.
7. Une capture rapprochée de la carte avec une étiquette de survol visible,
   et une autre de la légende sous la carte.

Joindre aussi, si le format le permet :

- deux ou trois photos du voyage en pleine résolution, pour que les
  propositions soient jugées sur de vraies images et non sur des rectangles ;
- ce fichier.
