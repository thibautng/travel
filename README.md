# Voyages

Journal de voyage familial : textes, photos, vidéos et carte de progression jour par jour.

La spécification de référence est [SPEC.md](SPEC.md). Elle fait autorité sur toute décision d’architecture, de modèle de données ou de comportement.

## Les deux moitiés du projet

| Quoi | Où | Langage |
|---|---|---|
| Pipeline médias | `carnet/` | Rust, binaire CLI |
| Site | `site/` | Astro |
| Contenu rédactionnel | `content/` | Markdown, écrit à la main |
| Données générées | `data/` | JSON, versionné |
| Dérivés générés | `media/` | non versionné, poussé vers R2 |

`content/` s’écrit à la main. `data/` et `media/` sont produits par `carnet` et ne se modifient jamais à la main : toute correction passe par `overrides.yaml`, puis `carnet build`.

## Le pipeline

```
carnet scan  <voyage>   inventaire du dossier source, lecture EXIF
carnet build <voyage>   inventaire + surcharges + dérivés + traces
carnet check <voyage>   contrôles de cohérence, sans rien écrire
carnet stats <voyage>   récapitulatif et anomalies
```

## Installation

Rust est installé avec la cible GNU, la machine ne disposant pas du linker MSVC :

```
rustup default stable-x86_64-pc-windows-gnu
```

Outils externes attendus, par lot :

| Outil | Nécessaire à partir du |
|---|---|
| `cargo` | lot 1 |
| `ffmpeg` dans le `PATH` | lot 3 |
| clé OpenRouteService, variable `CARNET_ORS_CLE` | lot 2 |
| `rclone` | lot 6 |
| `node` et `npm` | lot 4 |

## Mesures

Consignées ici comme le demande le critère de fin du lot 3.

| Passage | Disque | Format | Temps | Résultat |
|---|---|---|---|---|
| Inventaire seul (`carnet scan`) | pCloud | | 13 s | 833 médias |
| Premier build | pCloud | JPEG | 351 s d’encodage | 705 photos, 882 Mo |
| Second build | pCloud | JPEG | 0 s d’encodage, 18 s au total | 705 repris du cache |
| Premier build | NTFS local | JPEG | **217 s** d’encodage | 705 photos, 879 Mo |
| Second build | NTFS local | JPEG | 0 s d’encodage, **3 s** au total | 705 repris du cache |
| Premier build | machine cible | AVIF | à mesurer | |

Le passage de pCloud à un disque local fait tomber l’encodage de 351 à 217
secondes et le build à vide de 18 à 3. Un bon tiers de ce que je prenais pour
du temps d’encodage était de l’attente disque.

Calibrage de l’encodage, mesuré sur une photo de 4 624 x 2 608 pixels en
monofil (`cargo test --release calibrage -- --ignored --nocapture`) :

| Format | Trois tailles | Poids |
|---|---|---|
| JPEG | 0,35 s | 779 Ko |
| AVIF | 8,21 s | 527 Ko |

L’AVIF coûte donc vingt-trois fois plus cher sur ce processeur, dépourvu
d’AVX2, et rend 32 % de poids en moins. Un build complet en AVIF y prendrait
une vingtaine de minutes.

## Poids à héberger

Mesuré le 30 août 2026 sur le build en JPEG.

| Poste | Fichiers | Poids |
|---|---|---|
| Vignettes, 320 px | 705 | 21 Mo |
| Lecture, 1024 px | 705 | 189 Mo |
| Visionneuse, 2048 px | 705 | **628 Mo** |
| **Total des dérivés** | **2 115** | **838 Mo** |
| Le site lui-même (`dist`) | 50 | 4 Mo |

L’estimation de 200 Mo de la section 8 était donc quatre fois trop basse. En
AVIF, avec le repli JPEG en 1024 qu’impose la section 6.2, et au taux de 32 %
mesuré ci-dessus, le total descendrait autour de **760 Mo** seulement : le
repli reprend une bonne part du gain.

Le vrai levier n’est pas le format mais la taille. **Le 2048 px pèse à lui
seul les trois quarts du total**, pour un seul usage, l’agrandissement au
clic. Le retirer ramènerait le site à 210 Mo. C’est un arbitrage à rendre,
pas une évidence.

Les vidéos, si elles sont ajoutées : 128 fichiers, **77 minutes**, 5,4 Go en
source à 9,3 Mbit/s de moyenne, 95 en 1080p et 33 en 1920. Transcodées en
H.264 720p CRF 23 comme le prévoit la section 6.2, comptez **1,2 à 1,7 Go**
selon l’agitation des plans, plus une trentaine de mégaoctets d’images
d’affiche. Soit environ **2,2 Go** au total, contre 840 Mo sans vidéo.

Les deux tiennent dans les 10 Go gratuits de R2, dont l’argument décisif
n’est pas le stockage mais **l’absence de frais de sortie** : sur un site où
chaque visiteur télécharge des dizaines de mégaoctets, c’est la bande
passante qui déraperait ailleurs.

## Ne pas travailler sur un lecteur pCloud

Le dépôt a d’abord vécu sur `P:`, qui n’est pas un disque mais un montage
pCloud. Trois symptômes s’y sont manifestés, dans cet ordre : l’absence de
liens durs, dont le cache de compilation incrémentale de cargo a besoin, des
refus d’écriture intermittents, puis une **corruption silencieuse** de
`data/2026-alpes/itineraires.json`, un nombre remplacé par des espaces au
milieu d’un fichier de 5,9 Mo.

La preuve a été faite en écrivant le même contenu, extrait de git, sur les
deux volumes : valide sur `C:`, corrompu sur `P:`, empreintes différentes.
Les disques physiques de la machine sont sains et leur SMART ne prédit
aucune panne.

Le dépôt vit donc sur `D:`, en NTFS. Les photos sources, elles, restent sur
pCloud : elles se lisent de façon stable et le pipeline n’y écrit jamais.

## Budget du site

Mesuré le 30 août 2026, carte comprise, **en brotli** comme le veut la
section 9.4.

| Mesure | Valeur | Budget de la section 9.4 |
|---|---|---|
| JavaScript, pages sans carte | 986 octets, insérés dans la page | moins de 50 Ko |
| MapLibre | 201 Ko | |
| Travailleur de MapLibre | 108 Ko | |
| Notre script de carte | 3 Ko | |
| **JavaScript, pages avec carte** | **312 Ko** | moins de 320 Ko |
| Trace complète, vue d’ensemble et lecteur | 133 Ko | |
| Trace d’une journée | 2 Ko en médiane, 44 Ko au pire | |
| Page d’une journée, HTML | 11 Ko | |
| Vue d’ensemble, HTML | 4 Ko | |
| `dist` complet | 4,1 Mo, 50 fichiers | |

Les 986 octets sont ceux de la visionneuse, seul script des pages sans
carte. Les dérivés d’images ne sont pas dans `dist` : ils vont sur R2.

Le budget des pages à carte a été relevé de 300 à 320 Ko après cette
mesure : MapLibre 6 sert son travailleur dans un second fichier, que
l’hypothèse de la spec ignorait. Sur les 312 Ko, 3 sont à nous.

MapLibre n’est chargé que lorsque la carte approche de l’écran. Sur un
téléphone, où la carte est le bandeau du haut, cela veut dire tout de suite ;
sur un ordinateur, une journée lue sans jamais regarder la carte ne le charge
jamais.

## Mise en ligne

Rien n'est encore publié. Ce qui suit est la procédure, écrite d'avance, et
les commandes sont à vérifier au premier passage réel.

**Répartition.** Le site va sur Cloudflare Pages, les dérivés et les tuiles
sur R2. Motif en section 10 de la spec : Pages plafonne à 25 Mo par fichier
et n'est pas fait pour porter 838 Mo d'images, et R2 ne facture pas la sortie,
ce qui est le poste qui déraperait ailleurs sur un site d'images.

**Le site.** Un push sur `main` suffit une fois le dépôt connecté à Pages,
avec `site` comme racine, `npm run build` comme commande et `dist` comme
sortie. `carnet` ne tourne jamais en CI : le dossier source de 8,6 Go vit sur
le poste, et `data/` est commité.

**Les dérivés.** `rclone` fait le différentiel, la reprise et les sommes de
contrôle, ce qui vaut mieux qu'une sous-commande `push` à écrire :

```
rclone sync media/ r2:voyages-medias/ --progress --checksum --transfers 8
```

Le dépôt R2 doit exposer les en-têtes CORS et accepter les requêtes HTTP
Range, dont les PMTiles ont besoin. L'adresse publique du dépôt se donne au
site par `PUBLIC_MEDIA_URL`, que `urlMedia()` lit déjà et qui vaut `/media`
en développement.

**Le fond de carte.** Tant que `PUBLIC_FOND_PMTILES` est vide, la carte
utilise l'instance publique d'OpenFreeMap, comme le prévoit D4. Pour passer
aux PMTiles auto-hébergés, produire l'extrait des Alpes puis le poser sur R2 :

```
pmtiles extract https://build.protomaps.com/20260801.pmtiles alpes.pmtiles   --bbox=5.5,44.0,14.5,48.5 --maxzoom=14
rclone copy alpes.pmtiles r2:voyages-tuiles/
```

Puis déclarer `PUBLIC_FOND_PMTILES` dans les variables de Pages. Le protocole
`pmtiles://` et le style associé ne sont embarqués dans le site que si cette
variable est renseignée : sans elle, le morceau est éliminé à la
construction, et les 312 Ko de JavaScript mesurés plus haut ne bougent pas.

**En-têtes.** `site/public/_headers` porte le `X-Robots-Tag: noindex`, qui
double la balise du gabarit au niveau HTTP, et les durées de cache : un an
pour `/_astro/`, dont les noms portent une empreinte, une heure pour les
données, qui changent à chaque build du pipeline.

**Accès.** Le site est non indexé mais public : quiconque a le lien entre.
Pour un carnet qui montre des enfants, **Cloudflare Access** ajoute un mot de
passe ou une liste d'adresses autorisées, gratuitement jusqu'à cinquante
personnes, sur la même infrastructure. À trancher avant de diffuser le lien.

## État

Lots 1 à 5 terminés, en JPEG. Reste le lot 6 : mise en ligne, R2, extrait PMTiles. Voir la feuille de route en section 11 de [SPEC.md](SPEC.md).

`carnet build 2026-alpes` produit `data/2026-alpes/` et `media/2026-alpes/` :
833 médias inventoriés, 23 journées, 121 tronçons de trace, 3 491 km, et les
dérivés des 705 photos. Le transcodage vidéo attend la sélection (D8).

Options utiles : `--format avif|jpeg`, `--force` pour ignorer le cache,
`--sans-derives` pour ne produire que `data/`.

La clé OpenRouteService se place dans la variable d’environnement
`CARNET_ORS_CLE`. Une fois `data/2026-alpes/itineraires.json` peuplé, elle
n’est plus nécessaire : le cache rend le build reproductible hors ligne.
