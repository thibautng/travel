# Projet | Site des voyages de la famille

## Lis ceci en premier

@SPEC.md

`SPEC.md` est la spécification de référence du projet. Elle fait autorité sur toute décision d'architecture, de modèle de données ou de comportement. **Ne commence aucune tâche sans t'y être référé.**

Si une demande contredit `SPEC.md`, ne l'exécute pas en silence : signale la contradiction, cite la section concernée, et demande si la spécification doit être mise à jour. Une décision qui change se répercute dans `SPEC.md` avant d'être codée.

Si `SPEC.md` ne dit rien sur un point, dis-le explicitement et propose une option, plutôt que de choisir en silence.

## Ce qui est déjà tranché

Ces points sont arrêtés dans `SPEC.md` section 2. Ne les rouvre pas, ne propose pas d'alternative, sauf si je le demande.

- Le pipeline médias est en **Rust** (binaire `carnet`). Le front n'est **pas** en Rust.
- Le site est **statique** (Astro). Pas de serveur, pas de base de données.
- La carte est **MapLibre GL JS** avec un fond **PMTiles**.
- `ffmpeg` est appelé en sous-processus pour la vidéo, pas de transcodage en Rust pur.

## Méthode de travail

Le projet avance par **lots**, décrits en section 11 de `SPEC.md`. Un seul lot à la fois.

Au début de chaque session :

1. Annonce le lot sur lequel on travaille et son critère de fin.
2. Propose un plan avant d'écrire du code. Attends ma validation.
3. Ne déborde jamais sur le lot suivant, même si c'est tentant.

À la fin d'une tâche, dis explicitement si le critère de fin du lot est atteint ou non, et ce qui manque.

## Règles de code

- `cargo clippy -- -D warnings` doit passer avant tout commit.
- `anyhow` dans le binaire, `thiserror` dans les modules. Pas de `unwrap` ni d'`expect` hors tests.
- Chaque contrainte de la section 8 de `SPEC.md` a un test avec un fichier d'exemple dans `carnet/tests/fixtures/`.
- Aucune dépendance ajoutée sans motif écrit dans le message de commit.
- Le contenu de `data/` et `media/` est généré. Ne le modifie jamais à la main : corrige `overrides.yaml` et relance `carnet build`.

## Langue et typographie

Contenu, commentaires et messages de commit en français. Identifiants de code en anglais.

Dans tout texte français, y compris les chaînes de caractères du code et les messages d'erreur :

- Apostrophes typographiques ( ’ ), jamais d'apostrophes droites.
- Guillemets français ( « » ), jamais de guillemets droits.
- Accents corrects partout : é, è, ê, ë, à, ù, ç, œ.
- **Jamais de tiret cadratin.** Utiliser un tiret simple, une virgule, une parenthèse ou deux-points.

## Communication

- Conclusion d'abord, détail ensuite.
- Pas de préambule, pas de « Parfait ! », pas de « Excellente question ! ».
- Pas de récapitulatif de ce que je viens de dire.
- Si quelque chose est ambigu, pose une question courte plutôt que de deviner.
- Signale les problèmes que tu vois, même si je ne les ai pas demandés. Ne les corrige pas sans me le dire.

## Données du premier voyage

Source des médias : `P:\Photos\Thibaut\2026 Tour des Alpes`, 835 fichiers, 8,6 Go, 707 photos et 128 vidéos. Deux de ces photos sont dans le sous-dossier `[Originals]` et portent le même nom que des fichiers de la racine.

Ce dossier **ne doit jamais être modifié**. Il est en lecture seule pour le pipeline.

Les dix contraintes de qualité des données connues sur ce dossier sont en section 8 de `SPEC.md`. Elles ne sont pas théoriques : elles ont été constatées sur les fichiers réels. Le critère de fin du lot 1 est détaillé contrainte par contrainte en section 11 : C1 à C4, C8, C9 et C10 sont détectées automatiquement, C5 est signalée comme trou candidat, C6 est appliquée par normalisation, C7 est rapportée par `carnet stats`.

## Machines

Le poste actuel est un i7-2600K sans AVX2. Il convient aux lots 1 et 2, qui ne produisent aucune image. Le lot 3, encodage des dérivés, attend la machine cible. Ne pas fixer de budget de temps d'encodage mesuré sur ce poste.
