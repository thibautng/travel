# Questions ouvertes et notes de travail

Ce fichier recueille ce qui ne doit pas paraître sur le site : les questions
posées à la famille et les constats techniques tirés des photos. Il n'est ni
publié ni lu par le pipeline.

Les notes qui suivent venaient du carnet importé de Notion, où elles étaient
écrites en italique au fil des journées. Elles s'affichaient telles quelles
sur les pages publiques, ce qui n'était pas leur destination.

## Questions à la famille

**11 août, dix heures manquantes.** Départ de Fiè à 8 h 30, première photo au
lac de Garde à 18 h 26. Dix heures pour deux cents kilomètres de route.
Qu'avez-vous fait entre les deux ? Le récit du jour ne le dit pas, et la
carte trace une ligne d'un camp à l'autre faute de mieux.

**14 août, le bâtiment du plateau.** Le texte corrigé dit seulement ce que ce
n'était pas — le refuge Sogno di Berdzé. Était-ce le casotto du Parc, à
Bardoney ? Si oui, la phrase peut le nommer.

**Légendes des photos posées dans le récit.** Elles disent le moment et
l'heure, jamais le sujet : les images n'ont pas été regardées, et une légende
inventée vaudrait moins que pas de légende. À relire journée par journée.

## Constats techniques

**31 juillet, décrochage GPS au Königssee.** Aucune photo de la journée ne
descend au sud de 47,512 / 12,993, soit 1,5 km avant le débarcadère de Salet.
Le téléphone a perdu les satellites dans la vallée encaissée et a recopié sa
dernière position connue. Premier décrochage avéré du voyage. La trace du
fond du lac reste donc à corriger à la main dans `overrides.yaml`, la
navigation en bateau n'ayant aucun réseau routable à suivre.

**Deux photos verdies, le 29 juillet.** `IMG20260729180304` et
`IMG20260729190244` sortent avec une dominante verte des dérivés. Cause non
diagnostiquée ; à reprendre quand la question reviendra.

## Réglé

**24 juillet, nom du camping.** Campingplatz Traiermühle, à Bad
Peterstal-Griesbach, confirmé par la position des photos. Le texte le nomme.

**6 août, seconde moitié du Drauradweg.** La note demandait de tracer à la
main la partie du parcours qu'aucune photo ne documente. C'est fait :
`overrides.yaml` porte le segment de vélo jusqu'à Lienz et le train du
retour.
