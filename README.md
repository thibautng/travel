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

| Passage | Machine | Format | Temps | Résultat |
|---|---|---|---|---|
| Inventaire seul (`carnet scan`) | i7-2600K | | 13 s | 833 médias |
| Build sans dérivés | i7-2600K | | 16 s | `data/` complet |
| Premier build | i7-2600K | JPEG | 351 s d’encodage | 705 photos, 882 Mo |
| Second build | i7-2600K | JPEG | 0 s d’encodage, 18 s au total | 705 repris du cache |
| Premier build | machine cible | AVIF | à mesurer | |

Le poids dépasse largement l’estimation de 200 Mo de la section 8 : le JPEG
en trois tailles coûte environ 1,2 Mo par photo. L’AVIF devrait ramener le
total autour de 400 à 450 Mo, ce qui reste très en deçà des 10 Go gratuits
de R2, mais l’estimation de la spec est à corriger une fois la mesure faite.

## Un piquet sur ce poste

Le disque `P:` ne supporte pas les liens durs, dont le cache de compilation
incrémentale de cargo a besoin. Les builds finissent par échouer sur un
`stream did not contain valid UTF-8` qui n’a rien à voir avec le code.
Remède : `CARGO_INCREMENTAL=0`.

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
