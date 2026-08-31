# Notes d'implémentation — direction « Le cartographe »

Cible : le site Astro statique de `thibautng/travel`. Ces notes traduisent la
maquette `Cartographe.dc.html` en travail réel, dans les contraintes du brief :
pas de framework d'interface, moins de 50 Ko de JavaScript sur une page sans
carte, moins de 320 Ko avec, français et typographie française partout, cibles
tactiles de 44 px, la couleur ne porte jamais seule une information.

La maquette est une maquette : elle contient des fac-similés de carte en SVG et
un peu d'état React. **Rien de tout cela ne part en production.** Ce qui part,
c'est la grille, les jetons, la hiérarchie typographique, le protocole
d'ancrage numéroté et la fusion des deux pages de carte.

---

## 1. Jetons

À poser dans un unique bloc `:root` du layout, sans fichier de tokens
supplémentaire.

### Couleurs d'interface

| Jeton | Clair | Sombre |
| --- | --- | --- |
| `--bg` | `#f2f3ef` | `#14181a` |
| `--panel` | `#ffffff` | `#1b2124` |
| `--ink` | `#1e2422` | `#e6e9e5` |
| `--muted` | `#6b7370` | `#9aa29d` |
| `--faint` | `#8b918d` | `#79817d` |
| `--rule` | `#dcdfd8` | `#2b3235` |
| `--accent` | `#d1491f` | `#e8683c` |
| `--hl` (ligne active) | `#f7f2ee` | `#241f1d` |

Le fond de carte MapLibre et sa grille suivent : `--map` `#eaece7` / `#1b2124`,
`--grid` `#dfe2dc` / `#242c2f`.

### Couleurs des modes de déplacement

Elles restent hors de la palette d'interface, comme aujourd'hui, mais elles
**changent en sombre** : un brun `#4a3b2e` sur fond `#1b2124` tombe à 1,3:1 et
disparaît. Version sombre : la même teinte, luminosité inversée.

| Mode | Clair | Sombre |
| --- | --- | --- |
| Route | `#4a3b2e` | `#b6a692` |
| Marche | `#d1491f` | `#f0714a` |
| Vélo | `#2e7d32` | `#5fae63` |
| Train | `#8e24aa` | `#c07ad6` |
| Bateau | `#1565c0` | `#5aa0e6` |
| Téléphérique | `#b08300` | `#d8ae3d` |

Conséquence côté carte : le style MapLibre doit lire ces couleurs depuis
les variables CSS au moment de la construction du style, pas les coder en dur —
et se reconstruire si la préférence système change (`matchMedia('(prefers-color-scheme: dark)')`,
écouteur `change`, `map.setPaintProperty` sur les six couches de trace).

### Typographie

Trois familles, chargées en woff2 auto-hébergé, sous-ensemble latin étendu
(le voyage écrit Sušec, Vršič, Löcherberg, Königssee) :

- **IBM Plex Serif** 400 + 400 italique — récits, titres de journée et de voyage.
- **IBM Plex Sans** 400/500 — interface, légendes, libellés.
- **IBM Plex Mono** 400/500 — tous les chiffres : dates courtes, kilomètres,
  compteurs de médias, numéros de jour.

Poids transféré : 4 fichiers woff2 sous-ensembles, environ 90 à 110 Ko au
total, hors budget JavaScript. C'est le seul vrai coût ajouté par cette
direction. Si ce coût est refusé, la direction survit avec
`ui-serif / system-ui / ui-monospace` : elle perd en tenue, pas en structure —
mais `font-variant-numeric: tabular-nums` devient obligatoire partout où il y a
une colonne de chiffres.

### Échelles

- Mesure de lecture : 46 rem (inchangé). Page large : 76 rem.
- Titre de journée : `clamp(2rem, 1.4rem + 2.2vw, 2.875rem)`, `line-height: 1.06`, `letter-spacing: -0.022em`.
- Récit : 19 px / 1.72. Interface : 13 px. Chiffres de tableau : 13 px mono.
- Filets : 1 px `--rule`. Aucun rayon, aucune ombre : la maquette n'en a pas et
  n'en veut pas.

---

## 2. Pages, routes, ce qui disparaît

| Route | Statut | Contenu |
| --- | --- | --- |
| `/` | refondue | Deux colonnes : liste des voyages à gauche (440 px), carte pleine hauteur à droite. |
| `/voyages/[voyage]/` | refondue + absorbe le lecteur | Titre, chiffres, graphique des modes, **carte + curseur des 23 journées**, liste des journées. |
| `/voyages/[voyage]/carte/` | **supprimée** | Redirection permanente vers `/voyages/[voyage]/#lecteur`. |
| `/voyages/[voyage]/jours/[jour]/` | refondue | Frise enrichie, récit à ancres numérotées, carte collante numérotée, planche contact. |
| `/voyages/[voyage]/photos/` | retouchée | Index des journées en tête, en-têtes de journée collants, grille dense. |

La suppression de `/carte/` est le cœur de la proposition : une seule carte
interactive par voyage, une seule adresse, un seul modèle mental. Sur
Cloudflare, la redirection se fait dans `worker/index.js` (ou un `_redirects`),
code 301, en conservant le préfixe de voyage.

---

## 3. Le lien texte ↔ carte : ancres numérotées

Aujourd'hui le lien n'existe qu'au survol, donc il n'existe pas au doigt ni au
clavier. La direction le rend explicite : chaque bloc média posé dans le récit
reçoit un **numéro**, et le même numéro est une pastille sur la carte.

### Numérotation

Elle sort du récit, pas des données : les directives `::photo` et `::galerie`
sont numérotées **dans l'ordre du Markdown**, 1, 2, 3. Un jour à trois blocs a
trois numéros ; un jour sans bloc n'a pas de pastille numérotée et la carte
retombe sur les pastilles anonymes actuelles.

À faire dans `site/src/directives.ts` : émettre sur le conteneur du bloc

```html
<figure id="m1" data-ancre="1" tabindex="-1">
```

et exposer, pour la carte, un point par bloc. Position du point = position de
la **première photo du bloc** (celle qui a déjà une position EXIF ou
reconstituée dans le pipeline). Aucun nouveau calcul géographique.

### Rendu de la pastille

Cercle de 22 px de diamètre, `--accent`, chiffre blanc en mono 12 px. Sous la
carte, une rangée de boutons carrés de 44 px portant les mêmes numéros : c'est
la cible tactile, la pastille de carte n'a pas à l'être. Deux libellés
redondants au lieu de la couleur seule : le numéro est écrit, la légende sous
la carte le rappelle.

### Comportement

- Clic sur un numéro (pastille ou bouton) → le bloc correspondant reçoit
  `.actif` (contour 2 px `--accent`, décalage 3 px) et la page défile jusqu'à lui.
- Clic sur un bloc du récit → la pastille correspondante grossit (rayon 11 → 15).
- L'état actif est **unique** et persiste : pas de retour au repos automatique,
  pas d'animation d'entrée.
- Sans JavaScript : les numéros restent des liens `href="#m1"`, l'ancre HTML
  fait le défilement, la carte reste statique. Rien ne casse.

### Coût

Un seul écouteur délégué sur le document, plus l'appel MapLibre :

```js
const carte = document.querySelector('#carte-jour');
document.addEventListener('click', (e) => {
  const cible = e.target.closest('[data-ancre]');
  if (!cible) return;
  const n = cible.dataset.ancre;
  document.querySelectorAll('.actif').forEach((el) => el.classList.remove('actif'));
  const bloc = document.getElementById('m' + n);
  if (bloc) { bloc.classList.add('actif'); bloc.focus({ preventScroll: true }); }
  carte.dispatchEvent(new CustomEvent('ancre', { detail: { n } }));
});
```

Environ 500 octets avant compression, sans dépendance. Le module de carte
écoute `ancre` et met à jour une expression `feature-state` sur la couche des
pastilles : aucun rechargement de source.

Le lien inverse (photo au centre de l'écran → anneau sur la carte) existe déjà.
Il reste, mais devient secondaire : c'est le numéro qui porte l'information.

---

## 4. Le curseur des journées

Un `<input type="range" min="0" max="23">` natif, pas un composant. Zéro montre
le voyage entier ; chaque cran isole une journée.

- **Sans JavaScript** : le curseur est masqué et remplacé par la liste des
  journées, qui est déjà sous la carte. Aucune fonction perdue, seulement le
  balayage.
- **Avec** : à chaque `input`, on met à jour trois choses — le filtre des
  couches MapLibre (`['==', ['get', 'jour'], '2026-08-08']`), le cadre de la
  fiche de journée, et le `aria-valuetext` du curseur (« jour 16 sur 23,
  samedi 8 août, Tre Cime di Lavaredo en navette »). Pas de `pushState` à
  chaque cran : un `replaceState` différé de 400 ms, pour que l'adresse reste
  partageable sans polluer l'historique.
- Les deux boutons ronds de 44 px décrémentent et incrémentent. Ils sont des
  `<button>` réels, focusables, avec `aria-label`.
- Le clavier fonctionne gratuitement : flèches sur le curseur.

Coût estimé : 1,1 à 1,4 Ko avant compression, y compris la fiche de journée et
le `aria-valuetext`.

Recentrage de la carte : `fitBounds` sur l'emprise du jour, `duration: 300`,
`essential: false` pour respecter `prefers-reduced-motion`. C'est la seule
animation du site, comme aujourd'hui.

---

## 5. La frise des journées

23 cellules `flex: 1`, chacune : numéro du jour en mono 13 px, kilomètres
**arrondis à l'entier** en mono 10 px, et une barre de 3 px à la couleur du
mode dominant. Le jour courant est en `--ink` plein.

Trois pièges rencontrés dans la maquette, à ne pas refaire :

1. `flex: 1` sans `min-width: 0` élargit la page : les cellules ne descendent
   jamais sous la largeur de leur texte. Toujours `min-width: 0`.
2. Les kilomètres non arrondis se coupent en plein milieu (« 531, ») dès que la
   cellule tombe sous ~34 px. Arrondir, et sous 640 px ne garder que le numéro
   et la barre.
3. La barre de mode est une information portée par la couleur seule : elle doit
   être doublée par l'attribut `title` de la cellule (« 8 août — Tre Cime di
   Lavaredo en navette — marche — 101 km ») et par la colonne « mode » écrite
   en texte dans la liste des journées.

Le **mode dominant** est à calculer dans `carnet/` (mode qui totalise le plus
de kilomètres sur la journée, hors route si un autre mode dépasse 5 km — sinon
tous les jours de transit ressemblent aux jours de marche) et à sérialiser dans
`data/<voyage>/jours.json`. C'est le seul champ de données nouveau que cette
direction demande.

---

## 6. Voyages sans coordonnées GPS

Le Japon, la Polynésie et la Tunisie n'auront pas de trace. La direction ne se
casse pas, à trois conditions :

- **Accueil** : l'entrée du voyage garde exactement la même forme, la colonne
  de droite affichant une vignette au lieu d'un fragment de carte. Aucune
  variante de gabarit.
- **Page de voyage** : le bloc carte + curseur est simplement absent, et la
  liste des journées remonte. Les chiffres clés perdent « kilomètres » et
  gardent journées, nuits, médias, pays.
- **Page de journée** : la colonne de droite passe de la carte à la planche
  contact du jour, en deux colonnes de vignettes. Le récit reste à sa largeur.
  Les blocs médias ne sont plus numérotés, puisqu'il n'y a rien à relier.

Le test à écrire dans `carnet/tests/contraintes.rs` : un voyage dont aucune
photo n'a de position produit un `jours.json` valide, sans champ de trace, et
le site se construit.

---

## 7. Mode sombre

Il n'est plus un échange de variables :

- Les couleurs de mode changent (§1), sinon la trace routière disparaît.
- Le fond de carte MapLibre passe sur un style sombre. Positron a une variante
  sombre chez OpenFreeMap ; à défaut, désaturer et assombrir les couches de
  fond dans le style, ce qui coûte moins qu'un second jeu de tuiles.
- Les photos ne sont ni assombries ni voilées. Un cadre de 1 px `--rule` les
  détache du fond, c'est tout.
- L'accent passe de `#d1491f` à `#e8683c` : le premier tombe à 3,1:1 sur
  `#14181a`.
- Toujours aucune bascule manuelle : `prefers-color-scheme` seul, comme
  aujourd'hui.

---

## 8. Budget

Estimations avant compression, hors MapLibre.

| Élément | Coût | Note |
| --- | --- | --- |
| Ancres numérotées | ~0,5 Ko | délégation d'événement |
| Curseur des journées | ~1,3 Ko | page de voyage seulement |
| Visionneuse existante | 986 o | inchangée |
| Frise, planche contact, en-têtes collants | 0 | CSS seul |
| Total page de journée | ~1,5 Ko | + MapLibre |
| Total page de voyage | ~1,3 Ko | + MapLibre |

Les deux pages restent très en dessous des 50 Ko hors carte. Le budget avec
carte est inchangé : la fusion supprime même un chargement de MapLibre du
parcours, puisque `/carte/` n'existe plus.

Les fontes sont le vrai arbitrage : 90 à 110 Ko de woff2, mis en cache un an,
sur un site que quinze personnes visitent plusieurs fois.

---

## 9. Ordre de travail suggéré

1. Jetons et fontes dans le layout, sans toucher aux gabarits. Le site change
   d'allure sans changer de structure : c'est le point de non-retour le moins
   risqué.
2. Frise enrichie (+ champ `mode_dominant` dans `carnet/`).
3. Fusion `/carte/` dans la page de voyage, redirection, curseur.
4. Ancres numérotées, dans `directives.ts` puis dans le module de carte.
5. Page photos : index et en-têtes collants.
6. Mode sombre, y compris le style de carte.
7. Accueil.

L'accueil vient en dernier volontairement : c'est la page la plus pauvre, mais
c'est aussi la seule dont la forme définitive dépend d'un second voyage
réellement publié.

---

## 10. Ce qui reste à trancher

- **Les fontes**, oui ou non (§1). C'est la seule décision de budget.
- **La planche contact** : la maquette en montre douze et renvoie au reste.
  Faut-il un dépliement sur place, ou le renvoi vers `/photos/` ancré sur la
  journée suffit-il ?
- **Le mode dominant** d'un jour de transit avec deux heures de marche : route
  ou marche ? La règle proposée (§5) est un choix, pas une évidence.
- **La fiche de journée** attachée au curseur : faut-il la vignette de la
  première photo du jour, comme l'étiquette de survol actuelle, ou le texte
  seul suffit-il ?
