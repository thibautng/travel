# SPEC | Site des voyages de la famille

Document de référence pour la construction du site. À lire par Claude Code au début de chaque session de développement.

**Version** 1.2 | 27 août 2026
**Statut** spécification révisée après audit EXIF des quatre dossiers sources et du poste, développement non commencé

Les modifications apportées depuis la version 1.0 sont récapitulées en annexe A.

---

## 1. Objet

Construire un journal de voyage web, familial et durable, qui rassemble textes, photos, vidéos et une carte de progression jour par jour. Modèle de référence fonctionnel : [FindPenguins](https://findpenguins.com/). Premier voyage à publier : le Tour des Alpes orientales et Slovénie, du 24 juillet au 15 août 2026, 23 jours, 22 nuits, environ 4 400 km. Le site doit ensuite accueillir les voyages précédents (Japon, Polynésie, Tunisie, Asie) et les suivants.

### Utilisateurs

- Les quatre membres de la famille, en lecture, sur ordinateur et sur téléphone.
- Des proches, par lien public non indexé.
- Deux filles de 10 et 7 ans, qui écriront peut-être une partie des textes.

### Non-objectifs

Pas de comptes utilisateurs, pas de commentaires, pas de réseau social, pas d’édition en ligne, pas de temps réel. Le contenu est écrit une fois et lu pendant des années.

### Deux régimes de données

Le site doit vivre avec deux qualités de sources très différentes, et cette distinction irrigue tout le reste du document. Elle ne se déduit pas de l’ancienneté du voyage mais de l’appareil qui a pris les photos. Mesures faites le 27 août 2026 sur les quatre dossiers disponibles.

| Voyage | Photos | Date EXIF | Position GPS | Appareils |
|---|---|---|---|---|
| 2026 Tour des Alpes | 707 | 100 % | 97 % | OPPO Reno6 Pro 5G, GoPro HERO7 |
| 2025 Tunisie | 1 105 | 91 % | 90 % | OPPO Reno6 Pro 5G |
| 2024 Japon | 966 | 100 % | 100 % | OPPO Reno6 Pro 5G |
| 2019 Polynésie | 1 356 | 86 % | **4 %** | Nokia 7 plus, Panasonic DMC-LX7 |

- **Régime riche : Alpes, Tunisie, Japon.** Même téléphone, même convention de nommage, date et position quasi systématiques. La carte se construit largement toute seule. Ces trois voyages relèvent du même pipeline, sans traitement particulier.
- **Régime pauvre : Polynésie, et vraisemblablement les voyages plus anciens.** Un appareil photo compact sans GPS et un téléphone de 2018 qui ne géolocalisait qu’exceptionnellement. La position vient du lieu déclaré pour la journée, jamais du média. Voir D5 et la section 11, lot 7.

La date, elle, est disponible partout : c’est la bonne nouvelle de l’audit. Aucun voyage n’exige de redatation massive. Les 10 à 15 % de photos sans date sont des fichiers reçus par messagerie, voir contrainte C10.

---

## 2. Décisions d’architecture

Ces décisions sont arrêtées. Ne pas les rouvrir sans discussion explicite.

### D1 | Rust pour le pipeline, pas pour le front

Le traitement des médias est écrit en Rust : lecture EXIF, filtrage des positions, génération des dérivés d’images, transcodage vidéo, production des traces GeoJSON. C’est du calcul lourd, parallélisable, sur disque, et c’est le terrain naturel de Rust.

L’interface web n’est pas en Rust. Motif : aucune bibliothèque cartographique Rust n’est utilisable en production. `maplibre-rs` est estampillé « experimental » par MapLibre eux-mêmes, sans rendu de texte, sans labels, sans symboles. Il n’existe pas non plus de binding Rust mature de MapLibre GL JS. Un front Leptos ou Dioxus reviendrait à écrire du Rust qui pilote du JavaScript à travers `wasm-bindgen`, pour un bundle plus lourd et aucun gain.

### D2 | Site statique, pas d’application serveur

Le site est généré statiquement et déployé sur un hébergement de fichiers. Pas de serveur, pas de base de données, pas de sauvegardes à maintenir. Une archive familiale doit survivre dix ans sans intervention.

### D3 | Pile retenue

| Couche | Choix | Motif |
|---|---|---|
| Pipeline médias | Rust, binaire CLI nommé `carnet` | Performance, EXIF vidéo, apprentissage |
| Générateur de site | [Astro](https://astro.build) | HTML par défaut, JavaScript uniquement là où il est nécessaire, idéal pour un site à dominante photo |
| Carte | [MapLibre GL JS](https://github.com/maplibre/maplibre-gl-js) | Standard libre, actif, BSD-3 |
| Fond de carte | [PMTiles](https://github.com/protomaps/PMTiles) + style [protomaps/basemaps](https://github.com/protomaps/basemaps), **hébergés sur R2** | Un fichier par zone sur stockage objet, lu par requêtes HTTP Range. Aucune clé d’API, aucun serveur de tuiles |
| Contenu rédactionnel | Markdown avec frontmatter, versionné dans Git | Lisible, éditable à la main, pérenne |
| Stockage médias et tuiles | Cloudflare R2 ou équivalent S3 | 10 Go gratuits, servi par CDN |
| Hébergement site | Cloudflare Pages ou Netlify | Gratuit, build automatique depuis Git |

### D4 | Repli si le fond de carte auto-hébergé pose problème

[OpenFreeMap](https://openfreemap.org) offre une instance publique gratuite, sans inscription ni clé d’API. C’est le fond de carte **par défaut pendant le développement** des lots 4 et 5, pour ne pas dépendre de R2 avant le lot 6, et le repli en cas de problème sur les PMTiles auto-hébergés.

### D5 | Le lieu porte la position, pas seulement le média

Un voyage déclare une liste de `lieux` géolocalisés. Chaque journée en désigne un. Tout média dépourvu de position exploitable **hérite** de la position du lieu de sa journée, avec `origine_position: heritee` et `fiabilite: basse`.

Motifs. Les voyages antérieurs n’ont aucune coordonnée GPS et ce mécanisme est leur seule source de position. Sur le voyage 2026, il rattrape aussi les 14 fichiers GoPro et les positions absentes, qui disparaîtraient sinon de la carte. Enfin, la granularité « un lieu par journée » est honnête : elle ne prétend pas savoir où la photo a été prise à un mètre près.

Inspiration assumée : la relation lieu / date / visite d’AdventureLog, citée en section 3.

### D6 | Les traces routières sont calculées, pas mesurées

Il n’existe aucun enregistrement GPS continu du voyage, seulement les positions EXIF des photos. Relier ces positions par des segments droits produirait des lignes qui traversent les massifs.

Pour tout mode qui dispose d’un réseau, `carnet` interroge donc un moteur d’itinéraire entre deux positions fiables consécutives, et **fige le résultat dans un cache versionné**. Le build reste reproductible hors ligne et sans dépendance permanente à un service tiers.

**Chaque mode calculable va sur son propre réseau.** La route emprunte le profil `driving-car`, le vélo le profil `cycling-regular`, qui suit les pistes cyclables et non la chaussée, la marche le profil `foot-walking`, qui suit les sentiers. Les modes `bateau`, `train` et `telepherique` ne sont **jamais** calculés : segments droits, ou tracé manuel via `segments`.

La règle n’est donc pas « seule la route est calculée » mais « on ne calcule un mode que sur un réseau qui lui correspond ». Un bateau n’a pas d’itinéraire, et un train ne suit pas la route. La garde reste un refus et non une convention : `resoudre()` renvoie une erreur pour tout mode dépourvu de profil, et un test le vérifie sur les trois.

*Amendement du 30 août 2026.* La marche était exclue du calcul, au motif qu’une randonnée envoyée au moteur suivrait les départementales. C’est vrai de `driving-car`, faux de `foot-walking`, qui connaît les sentiers. Le tour de l’Eibsee, le 27 juillet, coupait le lac en droite entre deux photos alors que le chemin en fait le tour. La marche rejoint donc le vélo parmi les modes calculables. La contrepartie est un volume d’appels sensiblement plus élevé, les journées à pied étant les mieux photographiées : le premier build après ce changement a épuisé le quota journalier du palier gratuit.

**Le moteur peut refuser, et son refus se voit.** Un appel refusé retombait sur le segment droit sans rien dire, indistinguable d’un tronçon qu’on n’avait pas cherché à calculer. Un refus 429 déclenche désormais une attente d’une minute puis une seule nouvelle tentative — la minute peut être pleine sans que la journée le soit ; si elle échoue, le build cesse d’appeler et le rapport annonce le quota épuisé avec le nombre de tronçons restés droits. Le cache étant conservé, un build relancé le lendemain ne recalcule que ce qui manque.

**Les journées de déplacement se tracent d’un camp à l’autre.** Les photos ne documentent pas les trajets : sur les 4 400 km annoncés, elles n’en dessinaient que 888, et les jours de transit sont précisément ceux où l’on photographie le moins. Le 24 juillet, jour du départ, porte cinq médias et ne produisait aucune trace.

Quand le camp du soir diffère de celui de la veille, l’itinéraire routier de l’un à l’autre est donc calculé, et la trace produite est de source `heritee` : elle ne prétend pas dire par où l’on est passé, elle dit d’où l’on est parti et où l’on est arrivé. Elle remplace, ce jour-là, les tronçons routiers déduits de la vitesse, qui décrivent le même trajet en moins bien. Au premier et au dernier jour du voyage, où l’un des deux camps manque, la position connue la plus extrême de la journée en tient lieu.

Le résultat porte le total de 888 à **2 691 km**, dont 2 159 hérités. Il ne s’agit pas d’empiler les deux lectures : sur une journée de transit, l’itinéraire entre camps remplace les tronçons routiers déduits de la vitesse, qui décrivaient le même trajet en moins bien. L’écart restant aux 4 400 km tient aux excursions parties et revenues au même camp dont le retour n’est pas photographié.

Un itinéraire calculé est le trajet le plus rapide, pas nécessairement celui qui a été pris. Sur les cols alpins l’écart peut être franc, un tunnel au lieu d’un col. D’où deux exigences : `overrides.yaml` peut imposer des points de passage sur un segment `route`, et le rendu distingue visuellement le tracé mesuré du tracé calculé (section 9.2).

### D7 | Tout n’est pas publié

Le dossier source est une archive, le site est un récit. Les deux n’ont pas la même vocation, et 833 médias pour 23 journées, ce n’est pas un journal de voyage, c’est une sauvegarde.

Un fichier `content/voyages/<id>/selection.yaml` désigne donc les médias retenus. Trois règles :

- tout média cité par une directive `::photo`, `::galerie` ou `::video` dans le récit est retenu d’office, sans avoir à le répéter ;
- `retenus` et `exclus` complètent ce choix à la main, par identifiant ou par motif ;
- en l’absence de ce fichier, tout est retenu, ce qui préserve le comportement actuel.

Conséquences. `media.json` continue de décrire **tous** les médias, avec un champ `publie`, pour que l’inventaire reste complet et que rien ne se perde. Mais seuls les médias retenus reçoivent des dérivés et montent sur R2. C’est le levier principal sur le coût d’hébergement : les 10 Go gratuits de R2 sont largement suffisants pour une sélection, ils le seraient beaucoup moins pour l’intégralité des vidéos.

La route « toutes les photos du voyage » de la section 9.1 s’entend donc comme toutes les photos **retenues**.

### D8 | Les vidéos attendent

Le pipeline prévoit le transcodage vidéo (section 6.2, étape 10) et la fiabilité de leur position (C1), mais **la génération des dérivés vidéo reste en pause** jusqu’à ce que la sélection soit faite. Motif : 5,6 Go de source pour 128 vidéos, dont une partie seulement mérite d’être publiée, et le transcodage est de loin l’étape la plus coûteuse du pipeline.

Concrètement, le drapeau `--videos` existe, la sélection le précède. Les vidéos restent inventoriées, datées, positionnées et visibles dans les statistiques.

---

## 3. Il n’existe rien à forker

Recherche menée le 26 août 2026. Aucun projet open source ne reproduit le modèle FindPenguins, à savoir entrées datées avec texte long, photos, vidéos, carte de progression et publication publique responsive. Les projets existants se répartissent en trois familles disjointes.

- **Planificateurs avec module journal** : [AdventureLog](https://github.com/seanmorley15/AdventureLog) (Svelte, Django, PostGIS, MapLibre, GPL-3.0, actif), [TREK](https://github.com/liketrek/TREK) (NestJS, React, AGPL-3.0). Centre de gravité : préparer le voyage, pas le raconter.
- **Timelines GPS enrichies de photos** : [Dawarich](https://github.com/Freika/dawarich), [Reitti](https://github.com/dedicatedcode/reitti), GeoPulse. Carte et chronologie, pas de texte long.
- **Galeries photo avec vue carte** : [Immich](https://github.com/immich-app/immich), [PhotoPrism](https://github.com/photoprism/photoprism), [Photofield](https://github.com/SmilyOrg/photofield). GPS EXIF géré, récit absent.

Le seul clone conceptuel direct, [OpenStep](https://github.com/TheoLechemia/openstep), est un prototype à 3 étoiles sans licence déclarée. [mappics](https://github.com/antodippo/mappics), la galerie cartographique la plus proche du besoin, a été archivée en juillet 2026.

**À lire pour s’en inspirer, sans forker** : le modèle de données d’AdventureLog (relation lieu, date, visite, média), repris en D5, et l’approche de reverse geocoding local embarqué de Photofield (environ 50 000 lieux, sans appel réseau).

---

## 4. Arborescence du dépôt

```
voyages/
  SPEC.md                      ce document
  README.md
  carnet/                      pipeline Rust
    Cargo.toml
    src/
      main.rs                  CLI, sous-commandes
      scan.rs                  parcours du dossier source, lecture EXIF
      noms.rs                  normalisation des noms, identifiants, collisions
      quality.rs               scoring de fiabilité des positions
      overrides.rs             application du fichier de surcharge
      lieux.rs                 héritage de position, interpolation
      itineraire.rs            appel au moteur d’itinéraire, cache
      derive.rs                génération des dérivés image et vidéo
      track.rs                 construction des traces GeoJSON par journée
      emit.rs                  écriture des JSON de sortie
    tests/
      fixtures/                un fichier d’exemple par contrainte C1 à C8
  content/                     contenu rédactionnel, versionné, écrit à la main
    voyages/
      2026-alpes/
        voyage.yaml            métadonnées du voyage, lieux, section 5.1
        overrides.yaml         corrections manuelles, section 7
        selection.yaml         médias retenus pour publication, D7
        jours/
          2026-07-24.md
          2026-07-25.md
          ...
          2026-08-15.md
  data/                        généré par carnet, versionné (JSON léger)
    2026-alpes/
      media.json
      jours.json
      trace.geojson
      itineraires.json         cache des appels au moteur d’itinéraire
      .build-cache.json        empreintes de build, section 6.2
  site/                        application Astro
    src/
      pages/
      components/
      layouts/
      styles/
    public/
  media/                       généré par carnet, NON versionné, poussé vers R2
    2026-alpes/
      photos/
      videos/
      posters/
```

Deux règles de frontière, non négociables.

1. `content/` est écrit à la main. `data/` et `media/` sont produits par `carnet` et ne se modifient jamais à la main. Toute correction passe par `overrides.yaml`, puis `carnet build`.
2. **`data/` ne contient jamais de contenu rédactionnel.** Ni titre, ni texte, ni légende rédigée. Uniquement des faits mesurés ou calculés. Le texte a une seule source, `content/`.

Le fond de carte n’est pas dans le dépôt. Les fichiers PMTiles vivent sur R2 et leur URL est une variable d’environnement du site. Motif : Cloudflare Pages plafonne à 25 Mo par fichier, et un extrait régional en pèse plusieurs centaines.

---

## 5. Modèle de données

### 5.1 Voyage

```yaml
# content/voyages/2026-alpes/voyage.yaml
id: 2026-alpes
titre: Tour des Alpes orientales et Slovénie
sous_titre: Allemagne, Autriche, Slovénie, Italie
date_debut: 2026-07-24
date_fin: 2026-08-15
pays: [DE, AT, SI, IT, FR]
distance_km: 4400
nuits: 22
mode: voiture et camping
fuseau: Europe/Paris
source_photos: "P:/Photos/Thibaut/2026 Tour des Alpes"
dossiers_ignores: ["[Originals]", ".thumbnails", "@eaDir"]
notion: "https://app.notion.com/p/371ec81aae12810fa2e9dfbe7adbcff8"

lieux:
  - id: valnontey
    nom: Valnontey, Cogne
    type: camp
    position: { lat: 45.5928, lon: 7.3389 }
    du: 2026-08-12
    au: 2026-08-15
  - id: lillaz
    nom: Lillaz, Cogne
    type: etape
    position: { lat: 45.6053, lon: 7.3878 }
```

`fuseau` est obligatoire. Il donne l’heure locale de référence du voyage, faute de quoi le rattachement d’un média à sa journée civile n’a pas de sens. Pour le voyage 2026, un fuseau unique suffit : DE, AT, SI, IT et FR sont tous en CEST. Un voyage à escales multiples devra faire évoluer ce champ, voir section 6.2, étape 2.

`dossiers_ignores` liste les sous-dossiers à ne pas parcourir. Voir contrainte C8.

`lieux` porte les positions de référence du voyage. `type` prend `camp` (on y dort, marqueur distinct sur la carte) ou `etape`. Les champs `du` et `au` ne concernent que les camps. Les coordonnées d’un lieu se saisissent une fois, à la main, éventuellement en géocodant son nom au moment de la rédaction. Elles ne sont jamais géocodées au build.

### 5.2 Journée

Un fichier Markdown par journée, nommé par sa date. Le frontmatter porte les métadonnées, le corps porte le récit.

```markdown
---
date: 2026-08-14
titre: Lillaz, la grande boucle de l’Urtier et les dentellières
lieu: lillaz
camp: valnontey
etiquettes: [randonnee, fete, artisanat]
distance_marche_km: 14
denivele_m: 880
altitude_max_m: 2396
temps_fort: true
couverture: IMG20260814151616
---

La plus longue marche du voyage, et la dernière.

::photo{id="IMG20260814151616" legende="La montée vers le Lago di Loie"}

Le texte continue...

::galerie{ids="IMG20260814140048,IMG20260814142050,IMG20260814144903"}
```

`lieu` et `camp` référencent un `id` déclaré dans `voyage.yaml`. `lieu` est le point d’ancrage de la journée : c’est de lui qu’héritent les médias sans position (D5). `carnet check` échoue si une journée cite un identifiant inconnu.

`couverture` est facultatif : à défaut, la couverture est le premier média fiable de la journée.

Les champs chiffrés (`distance_marche_km`, `denivele_m`, `altitude_max_m`) sont facultatifs et purement rédactionnels. Le site n’affiche que ceux qui sont renseignés.

Les directives `::photo`, `::galerie` et `::video` sont des composants Markdown personnalisés résolus au build. Elles ne portent qu’un identifiant : toutes les métadonnées viennent de `media.json`.

### 5.3 Identifiant de média

L’identifiant est le nom de fichier privé de son extension, normalisé : tout caractère hors `[A-Za-z0-9_-]` est remplacé par un tiret. `IMG20260808113008~2.jpg` donne donc `IMG20260808113008-2`.

Motif du choix : ces identifiants sont **tapés à la main** dans `::photo{id="..."}`. Ils doivent rester devinables à partir du nom du fichier. Un hash de contenu serait plus robuste au renommage et illisible dans le Markdown. Un identifiant dérivé de l’horodatage serait instable, puisque C3 et C4 établissent que l’EXIF de certains fichiers est faux ou absent.

Les suffixes de variante (`_01`, `~2`) sont **conservés** dans l’identifiant : ce sont des images différentes, souvent une version retouchée à côté de son original, et le choix de celle qui est publiée appartient à la rédaction.

L’unicité est vérifiée. **Une collision d’identifiant fait échouer `carnet scan` avec la liste des fichiers en cause.** Aucun arbitrage automatique, aucun « le plus récent gagne » : c’est ainsi que naissent les archives fausses. La résolution se fait dans `overrides.yaml`, par exclusion ou par renommage explicite.

`fichier_source` conserve toujours le nom exact du fichier d’origine, tilde compris, ainsi que son chemin relatif au dossier source.

### 5.4 Média

```json
{
  "id": "IMG20260814151616",
  "type": "photo",
  "fichier_source": "IMG20260814151616.jpg",
  "prise_le": "2026-08-14T15:16:16+02:00",
  "jour": "2026-08-14",
  "position": { "lat": 45.58542, "lon": 7.42340, "alt": 2381 },
  "fiabilite": "haute",
  "origine_position": "exif",
  "lieu": null,
  "publie": true,
  "anomalies": [],
  "largeur": 4096,
  "hauteur": 3072,
  "orientation": 1,
  "appareil": "OPPO Reno6 Pro 5G",
  "derives": {
    "vignette": "photos/IMG20260814151616-320.avif",
    "moyen": "photos/IMG20260814151616-1024.avif",
    "grand": "photos/IMG20260814151616-2048.avif",
    "repli": "photos/IMG20260814151616-1024.jpg"
  },
  "lqip": "data:image/webp;base64,..."
}
```

`fiabilite` prend trois valeurs.

| Valeur | Signification |
|---|---|
| `haute` | Position issue de l’EXIF avec une **altitude non nulle**, ou position posée à la main dans `overrides.yaml` |
| `basse` | Position à altitude strictement nulle, sans altitude, héritée d’un lieu, ou interpolée |
| `absente` | Aucune position, ni EXIF, ni surcharge, ni lieu de rattachement |

Le critère est l’altitude **non nulle**, et non la simple présence du champ. Voir contrainte C1 : sur les 688 photos géolocalisées du Tour des Alpes, le champ `GPSAltitude` est présent 688 fois. C’est sa valeur nulle qui trahit la position réseau.

`origine_position` prend `exif`, `override`, `heritee` ou `interpolee`.

`lieu` porte l’identifiant du lieu dont la position a été héritée, et vaut `null` sinon.

`publie` dit si le média est retenu pour le site (D7). Un média non retenu reste décrit ici, mais ne reçoit aucun dérivé et ne monte pas sur R2.

`anomalies` est un tableau, éventuellement vide, qui documente **pourquoi** un média est suspect. Valeurs : `altitude_nulle`, `position_clonee`, `nom_menteur`, `horloge_perdue`, `homonyme`, `nom_normalise`, `exif_absent`, `date_du_nom`, `hemisphere_absent`.

La distinction entre `fiabilite` et `anomalies` est délibérée. `fiabilite` est le verdict, consommé par la carte. `anomalies` est le motif, consommé par `carnet stats` et par l’humain qui écrit `overrides.yaml`. Une position peut porter une anomalie sans être déclassée : deux médias aux coordonnées identiques mais avec altitude sont signalés, sans perdre leur fiabilité haute.

Le champ `repli` du bloc `derives` n’existe qu’en une seule taille. Voir section 6.2, étape 8.

### 5.5 Jour agrégé

`jours.json` est un index **strictement dérivé**, calculé par `carnet`. Il évite au site de recharger `media.json` entier pour dessiner une frise, une page de voyage ou le cadrage initial d’une carte. Conformément à la règle de la section 4, il ne contient aucun texte rédigé.

```json
{
  "jour": "2026-08-14",
  "lieu": "lillaz",
  "camp": "valnontey",
  "premiere_prise": "2026-08-14T08:12:04+02:00",
  "derniere_prise": "2026-08-14T21:47:33+02:00",
  "medias": { "photo": 42, "video": 6, "total": 48 },
  "couverture": "IMG20260814151616",
  "bbox": [7.3020, 45.5510, 7.4480, 45.6210],
  "distance_trace_km": 14.2,
  "modes": ["marche"],
  "anomalies": ["trou_candidat"]
}
```

### 5.6 Trace

Un `FeatureCollection` GeoJSON par voyage. Une `Feature` de type `LineString` par journée et par mode, plus les points de médias en `Point`.

```json
{
  "type": "Feature",
  "geometry": { "type": "LineString", "coordinates": [[7.3947, 45.5947], ...] },
  "properties": {
    "jour": "2026-08-14",
    "mode": "marche",
    "source": "mesuree",
    "couleur": "#c0562a"
  }
}
```

`mode` prend `route`, `marche`, `velo`, `bateau`, `train`, `telepherique`. La couleur de la trace est dérivée du mode, pas stockée à la main.

`source` prend quatre valeurs, qui déterminent le style de rendu (section 9.2).

| Valeur | Origine |
|---|---|
| `mesuree` | Positions EXIF fiables reliées entre elles |
| `calculee` | Itinéraire produit par le moteur de routage, mode `route` uniquement |
| `manuelle` | Points saisis dans `segments` d’`overrides.yaml` |
| `heritee` | Itinéraire calculé entre deux lieux déclarés : transit d’un camp au suivant (D6), ou polyligne des lieux successifs faute de toute position de média |

---

## 6. Le pipeline Rust : `carnet`

### 6.1 Sous-commandes

```
carnet scan     <voyage>   lit le dossier source, produit un inventaire brut
carnet build    <voyage>   inventaire + surcharges + dérivés + traces, produit data/ et media/
carnet check    <voyage>   contrôles de cohérence, sans rien écrire
carnet stats    <voyage>   récapitulatif : nb médias, couverture GPS, trous horaires, anomalies
```

Il n’existe pas de sous-commande `push`. La synchronisation de `media/` vers R2 se fait avec `rclone`, documenté dans le README. Motif : `rclone` gère déjà le différentiel, la reprise, les sommes de contrôle et le parallélisme. Le réécrire en Rust ajouterait une dépendance lourde pour un gain nul.

`carnet build` est idempotent. Un média déjà dérivé, dont la source n’a pas changé (taille et mtime) **et dont les paramètres d’encodage n’ont pas changé**, n’est pas retraité. Voir étape 14.

### 6.2 Étapes de `build`

1. **Inventaire.** Parcours récursif du dossier source, en sautant les `dossiers_ignores`. Pour chaque fichier, lecture EXIF avec `nom-exif` : `DateTimeOriginal`, `OffsetTimeOriginal`, `GPSLatitude`, `GPSLatitudeRef`, `GPSLongitude`, `GPSLongitudeRef`, `GPSAltitude`, `GPSAltitudeRef`, modèle d’appareil, dimensions, orientation. **Les références d’hémisphère sont appliquées** : `S` nie la latitude, `W` nie la longitude, `GPSAltitudeRef` à 1 nie l’altitude (contrainte C9). Normalisation du nom et calcul de l’identifiant (5.3). Contrôle d’unicité, échec bruyant en cas de collision.
2. **Datation.** La date de prise de vue vient de l’EXIF, jamais du nom de fichier (contrainte C3). Si `OffsetTimeOriginal` est présent, il fait foi. Sinon, l’horodatage est interprété dans le `fuseau` du voyage. Aucune résolution de fuseau à partir des coordonnées n’est faite : voir la note de fin de section.

   Un fichier **totalement dépourvu d’EXIF** est daté par son nom, si celui-ci porte une date reconnaissable, avec l’anomalie `date_du_nom` (contrainte C10). L’ordre de préséance est donc : EXIF, puis nom, puis exclusion. **Jamais la date de modification du fichier**, qui ne survit pas aux copies et aux sauvegardes.
3. **Scoring.** `fiabilite: haute` si la position porte une altitude **non nulle**, `basse` si l’altitude est absente ou strictement nulle (anomalie `altitude_nulle`). En parallèle, détection des positions clonées : deux médias séparés de plus de 20 minutes et distants de moins de **25 mètres** reçoivent l’anomalie `position_clonee`, **quelle que soit leur altitude**. Le clone ne déclasse pas à lui seul : c’est un signalement, pas un verdict (contraintes C1 et C2).
4. **Surcharges.** Application d’`overrides.yaml`. Les valeurs manuelles écrasent tout, y compris le verdict de fiabilité : une position posée à la main est `haute` et `origine_position: override`.
5. **Rattachement au jour.** Un média appartient au jour civil de sa prise de vue, en heure locale du voyage.
6. **Héritage.** Tout média encore sans position reçoit celle du `lieu` de sa journée, avec `origine_position: heritee` et `fiabilite: basse` (D5). Si la journée ne déclare pas de lieu, la position reste `absente`.
7. **Interpolation.** Un média sans position EXIF, encadré dans la même journée par deux positions fiables, reçoit une position pondérée par le temps, sous garde-fous : moins de 30 minutes d’écart de part et d’autre, et moins de 5 km entre les deux bornes. `origine_position: interpolee`, `fiabilite: basse`. L’interpolation prime sur l’héritage, qui est plus grossier.
8. **Sélection.** Lecture de `selection.yaml` et des directives du récit, renseignement du champ `publie` (D7). Les étapes suivantes ne traitent que les médias retenus.
9. **Dérivés images.** Trois largeurs, 320, 1024 et 2048 pixels. Le format est un **paramètre du pipeline**, dont la valeur par défaut est AVIF (encodage `ravif`), avec WebP comme valeur alternative sur une machine dépourvue d’AVX2. Un unique repli JPEG en 1024 est produit dans tous les cas, comme filet de sécurité pour les appareils anciens. Redimensionnement avec `fast_image_resize`. Rotation appliquée selon l’orientation EXIF, puis métadonnées EXIF supprimées des dérivés, hors copyright.
10. **LQIP.** Génération d’un aperçu de 16 pixels de large encodé en base64, inline dans le JSON, pour un rendu progressif sans requête supplémentaire.
11. **Dérivés vidéos.** Appel à `ffmpeg` en sous-processus : H.264 720p, CRF 23, audio AAC 128 kb/s, plus une image poster extraite à 1 seconde. `ffmpeg` est une dépendance externe assumée, ne pas tenter de transcoder en Rust pur. Étape activée par le drapeau `--videos`, et **en pause** jusqu'à la sélection (D8).
12. **Itinéraires.** Pour chaque couple de positions fiables consécutives d’une journée en mode `route`, résolution de l’itinéraire par le moteur de routage, puis écriture dans `data/<voyage>/itineraires.json`. **Le cache est consulté avant tout appel réseau** : un build ultérieur n’émet aucune requête. La clé de cache est le couple de coordonnées arrondi, le mode et les éventuels points de passage.
13. **Traces.** Pour chaque journée et chaque mode, tri des positions par heure, construction des `LineString`, substitution des tronçons `route` par leur itinéraire calculé, injection des segments manuels d’`overrides.yaml`. Renseignement de `source` pour chaque `Feature`.
14. **Pose sur la trace.** Les positions `basse` encadrées dans la journée par deux positions fiables sont replacées sur le tronçon qui relie ces deux bornes, à la fraction que dit leur horodatage. `origine_position: posee`, la fiabilité reste `basse`. Cette étape vient après les traces parce qu’elle en dépend, et les tronçons n’en sont pas affectés : ils sont bâtis sur les seules positions fiables. Garde-fous : 90 minutes d’écart au plus de part et d’autre, et un tronçon qui relie effectivement les deux bornes, à 60 mètres près.

15. **Émission.** Écriture de `media.json`, `jours.json`, `trace.geojson` et `itineraires.json` dans `data/<voyage>/`, des fichiers dérivés dans `media/<voyage>/`, et de `.build-cache.json`, qui porte pour chaque source son couple taille et mtime **ainsi qu’une empreinte des paramètres d’encodage** (format, qualité, tailles, version du pipeline). Un changement de paramètre invalide le cache. Le drapeau `--force` ignore le cache.

Note sur les fuseaux. `chrono-tz` convertit une zone connue, il ne la déduit pas de coordonnées. Le champ `fuseau` du voyage suffit au Tour des Alpes et à tout voyage tenant dans un seul fuseau. Le jour où un voyage traversera plusieurs fuseaux (Japon, Polynésie), deux options seront à trancher : une liste de fuseaux datés dans `voyage.yaml`, ou l’ajout de `tzf-rs` pour résoudre la zone à partir de la position. Ne rien implémenter avant d’en avoir besoin.

### 6.3 Dépendances Rust

```toml
[dependencies]
nom-exif = "3"              # EXIF photos ET vidéos, activement maintenu
image = "0.25"
fast_image_resize = "6"
ravif = "0.13"
webp = "0.3"                # format alternatif, machine sans AVX2
geojson = "1"
geo = "0.33"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_norway = "0.9"        # remplace serde_yaml, archivé en 2024
chrono = "0.4"
chrono-tz = "0.10"
rayon = "1"                 # parallélisation du traitement des médias
clap = { version = "4", features = ["derive"] }
walkdir = "2"
indicatif = "0.17"          # barre de progression
ureq = "3"                  # appels au moteur d’itinéraire, client HTTP léger et bloquant
anyhow = "1"
thiserror = "2"
```

Ne pas utiliser `kamadak-exif` : sans mise à jour depuis novembre 2024 et sans support vidéo. Ne pas utiliser `exif-oxide` : explicitement non production-ready, et double licence commerciale ou AGPL.

`serde_yaml` 0.9 est **abandonné**, son dépôt est archivé depuis 2024. Le remplaçant retenu est `serde_norway`, fork maintenu à l’API identique. Vérifier sa fraîcheur sur crates.io au moment du lot 2, et se rabattre sur `serde_yaml_ng` s’il est plus vivant. Le format YAML est conservé : `overrides.yaml` est écrit à la main, avec des blocs imbriqués et des notes en français, ce que le TOML rendrait pénible.

`ureq` est préféré à `reqwest` : le besoin est de quelques centaines d’appels bloquants, sans asynchrone, et cela évite de tirer tout l’écosystème Tokio.

### 6.4 Performance attendue

835 fichiers, 8,6 Go. Deux machines sont en jeu, et la feuille de route en tient compte.

- **Poste actuel**, Intel i7-2600K, 4 cœurs et 8 threads, 16 Go, **sans AVX2**. `rav1e` et `ravif` en dépendent fortement : l’encodage AVIF y est plus lent d’un ordre de grandeur. Ce poste convient parfaitement aux lots 1 et 2, qui ne produisent aucune image : l’inventaire complet des 835 fichiers doit y prendre moins de 30 secondes.
- **Machine cible**, commandée, disponible sous quelques jours, avec AVX2. C’est elle qui exécutera le lot 3.

Budget : la génération des dérivés images des 705 photos doit prendre moins de 3 minutes **sur la machine cible**. Le temps réel du premier build est mesuré et consigné dans le README. Si le lot 3 devait finalement s’exécuter sur le poste actuel, basculer le paramètre de format sur WebP : environ 25 % de poids en plus, pour un budget qui redescend à quelques minutes.

Le transcodage vidéo domine le temps total et se compte en dizaines de minutes sur l’une comme sur l’autre machine, d’où le drapeau `--videos`.

### 6.5 Environnement requis

| Outil | Nécessaire à partir du | État sur le poste au 27 août 2026 |
|---|---|---|
| `cargo` et `rustc` | lot 1 | absent, à installer |
| `ffmpeg` dans le `PATH` | lot 3 | absent, à installer |
| Clé du moteur d’itinéraire, variable `CARNET_ORS_CLE` | lot 2 | à créer |
| `rclone` | lot 6 | absent, à installer |
| `node` et `npm` | lot 4 | présents, node 24 |
| `git` | lot 1 | présent |

Moteur d’itinéraire retenu : [OpenRouteService](https://openrouteservice.org), clé gratuite, quota de 2 000 requêtes par jour, très supérieur au besoin, avec des conditions d’usage explicites. Repli en cas d’indisponibilité : tracé manuel des étapes dans `segments`.

---

## 7. Le fichier de surcharge

`content/voyages/<id>/overrides.yaml`. C’est la pièce maîtresse de la fiabilité du projet. Sans lui, les erreurs GPS deviennent définitives.

```yaml
# Corriger la position d’un média
medias:
  IMG20260731102820:
    position: { lat: 47.4880, lon: 13.0055, alt: 620 }
    note: "Fischunkelalm, GPS décroché dans la vallée du Königssee"
  IMG_20260730_071148:
    prise_le: "2026-07-28T12:36:27+02:00"
    note: "Fichier repartagé, le nom porte la date du partage"

# Ignorer un média
exclusions:
  - IMG20260808120006

# Redater en bloc un lot de fichiers sans EXIF fiable
lots:
  - motif: "GOPR2*.JPG"
    jour: 2026-08-02
    position: { lat: 46.2010, lon: 13.6480 }
    repartir_sur_segment: true
    note: "GoPro sans GPS, horloge perdue. Canyoning du Sušec."

# Segments de trace tracés à la main
segments:
  - jour: 2026-08-06
    mode: velo
    apres: IMG20260806133042
    points: [[12.5320, 46.7575], [12.6100, 46.7900], [12.7690, 46.8290]]
    note: "Seconde moitié Tassenbach vers Lienz, aucune photo prise"
  - jour: 2026-07-31
    mode: bateau
    points: [[12.9887, 47.5893], [12.9797, 47.5225], [12.9847, 47.4967]]
    note: "Königssee, Schönau vers St. Bartholomä puis Salet"

# Corriger le mode de déplacement, à la journée ou à la tranche horaire
modes:
  - jour: 2026-08-06
    mode: velo
    de: "13:30"
    a: "18:00"
    note: "Tassenbach vers Lienz"
  - jour: 2026-08-06
    mode: route

# Forcer le passage d’un itinéraire calculé
itineraires:
  - jour: 2026-08-08
    mode: route
    points_de_passage: [[12.2960, 46.6180]]
    note: "Passage par le col, le moteur proposait le tunnel"
```

Règle de priorité : `overrides` écrase `exif`, toujours, sans avertissement. `carnet check` liste les surcharges appliquées pour qu’elles restent visibles.

Attention à la portée du motif : les fichiers GoPro vont de `GOPR2699` à `GOPR2717`, et le motif `GOPR27*` en raterait le premier. Un motif trop étroit ne prévient pas, il applique la correction à une partie du lot seulement. `carnet check` affiche le nombre de fichiers touchés par chaque lot, c’est ce compte qu’il faut vérifier.

`repartir_sur_segment` répartit les médias d’un lot le long du segment manuel de la journée, au lieu de les empiler sur un point unique. L’ordre relatif vient de l’horodatage EXIF, faux en absolu mais cohérent en relatif, avec repli sur le numéro de fichier.

`points_de_passage` impose au moteur d’itinéraire de passer par les points donnés, pour corriger un trajet plausible mais faux (D6). L’ordre compte : sur le retour du 15 août, le seul col du Petit-Saint-Bernard ne suffisait pas, le moteur y montait puis redescendait en Italie pour prendre le tunnel du Mont-Blanc, qu’il jugeait plus rapide. Un second point relevé près de Lyon a forcé la descente par la Tarentaise.

`calculer: true` sur un segment fait tracer la route entre ses points par le moteur, au lieu de les relier par des droites. C’est ce qui permet de déclarer un trajet routier qu’aucune photo ne documente : le 31 juillet, la voiture s’est arrêtée à l’embarcadère du Königssee avant la première photo de la journée, et le trajet depuis le camping n’existait nulle part. Le drapeau reste sans effet sur les modes non calculables, où la garde de D6 s’applique.

`modes` corrige le mode de déplacement inféré. L’inférence par la vitesse entre deux photos consécutives est une proposition, jamais un verdict : une voiture arrêtée pour déjeuner affiche la vitesse moyenne d’un vélo, et la première exécution du lot 2 a effectivement classé en `velo` plusieurs centaines de kilomètres parcourus en voiture. Une entrée sans `de` ni `a` couvre la journée entière ; la première règle qui couvre l’instant l’emporte, donc une tranche horaire déclarée avant la règle de journée prime sur elle.

Corriger les modes n’est pas cosmétique : c’est ce qui décide quels tronçons partent au moteur d’itinéraire (D6), et donc si la trace suit les routes ou coupe à travers les massifs.

---

## 8. Contraintes de données connues

Constats établis le 27 août 2026. Les contraintes C1 à C5 proviennent de l’analyse détaillée de 251 photos du dossier `2026 Tour des Alpes`, les contraintes C6 à C8 d’un inventaire exhaustif de ses noms de fichiers, les contraintes C1, C9 et C10 d’un audit EXIF des quatre dossiers sources, soit 4 134 photos. Le pipeline doit toutes les traiter dès la première version.

Chacune de ces dix contraintes a un test avec un fichier d’exemple dans `carnet/tests/fixtures/`.

**C1 | Un tiers des positions sont approximatives, et c’est l’altitude nulle qui les trahit.** Les positions réseau, prises quand le téléphone n’accroche pas les satellites, sont souvent recopiées d’un média à l’autre. Exemple : le 28 juillet, cinq photos affichent exactement les mêmes coordonnées entre 11 h 50 et 18 h 16, alors que la famille était à la Höllentalangerhütte à 12 h 36, position confirmée par une altitude de 1 428 mètres.

La formulation de la version 1.0, « position sans altitude = suspecte », était **inopérante**. Mesure faite sur les quatre dossiers : le champ `GPSAltitude` n’est jamais absent quand un bloc GPS existe, pas une seule fois sur 4 134 photos. Une règle fondée sur son absence n’aurait jamais rien déclassé.

Ce qui distingue réellement les deux populations est la **valeur strictement nulle** de l’altitude :

| Dossier | Photos géolocalisées | Altitude absente | Altitude exactement 0 | Altitude réelle |
|---|---|---|---|---|
| 2026 Tour des Alpes | 688 | 0 | 219 (32 %) | 469 |
| 2025 Tunisie (échantillon 400) | 356 | 0 | 82 (23 %) | 274 |
| 2024 Japon (échantillon 400) | 400 | 0 | 85 (21 %) | 315 |
| 2019 Polynésie | 56 | 0 | 0 | 56 |

Les preuves sont sans ambiguïté : dix-huit photos du 27 juillet portent la même position en montagne avec une altitude de 0, et les photos prises à Roissy le 19 octobre 2024 affichent elles aussi 0 mètre, pour un aéroport situé à 119 mètres. À l’inverse, une vraie mesure au niveau de la mer donne une décimale, jamais un zéro exact : les 56 positions polynésiennes, toutes littorales, s’échelonnent de 6,2 à 579,4 mètres.

Règle : **altitude non nulle = position fiable, altitude nulle ou absente = position suspecte**. Les positions `basse` ne sont pas affichées sur la carte par défaut, mais restent dans `media.json`. Le taux d’environ 30 % annoncé en version 1.0 est confirmé : 223 photos déclassées sur 691 géolocalisées, soit 32 %.

**Cette règle ne vaut que pour les photos.** La première exécution du pipeline l’a établi : les 128 vidéos du voyage portent une position, et **aucune ne porte d’altitude**. Le conteneur MP4 range la position dans une chaîne ISO 6709 dont ce téléphone omet la composante verticale. Appliquée telle quelle, la règle déclasse donc 100 % des vidéos.

C’est sans conséquence sur la trace, qui se construit à partir des photos, bien plus nombreuses et mieux réparties. C’en serait une sur l’affichage : une vidéo n’apparaîtrait jamais en pastille sur la carte.

**Règle retenue : une vidéo est fiable si une photo proche la confirme **et** si sa position n’est pas clonée.** L’altitude n’est pas un discriminant pour ce format, le conteneur MP4 ne la porte pas.

La confirmation par une photo demande deux conditions cumulées : une photo de fiabilité `haute` prise à moins de **dix minutes** de la vidéo, et une distance de moins de **500 mètres** entre les deux positions. La seconde n’est pas un excès de prudence : sans elle, une vidéo dont la position est gelée serait promue par une photo prise au même moment ailleurs.

*Amendement du 30 août 2026.* Les deux critères étaient d’abord alternés : confirmation **ou** absence de clonage, ce qui promouvait 110 vidéos sur 128. La carte a montré le défaut. Au bord de l’Eibsee, quatre vidéos portaient un relèvement d’antenne tombé au milieu du lac. Deux ont échappé au clonage — elles portaient la position du groupe de seize photos à trois mètres près, et C2 comparait alors au mètre. Les deux autres étaient bien clonées, mais une photo de la rive prise à quatre cents mètres les « confirmait » : le lac est plus petit que le seuil de confirmation. Toutes quatre promues fiables, elles servaient d’ancres à la trace, et le tour du lac à pied traversait l’eau.

Les deux critères sont donc cumulés, ce qui restitue la règle telle qu’elle avait été énoncée : la vidéo hérite de la fiabilité de la photo la plus proche dans le temps. Sans photo pour l’appuyer, il n’y a rien dont hériter. 32 vidéos sur 128 sont désormais fiables.

Les 96 autres ne sont pas perdues pour la carte : l’étape de pose (6.2, étape 14) les replace sur la trace du jour à l’heure qu’elles portent, et elles s’affichent en pastille creuse. Une position posée sur le chemin réellement suivi vaut mieux qu’un relèvement d’antenne tenu pour mesuré.

Une vidéo sans position du tout relève, elle, de l’héritage depuis le lieu de la journée (D5).

**C2 | Détection des positions clonées.** Deux médias éloignés de plus de 20 minutes et distants de moins de **25 mètres** reçoivent l’anomalie `position_clonee`, **quelle que soit leur altitude**. Le signalement ne déclasse pas à lui seul : le déclassement vient de C1.

*Amendement du 30 août 2026.* La comparaison portait d’abord sur les coordonnées arrondies à la cinquième décimale, soit au mètre. C’était trop fin : deux appareils qui recopient le même relèvement d’antenne ne l’écrivent pas au même mètre. Autour de l’Eibsee, deux vidéos à trois mètres d’un groupe de seize photos y échappaient. Le regroupement se fait donc par voisinage à 25 mètres, et non par cellule d’une grille : deux points à un mètre l’un de l’autre mais de part et d’autre d’une frontière de cellule ne se seraient jamais rencontrés.

Ce découplage est ce qui rend la détection utile, et l’audit le confirme dans les deux sens. Les deux plus gros groupes du Tour des Alpes, 18 et 16 photos, portent tous une altitude nulle : C1 les attrape déjà. Mais un troisième groupe de 15 photos porte une altitude réelle et identique au millimètre, 2 331,958 mètres, ce qui ressemble à un gel de position plutôt qu’à un vrai relevé. À l’opposé, les groupes polynésiens montrent des altitudes qui varient de quelques dizaines de centimètres d’une photo à l’autre, signature d’un vrai bruit GPS.

`carnet stats` doit donc rapporter séparément le nombre de groupes dont le triplet latitude, longitude **et** altitude est strictement identique sur plus de 20 minutes. C’est la mesure qui dira s’il faut, à terme, déclasser aussi ces positions.

**C3 | Les noms de fichiers mentent parfois.** Cinq fichiers portent un nom en `IMG_AAAAMMJJ_HHMMSS` avec tirets bas, différent de la convention `IMGAAAAMMJJHHMMSS` du téléphone : `IMG_20260727_183401.jpg`, `IMG_20260730_071148.jpg`, `IMG_20260730_145328.jpg`, `IMG_20260802_225904.jpg`, `IMG_20260808_114119.jpg`. Ce sont des photos repartagées : le nom porte la date du partage, l’EXIF interne porte la vraie date. Deux cas vérifiés : `IMG_20260730_071148.jpg` a été prise le 28 juillet à 12 h 36, `IMG_20260802_225904.jpg` a été prise le 31 juillet à 10 h 28. **Toujours dater par l’EXIF**, et poser l’anomalie `nom_menteur`.

**C4 | Les 14 fichiers GoPro sont inexploitables tels quels.** HERO7 Black, tous en `.JPG`, aucune vidéo GoPro dans le dossier. Aucune donnée GPS, et horloge perdue : tous horodatés au 3 janvier 2016. Ce sont vraisemblablement les images d’eau, canyoning du Sušec compris. À replacer via la section `lots` d’`overrides.yaml`, avec `repartir_sur_segment`. Anomalie `horloge_perdue`. Leur ordre relatif reste exploitable : les horodatages, bien que faux, sont croissants, et les noms `GOPR2699` à `GOPR2717` sont séquentiels.

**C5 | Deux journées ont un trou de trace assumé.** Le 31 juillet au Königssee, aucune position ne descend au sud de 47,512 / 12,993, alors que la marche est bien allée jusqu’à l’Obersee et la Fischunkelalm : décrochage GPS en vallée encaissée. Le 6 août, la dernière position est à Tassenbach, à mi-parcours, alors que Lienz a bien été atteinte à vélo. Ces deux traces se complètent à la main via `segments`.

Le pipeline ne peut pas savoir qu’une marche est allée plus loin que la dernière photo : il ne détecte donc pas ces trous, il les **signale comme candidats**. Heuristique : tout couple de positions fiables consécutives d’une journée séparées de plus de 2 km et de plus de 45 minutes, ainsi que toute journée dont les positions fiables couvrent moins de la moitié de l’amplitude horaire de ses médias. L’humain tranche dans `overrides.yaml`.

**Le 6 août est signalé, le 31 juillet ne l’est pas, et ne peut pas l’être.** L’exécution du lot 1 a tranché la question. Ce jour-là, 57 positions fiables s’échelonnent de 8 h 32 à 12 h 51, toutes comprises entre 47,512 et 47,589 de latitude, sans le moindre saut de temps ni de distance. Les dix positions suspectes de la journée tombent, elles aussi, à l’intérieur de cette emprise. Autrement dit, la marche vers l’Obersee et la Fischunkelalm n’a laissé **aucune trace** dans les données : ni saut, ni gel de position détectable, ni couverture partielle.

Une heuristique qui signalerait quand même ce jour-là le ferait pour de mauvaises raisons. Le 31 juillet relève donc de la saisie manuelle dans `segments`, comme le prévoyait déjà cette contrainte, et non de la détection automatique.

**C6 | Trois familles de noms hostiles, pas une.** Le dossier contient :

- deux fichiers avec un tilde, `IMG20260808113008~2.jpg` et `IMG20260808121852~2.jpg`, caractère refusé par certains outils de transfert ;
- un fichier à suffixe numérique, `IMG20260807151640_01.jpg`, qui **coexiste avec `IMG20260807151640.jpg`** ;
- les cinq fichiers de C3, et les 14 GoPro de C4, qui ne suivent pas la convention du téléphone.

Les vidéos suivent une convention distincte, `VIDAAAAMMJJHHMMSS.mp4`, que la normalisation doit accepter au même titre que `IMG`. Les autres voyages en ajoutent deux : `IMG-AAAAMMJJ-WAnnnn.jpg` pour les fichiers reçus par messagerie (C10) et `Pnnnnnnn.JPG` pour l’appareil Panasonic de la Polynésie.

Règle : normalisation à l’ingestion (5.3), suffixes de variante conservés, anomalie `nom_normalise` posée sur tout fichier dont l’identifiant diffère du nom d’origine.

**C7 | Volumétrie.** Cible d’hébergement : rester dans les 10 Go gratuits de R2, sélection comprise (D7). Dossier source : 835 fichiers au total, 8,6 Go dont 5,6 Go de vidéo : 707 fichiers `.jpg` et 128 fichiers `.mp4`. Sur les 707 photos, 705 sont à la racine (dont 14 GoPro) et 2 dans le sous-dossier `[Originals]`, voir C8. Aucun fichier HEIF, MOV ou 3GP : pour ce voyage, `nom-exif` n’a besoin de couvrir que JPEG et MP4. Ne jamais servir les originaux depuis le site.

*Mesure du 30 août 2026.* L’estimation de 200 Mo de photos était quatre fois trop basse : les dérivés en JPEG pèsent **838 Mo**, dont 628 pour le seul 2048 px. En AVIF, repli JPEG compris, le total descendrait autour de 760 Mo, le repli reprenant une bonne part du gain. Les vidéos, elles, sont bien estimées : 77 minutes de rushes en 1080p, transcodées en 720p CRF 23, donnent 1,2 à 1,7 Go. Environ **2,2 Go au total**, ce qui tient dans les 10 Go de R2 mais laisse moins de marge qu’annoncé.

Le poids tient donc à la taille et non au format : le 2048 px pèse les trois quarts du total pour un seul usage, l’agrandissement au clic. Le retirer ramènerait les photos à 210 Mo. À arbitrer avant la mise en ligne.

**C8 | Un sous-dossier `[Originals]` produit des homonymes.** Le dossier source contient `[Originals]`, comportement classique de l’éditeur photo Android : la version retouchée reste à la racine, l’original y est déplacé. Il renferme deux fichiers, `IMG20260731092009.jpg` et `IMG20260808120006.jpg`, tous deux **homonymes exacts** de fichiers de la racine. Un parcours récursif naïf produit donc deux collisions d’identifiant.

Règle : `dossiers_ignores` dans `voyage.yaml` exclut `[Originals]` par défaut, et toute collision résiduelle fait échouer `carnet scan` (5.3). Anomalie `homonyme`.

Le cas n’est pas isolé : le dossier `2019 Polynésie` contient un sous-dossier `Sélection` de 10 fichiers, qui relève de la même règle.

**C9 | Les références d’hémisphère doivent être appliquées.** L’EXIF stocke la latitude et la longitude en valeur absolue, le signe étant porté par `GPSLatitudeRef` et `GPSLongitudeRef`. Les 56 positions du dossier `2019 Polynésie` portent les références `S` et `W`. Ignorer ces champs placerait Tahiti à 17,5° **nord** et 149,9° **est**, c’est-à-dire au milieu du Pacifique nord, à plus de 4 000 kilomètres de sa position réelle.

L’erreur est invisible sur les Alpes, la Tunisie et le Japon, tous en `N` et `E`, ce qui la rend d’autant plus dangereuse : elle ne se manifesterait qu’au lot 7, sur des données déjà publiées. `GPSAltitudeRef` obéit à la même logique, la valeur 1 signifiant sous le niveau de la mer, et se rencontre deux fois en Polynésie.

Quelques fichiers portent un bloc GPS sans référence lisible. Règle : supposer `N` et `E`, et poser l’anomalie `hemisphere_absent`.

**C10 | Les fichiers reçus par messagerie n’ont aucun EXIF.** Entre 9 et 14 % des photos des dossiers Tunisie et Polynésie sont nommées `IMG-AAAAMMJJ-WAnnnn.jpg` et ne contiennent **aucun bloc EXIF** : ni date, ni position, ni modèle d’appareil. Ce sont des photos reçues par messagerie, dont l’application a effacé toutes les métadonnées à l’envoi.

Pour ces fichiers, et pour eux seuls, le nom est la seule source de date disponible. La date qu’il porte est celle de la réception, généralement proche de la prise de vue mais pas identique. Règle : datation par le nom, anomalie `date_du_nom` et `exif_absent`, position `absente` donc héritée du lieu de la journée. **Jamais de repli sur la date de modification du fichier**, qui ne survit ni aux copies ni aux sauvegardes.

Cette règle ne contredit pas C3. C3 interdit de préférer le nom à un EXIF existant. C10 traite le cas où il n’y a pas d’EXIF du tout.

---

## 9. Le site

### 9.1 Routes

| Route | Contenu |
|---|---|
| `/` | Accueil, liste des voyages, carte du monde avec les zones parcourues |
| `/voyages/2026-alpes/` | Vue d’ensemble du voyage : carte complète, frise des 23 jours, chiffres clés |
| `/voyages/2026-alpes/carte/` | Carte plein écran avec lecteur de progression jour par jour |
| `/voyages/2026-alpes/jours/2026-08-14/` | Une journée : récit, galerie, mini-carte de la journée |
| `/voyages/2026-alpes/photos/` | Toutes les photos du voyage, groupées par jour |

### 9.2 Comportement de la carte

La carte est le cœur du site. Trois modes.

**Vue d’ensemble.** La trace complète du voyage, colorée par mode de déplacement. Les lieux de type `camp` en marqueurs distincts. Les journées cliquables. Zoom initial calé sur l’emprise du voyage.

**Lecture jour par jour.** Un curseur ou des flèches font défiler les journées. La trace du jour actif est mise en avant, les autres passent en gris clair. La carte recentre en douceur sur l’emprise du jour. Les photos du jour apparaissent en pastilles cliquables le long de la trace.

**Journée.** Sur la page d’une journée, une mini-carte non plein écran montre la seule trace du jour, avec les points photo. Cliquer un point fait défiler la page jusqu’à la photo correspondante, et inversement.

**Un seul style de trait.** La couleur dit le mode de déplacement, et rien d’autre. L’origine du tracé, relevé par les photos ou reconstitué par un moteur d’itinéraire, n’est plus rendue.

La version 1.2 prévoyait quatre styles de trait, sous le titre « Honnêteté du tracé », pour que le lecteur distingue le mesuré du déduit. La distinction a été mise en œuvre puis retirée à l’usage : elle chargeait la carte d’une information qui intéresse celui qui construit le site, non celui qui lit le récit. La propriété `source` reste dans `trace.geojson`, où elle documente la provenance ; le rendu ne s’en sert plus.

Les traits sont épais et bordés d’un halo clair. Le fond Positron est fait de traits gris fins, frontières et cours d’eau, dans lesquels une ligne colorée se perd sans ce dégagement.

Même principe pour les points de médias : pastille pleine pour une position `haute`, pastille creuse pour une position reconstituée, c’est-à-dire `posee`, `interpolee` ou `heritee`. Les positions `basse` qu’aucun mécanisme n’a su replacer ne sont pas affichées — ce serait poser un point au hasard — et les positions `absente` non plus.

La photo et la vidéo ne partagent pas la même teinte : la vidéo n’a pas de vignette à montrer au survol, et sans cette distinction elle passait pour une photo dont la vignette manquait.

### 9.3 Responsive

**Ordinateur, à partir de 1024 pixels.** Deux colonnes. Carte fixe à droite sur 45 % de la largeur, récit défilant à gauche. Le défilement du récit pilote la carte.

Le site ayant une page par journée, et non une seule page qui déroule le voyage, le pilotage joue à l’échelle de la photo et non de la journée : la photo qui occupe le centre de l’écran s’entoure d’un anneau sur la carte, et cliquer une pastille fait défiler le récit jusqu’à la photo. Le seuil est écrit en `rem` plutôt qu’en pixels : 64rem valent bien 1024 pixels, et suivent le lecteur qui grossit sa police.

**Téléphone, en dessous de 768 pixels.** Une colonne. La carte devient un bandeau réduit et collant en haut de l’écran, sur environ 30 % de la hauteur, dépliable en plein écran par un bouton. Le récit occupe le reste. Le pilotage par défilement reste actif sur le bandeau.

**Tablette, de 768 à 1024 pixels.** Comportement téléphone avec un bandeau plus haut.

### 9.4 Performance

Budgets exprimés en **poids transféré, compression brotli comprise, hors tuiles de fond de carte** :

- moins de 50 Ko de JavaScript sur les pages sans carte ;
- moins de 320 Ko sur les pages avec carte, MapLibre pesant à lui seul environ 310 Ko.

*Amendement du 30 août 2026, après mesure.* Le plafond était de 300 Ko, sur l’hypothèse d’un MapLibre à 230 Ko. La mesure donne 201 Ko pour la bibliothèque et **108 Ko de plus pour son travailleur**, que MapLibre 6 sert dans un second fichier : le décodage des tuiles vectorielles ne se fait plus sur le fil principal. Le total atteint 312 Ko, dont 3 Ko sont à nous. Le dépassement ne vient donc pas du site mais d’un changement de version de la bibliothèque, et il n’y a rien à y retrancher : le plafond monte à 320 Ko plutôt que d’être affiché comme tenu alors qu’il ne l’est pas.

**La trace se sert découpée.** Le fichier complet pèse 133 Ko en brotli, ce qui est le prix de la vue d’ensemble et du lecteur jour par jour, qui montrent tout le voyage. Une page de journée, elle, ne charge que la sienne, servie par un point de sortie séparé : **2 Ko en médiane** au lieu de 133. Le découpage se fait à la construction du site et non dans le pipeline, `data/` gardant un seul fichier qui reste la source.

**Plein écran et zoom.** La carte porte deux contrôles, le zoom et le plein écran, tous deux fournis par MapLibre. Une carte de 3 500 km lue dans un bandeau de téléphone a besoin de pouvoir s’étendre.

**Légende.** Sous la carte, les modes présents dans le voyage, leur couleur et leurs kilomètres, calculés depuis la trace et non répétés à la main, puis les deux marqueurs, le camping et la photo.

**Survol.** Passer le curseur sur une trace affiche le titre de la journée ; sur une pastille, son titre et la vignette de la photo. L’étiquette s’affiche en bas à gauche et non en bulle : une bulle masquerait le tracé que l’on est en train de suivre. C’est pour cela que `trace.geojson` porte, sur chaque point, le chemin de la vignette : le site ne le reconstruit pas par convention, il le lit, comme toute autre métadonnée (section 5.2).

**Agrandissement des photos.** Un clic sur une photo l’ouvre en plein écran, avec les flèches du clavier, l’échappement et le balayage au doigt pour passer de l’une à l’autre. C’est le seul JavaScript des pages sans carte, et il tient en moins d’un kilooctet : le `<dialog>` natif fournit gratuitement le fond assombri, la fermeture par Échap et le piège de focus, ce qui laisse au script la seule navigation.

Le composant ne connaît aucune photo. Il ramasse au chargement tout ce qui porte un attribut `data-grand`, dans l’ordre du document, de sorte qu’une galerie ajoutée plus tard fonctionne sans qu’il change.

MapLibre n’est chargé qu’à la demande, en `client:visible`. Les images sont servies en AVIF avec `srcset`, en `loading="lazy"`, avec le LQIP en fond le temps du chargement. Les vidéos ne sont jamais préchargées : poster seul, chargement au clic.

### 9.5 Ton visuel

Sobre. Le sujet, ce sont les photos et le texte, pas l’interface. Typographie lisible, marges généreuses, pas d’animation gratuite. Mode sombre et mode clair, pilotés par la préférence système. Palette dérivée des paysages : neutres chauds, un accent terre cuite pour les liens.

**La carte, elle, ne suit pas cette palette.** Six modes de déplacement rabattus vers le beige ne se distinguaient plus les uns des autres : le brun de la route se confondait avec le violet du train et avec le bleu du bateau, sur un fond Positron déjà gris. Les teintes des traçés sont donc franches et écartées sur la roue chromatique, la sobriété restant à la page qui entoure la carte. La route, qui pèse les trois quarts des kilomètres, prend le brun le plus sombre et sert de fond aux autres.

---

## 10. Déploiement

Dépôt Git unique. Push sur `main` déclenche le build Astro sur Cloudflare Pages. Le pipeline `carnet` ne tourne pas en CI : il est exécuté à la main sur le poste, et ses sorties `data/` sont commitées. Les dérivés `media/` sont poussés vers R2 avec `rclone`.

Motif : le dossier source de 8,6 Go vit sur le disque `P:` du poste et ne monte pas en CI.

**Fond de carte.** Les fichiers PMTiles sont hébergés sur R2, jamais dans le dépôt : Cloudflare Pages plafonne à 25 Mo par fichier. Deux catégories de fichiers :

- un fichier planétaire basse résolution, niveaux 0 à 6, quelques dizaines de mégaoctets, pour la carte du monde de l’accueil ;
- un extrait régional par voyage publié, construit au moment de publier ce voyage et pas avant.

Cette découpe est ce qui permet d’accueillir le Japon, la Polynésie, la Tunisie et l’Asie sans jamais héberger la planète entière. R2 doit exposer les en-têtes CORS nécessaires et accepter les requêtes HTTP Range.

Le site est public mais non indexé : `robots.txt` en `Disallow: /`, pas de sitemap soumis. Le partage se fait par lien.

---

## 11. Feuille de route

Sept lots, chacun livrable et testable seul. Ne pas commencer un lot avant que le précédent tourne.

**Lot 1 | Inventaire.** `carnet scan` et `carnet stats`. Lecture EXIF photos et vidéos, normalisation des noms et identifiants, scoring de fiabilité, anomalies. Sortie : un rapport lisible en console et un `media.json` sans dérivés. Tourne sur le poste actuel.

Critère de fin, contrainte par contrainte :

| Contrainte | Attendu | Repère chiffré sur le Tour des Alpes |
|---|---|---|
| C1 | Positions à altitude nulle déclassées en `basse` | 223 photos sur 691, plus les 128 vidéos |
| C2 | Positions clonées détectées, listées, non déclassées à ce titre | 24 groupes, 186 médias, dont 4 à altitude réelle identique |
| C3 | Les cinq fichiers repartagés datés par l’EXIF, anomalie posée | 5 fichiers |
| C4 | Les GoPro identifiés, anomalie `horloge_perdue` | 14 fichiers, tous au 3 janvier 2016 |
| C5 | Trous candidats signalés par l’heuristique | 26 candidats, dont le 6 août. Pas le 31 juillet, voir C5 |
| C6 | Noms normalisés, suffixes de variante préservés, anomalie posée | 22 fichiers hors convention |
| C7 | Volumétrie rapportée par `carnet stats` | 835 fichiers, 8,6 Go |
| C8 | `[Originals]` ignoré, toute collision résiduelle fait échouer la commande | 2 homonymes |
| C9 | Références d’hémisphère appliquées | 0 cas ici, à vérifier sur la fixture polynésienne |
| C10 | Fichiers sans EXIF datés par leur nom, anomalies posées | 0 cas ici, à vérifier sur la fixture tunisienne |

Les contraintes C9 et C10 ne se manifestent pas sur le Tour des Alpes. Elles sont néanmoins traitées dès le lot 1, avec des fixtures issues des dossiers Polynésie et Tunisie : ce sont précisément les pièges qui, découverts au lot 7, obligeraient à republier des données déjà en ligne.

**Lot 2 | Surcharges, lieux et traces.** `overrides.yaml`, héritage depuis les lieux, interpolation, appels au moteur d’itinéraire et cache, construction des `LineString` par journée et par mode, injection des segments manuels. Tourne sur le poste actuel.

Critère de fin : `trace.geojson` visuellement correct dans [geojson.io](https://geojson.io), chaque journée portant soit une trace continue, soit un trou explicitement documenté dans `overrides.yaml`. Aucun tronçon `marche` calculé sur le réseau routier.

**Lot 3 | Dérivés.** Génération des trois tailles d’images, repli JPEG, LQIP, posters vidéo, **pour les seuls médias retenus** (D7). Le transcodage vidéo reste en pause (D8) : le drapeau `--videos` existe, il attend la sélection. **À exécuter sur la machine cible**, celle qui dispose d’AVX2.

Critère de fin : `carnet build` idempotent, un second passage ne produisant aucun travail, et temps du premier passage mesuré et consigné dans le README. Objectif indicatif : moins de 3 minutes pour les 705 photos.

**Import du carnet, avant le lot 4.** L’import des 23 journées depuis Notion, initialement placé au lot 6, est avancé avant le site. Motif : construire des pages sans texte revient à dessiner des gabarits à l’aveugle, et le critère de fin du lot 4, « le voyage est lisible de bout en bout », serait invérifiable sur un site vide.

L’import reste **unique**. Après conversion, le Markdown de `content/` est la seule source de vérité et la page Notion est conservée comme archive figée. Les champs chiffrés du frontmatter absents de Notion restent vides plutôt qu’inventés.

**Lot 4 | Site, sans carte.** Astro, routes, rendu des journées depuis le Markdown, galeries, mode sombre, responsive. Critère de fin : le voyage est lisible de bout en bout sur téléphone.

**Lot 5 | Carte.** MapLibre, les trois modes de la section 9.2, la mise en page à deux colonnes de la section 9.3, la synchronisation défilement et carte. Critère de fin : le lecteur jour par jour fonctionne au doigt sur téléphone. PMTiles est passé au lot 6, avec le reste de la mise en ligne, et les quatre styles de trace ont été retirés (9.2).

**Lot 6 | Mise en ligne.** Placement des photos dans le récit, déploiement, R2, extrait PMTiles des Alpes. L’import du carnet, lui, a été avancé avant le lot 4.

La relecture et l’amélioration des textes se font dans le dépôt, au fil de l’eau, et n’attendent aucun lot.

**Lot 7 | Voyages antérieurs.** L’audit du 27 août 2026 a levé l’incertitude qui pesait sur ce lot, et l’a coupé en deux.

*Japon 2024 et Tunisie 2025* relèvent du **régime riche** (section 1) : même téléphone que le Tour des Alpes, date et position quasi systématiques, mêmes conventions de nommage. Aucun traitement particulier n’est requis, ce sont deux exécutions de plus du pipeline existant. Seule nouveauté : les fichiers de messagerie de C10, présents en Tunisie.

*Polynésie 2019* relève du **régime pauvre** : 4 % de positions seulement, deux appareils distincts (Nokia 7 plus et Panasonic DMC-LX7), et surtout **aucun `OffsetTimeOriginal`**. La position vient donc des `lieux` déclarés par journée (D5), et les traces sont de source `heritee` ou `manuelle`.

Trois points à instruire avant de s’engager sur la Polynésie, aucun n’étant bloquant pour les lots 1 à 6 :

1. **Le fuseau des horloges.** Sans offset dans l’EXIF, rien ne dit si les appareils étaient à l’heure de Papeete ou restés à l’heure de Paris, soit 12 heures d’écart et un décalage d’une journée entière sur le rattachement. Les 56 photos géolocalisées permettront de trancher, en confrontant leur heure à leur position.
2. **L’amplitude du dossier.** Les dates s’étalent du 29 août au 27 novembre 2019, soit trois mois. À confirmer : un seul long voyage, ou un dossier qui contient aussi l’avant et l’après.
3. **Les deux appareils.** Leurs horloges peuvent diverger entre elles. À mesurer avant de fusionner leurs médias dans une même chronologie.

---

## 12. Conventions pour Claude Code

- **Langue.** Le contenu, les commentaires et les messages de commit sont en français. Les identifiants de code sont en anglais.
- **Typographie française.** Apostrophes typographiques, guillemets français, accents corrects partout, y compris dans le code qui génère du texte. Jamais de tiret cadratin.
- **Rust.** `cargo clippy -- -D warnings` doit passer. `anyhow` dans le binaire, `thiserror` dans les modules. Pas de `unwrap` hors tests.
- **Tests.** Chacune des dix contraintes de la section 8 a un test avec un fichier d’exemple dans `carnet/tests/fixtures/`.
- **Pas de dépendance ajoutée sans motif écrit** dans le message de commit.
- **Le contenu de `data/` et `media/` ne se modifie jamais à la main.** Si une donnée est fausse, la corriger dans `overrides.yaml` et relancer `carnet build`.
- **`data/` ne contient jamais de contenu rédactionnel.** Uniquement des faits mesurés ou calculés. Le texte a une seule source, `content/`.
- **Aucun arbitrage silencieux sur les données.** Une ambiguïté (collision d’identifiant, lieu inconnu, surcharge inapplicable) fait échouer la commande avec un message explicite, plutôt que de choisir à la place de l’humain.

---

## 13. Sources

- [FindPenguins](https://findpenguins.com/), modèle fonctionnel de référence
- [MapLibre GL JS](https://github.com/maplibre/maplibre-gl-js) | [PMTiles](https://github.com/protomaps/PMTiles) | [protomaps/basemaps](https://github.com/protomaps/basemaps) | [OpenFreeMap](https://openfreemap.org) | [tilemaker](https://github.com/systemed/tilemaker)
- [OpenRouteService](https://openrouteservice.org), moteur d’itinéraire | [OSRM](https://project-osrm.org), repli auto-hébergeable
- [nom-exif](https://github.com/mindeng/nom-exif) | [image](https://github.com/image-rs/image) | [fast_image_resize](https://github.com/Cykooz/fast_image_resize) | [geojson](https://github.com/georust/geojson) | [serde_norway](https://crates.io/crates/serde_norway)
- [AdventureLog](https://github.com/seanmorley15/AdventureLog), à lire pour son modèle de données, repris en D5
- [Photofield](https://github.com/SmilyOrg/photofield), à lire pour son reverse geocoding local
- [Astro](https://astro.build) | [Cloudflare Pages](https://pages.cloudflare.com) | [Cloudflare R2](https://developers.cloudflare.com/r2/) | [rclone](https://rclone.org)
- Carnet du voyage 2026 : page Notion « Tour des Alpes orientales et Slovénie », id `371ec81a-ae12-810f-a2e9-dfbe7adbcff8`

---

## Annexe B | Ce qui a changé en version 1.2

Révision faite le 27 août 2026 après un audit EXIF des quatre dossiers sources, soit 4 134 photos, mené pour répondre à la question laissée ouverte par la version 1.1 : les voyages antérieurs ont-ils une date exploitable ?

La réponse est oui partout, et l’audit a rapporté quatre constats qui n’étaient pas cherchés.

- **La règle centrale de C1 était inopérante.** « Position sans altitude = suspecte » ne pouvait rien déclasser, le champ `GPSAltitude` n’étant jamais absent, sur aucune des 4 134 photos. Le vrai marqueur est l’altitude **exactement nulle**, qui concerne 32 % des positions du Tour des Alpes. Le taux annoncé en version 1.0 était donc juste, mais la règle qui l’encodait était fausse.
- **Les références d’hémisphère ne sont pas appliquées** par la version 1.1, ce qui aurait placé la Polynésie dans le Pacifique nord. Contrainte C9 ajoutée. Le piège est invisible sur les trois voyages européens et asiatiques, et ne se serait manifesté qu’au lot 7.
- **Les fichiers reçus par messagerie n’ont aucun EXIF**, ni date ni position, et représentent 9 à 14 % de certains dossiers. Contrainte C10 ajoutée, avec une règle de datation par le nom qui ne contredit pas C3.
- **Le partage des voyages n’est pas chronologique mais matériel.** Japon 2024 et Tunisie 2025, pris avec le même téléphone que les Alpes, relèvent du régime riche et ne demandent aucun traitement spécial. Seule la Polynésie 2019 relève du régime pauvre. Le lot 7 et la section 1 sont réécrits en conséquence.

Deux points nouveaux restent en suspens, tous deux sans effet sur les lots 1 à 6 : le fuseau des horloges polynésiennes, faute d’`OffsetTimeOriginal`, et le sort des positions clonées dont l’altitude est réelle et strictement identique, que `carnet stats` doit désormais compter séparément.

### Ce que la première exécution a corrigé

Le lot 1 a tourné sur les 833 médias du dossier source. Trois constats ont été reversés dans ce document.

- **La règle C1 ne vaut que pour les photos.** Les 128 vidéos portent une position sans altitude, le MP4 omettant la composante verticale. La règle les déclasse toutes. Trois options sont posées en C1, à trancher au lot 2.
- **Le trou du 31 juillet n’est pas détectable.** Les données ne portent aucune trace de la marche vers l’Obersee : ni saut, ni gel de position, ni couverture partielle. Le critère de fin du lot 1 est corrigé en conséquence, plutôt que d’inventer une heuristique qui viserait juste par hasard.
- **Les repères chiffrés de C2 étaient faux.** Les 137 groupes annoncés comptaient toutes les coordonnées répétées, y compris deux photos prises coup sur coup. Avec le seuil de vingt minutes qui définit la contrainte, il y a 24 groupes et 186 médias.

---

## Annexe A | Ce qui a changé en version 1.1

Révision faite le 27 août 2026, après relecture critique de la version 1.0, inspection du dossier source et audit du poste de travail. Aucune ligne de code n’avait encore été écrite.

### Décisions d’architecture ajoutées

- **D5, le lieu porte la position.** Motivé par la découverte que les voyages antérieurs n’ont aucune coordonnée GPS. Bénéfice collatéral sur le voyage 2026 : les GoPro et les positions absentes ne disparaissent plus de la carte.
- **D6, les traces routières sont calculées.** Motivé par l’absence de tout enregistrement GPS continu. La version 1.0 supposait implicitement qu’une trace se déduisait des photos, ce qui aurait produit des droites à travers les massifs.

### Corrections d’erreurs factuelles

- La volumétrie de C7 était fausse : 835 fichiers et non 834, 707 photos et non 705. Le compte de la version 1.0 incluait le dossier `[Originals]` comme s’il s’agissait d’un fichier.
- C6 annonçait un fichier au nom hostile. Il y en a trois familles, dont deux fichiers à tilde et un cas de coexistence entre une photo et sa variante `_01`.
- C4 laissait entendre que des vidéos GoPro existaient. Il n’y en a aucune, les 14 fichiers sont des `.JPG`.
- Le dossier ne contient ni HEIF, ni MOV, ni 3GP : uniquement du JPEG et du MP4.
- `serde_yaml` 0.9 est un dépôt archivé, remplacé par `serde_norway`.

### Contradictions levées

- **C2 était redondante avec C1.** La détection de clones s’applique désormais indépendamment de l’altitude, et le champ `anomalies` sépare le motif du verdict.
- **`origine_position: interpolee` n’était produit par aucune étape.** L’interpolation est spécifiée, sous garde-fous, et une quatrième valeur `heritee` est ajoutée.
- **Le critère de fin du lot 1 était intenable** pour C5, C6 et C7, qui ne sont pas des détections. Il est reformulé contrainte par contrainte.
- **`carnet push` figurait en section 10 mais pas parmi les sous-commandes.** Il est supprimé au profit de `rclone`.
- **`jours.json` n’avait aucun contenu défini.** Il devient un index strictement dérivé, sans texte, et la règle générale est inscrite en section 4.
- **`basemap.pmtiles` était placé dans `site/public/`**, ce qui est indéployable sur Cloudflare Pages. Les PMTiles passent sur R2, avec une découpe planète basse résolution plus extrait par voyage.
- **L’identifiant de média n’était pas spécifié**, et la règle naturelle produisait deux collisions certaines. La règle est écrite, l’unicité vérifiée, et l’échec est bruyant.
- **Les camps étaient cités par la carte sans exister dans le modèle.** Ils deviennent des `lieux` de type `camp`. Camps et lieux ont été fusionnés en une seule collection plutôt que modélisés séparément : un camp est un lieu où l’on dort, et deux blocs distincts auraient dupliqué nom et position.
- **Le rattachement au jour local n’avait pas d’outil.** Un champ `fuseau` obligatoire remplace une résolution automatique dont la dépendance n’était pas prévue.
- **Le budget JavaScript ne précisait pas s’il était brut ou transféré.** Il est désormais exprimé en poids transféré, et resserré, un budget indépassable n’en étant pas un.
- **L’idempotence reposait sur taille et mtime seuls**, donc ignorait tout changement de réglage d’encodage. Une empreinte des paramètres est ajoutée au cache.

### Contraintes ajoutées

- **C8**, le sous-dossier `[Originals]` et ses homonymes.

### Lots

- Le budget « moins de 3 minutes » du lot 3 est rattaché à la machine cible, le poste actuel étant dépourvu d’AVX2. Les lots 1 et 2, qui ne produisent aucune image, restent sur le poste actuel.
- **Lot 7 ajouté**, voyages antérieurs, plutôt que d’élargir le lot 4. Le modèle qu’il exige, D5, est en revanche décidé dès maintenant : le repeindre après le lot 5 aurait coûté cher.
