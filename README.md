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

## État

Lots 1 et 2 terminés. Voir la feuille de route en section 11 de [SPEC.md](SPEC.md).

`carnet build 2026-alpes` produit `data/2026-alpes/` : 833 médias inventoriés,
23 journées, 121 tronçons de trace, 3 491 km. Les dérivés d’images arrivent au
lot 3, et le transcodage vidéo attend la sélection (D8).

La clé OpenRouteService se place dans la variable d’environnement
`CARNET_ORS_CLE`. Une fois `data/2026-alpes/itineraires.json` peuplé, elle
n’est plus nécessaire : le cache rend le build reproductible hors ligne.
