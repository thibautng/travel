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

Le poids dépasse largement l’estimation de 200 Mo de la section 8 : le JPEG
en trois tailles coûte environ 1,2 Mo par photo. L’AVIF devrait ramener le
total autour de 400 à 450 Mo, ce qui reste très en deçà des 10 Go gratuits
de R2, mais l’estimation de la spec est à corriger une fois la mesure faite.

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

Mesuré sur le build du lot 4, avant la carte.

| Mesure | Valeur | Budget de la section 9.4 |
|---|---|---|
| JavaScript, pages sans carte | 986 octets, insérés dans la page | moins de 50 Ko transférés |
| Fichiers JavaScript servis à part | aucun | |
| Page médiane | 53 Ko de HTML | |
| Page la plus lourde | 271 Ko, celle des photos | |
| `dist` complet | 1,44 Mo, 27 fichiers | |

Les 986 octets sont ceux de la visionneuse, seul script des pages sans
carte. Les dérivés d’images ne sont pas dans `dist` : ils vont sur R2.

## État

Lots 1 et 2 terminés, lot 3 écrit et validé en JPEG. Voir la feuille de route en section 11 de [SPEC.md](SPEC.md).

`carnet build 2026-alpes` produit `data/2026-alpes/` et `media/2026-alpes/` :
833 médias inventoriés, 23 journées, 121 tronçons de trace, 3 491 km, et les
dérivés des 705 photos. Le transcodage vidéo attend la sélection (D8).

Options utiles : `--format avif|jpeg`, `--force` pour ignorer le cache,
`--sans-derives` pour ne produire que `data/`.

La clé OpenRouteService se place dans la variable d’environnement
`CARNET_ORS_CLE`. Une fois `data/2026-alpes/itineraires.json` peuplé, elle
n’est plus nécessaire : le cache rend le build reproductible hors ligne.
