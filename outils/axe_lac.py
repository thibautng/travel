# -*- coding: utf-8 -*-
"""Axe median d'un plan d'eau OpenStreetMap, pour tracer une navigation.

Le moteur d'itineraire ne connait que la terre : aucun profil ne route un
bateau. Les traversees sont donc ecrites a la main dans overrides.yaml, et
tant qu'elles l'etaient a l'oeil, elles coupaient les rives. Le Konigssee,
place de memoire, sortait du lac par le sud et traversait la montagne.

Cet outil va chercher le polygone du lac dans OpenStreetMap, puis calcule
pour une serie de latitudes la longitude mediane de l'eau : sur un lac
allonge nord-sud, c'est une bonne approximation du chenal. La sortie se colle
telle quelle dans la section `segments`.

    python outils/axe_lac.py "Konigssee" "47.48,12.94,47.61,13.02" 18

Le resultat est verifie une fois, puis fige dans overrides.yaml : le site ne
depend d'aucun service au moment du build.
"""
import json, sys, urllib.request, urllib.parse

REQUETE = """
[out:json][timeout:60];
(
  way["natural"="water"]["name"="%s"](%s);
  relation["natural"="water"]["name"="%s"](%s);
);
out geom;
"""


def interroger(nom, bbox):
    corps = REQUETE % (nom, bbox, nom, bbox)
    donnees = urllib.parse.urlencode({"data": corps}).encode()
    requete = urllib.request.Request(
        "https://overpass-api.de/api/interpreter",
        data=donnees,
        headers={"User-Agent": "carnet-voyages/0.1"},
    )
    with urllib.request.urlopen(requete, timeout=120) as reponse:
        return json.load(reponse)


def anneaux(reponse):
    """Toutes les suites de points fermees ou non, en [lon, lat]."""
    sorties = []
    for element in reponse.get("elements", []):
        if element.get("type") == "way" and "geometry" in element:
            sorties.append([[p["lon"], p["lat"]] for p in element["geometry"]])
        elif element.get("type") == "relation":
            for membre in element.get("members", []):
                if membre.get("role") == "outer" and "geometry" in membre:
                    sorties.append([[p["lon"], p["lat"]] for p in membre["geometry"]])
    return sorties


def dans_le_polygone(x, y, anneau):
    dedans = False
    n = len(anneau)
    for i in range(n):
        x1, y1 = anneau[i]
        x2, y2 = anneau[(i + 1) % n]
        if (y1 > y) != (y2 > y):
            xc = x1 + (y - y1) * (x2 - x1) / (y2 - y1)
            if x < xc:
                dedans = not dedans
    return dedans


def axe(anneau, pas):
    lats = [p[1] for p in anneau]
    lons = [p[0] for p in anneau]
    points = []
    sud, nord = min(lats), max(lats)
    for i in range(pas + 1):
        y = nord - (nord - sud) * i / pas
        # Intersections de la latitude y avec le contour.
        xs = []
        n = len(anneau)
        for k in range(n):
            x1, y1 = anneau[k]
            x2, y2 = anneau[(k + 1) % n]
            if (y1 > y) != (y2 > y):
                xs.append(x1 + (y - y1) * (x2 - x1) / (y2 - y1))
        if len(xs) < 2:
            continue
        xs.sort()
        # Le plus large intervalle d'eau a cette latitude.
        meilleur, largeur = None, -1
        for k in range(0, len(xs) - 1, 2):
            if xs[k + 1] - xs[k] > largeur:
                largeur = xs[k + 1] - xs[k]
                meilleur = (xs[k] + xs[k + 1]) / 2
        if meilleur is not None:
            points.append([round(meilleur, 5), round(y, 5)])
    return points, (min(lons), sud, max(lons), nord)


if __name__ == "__main__":
    nom, bbox, pas = sys.argv[1], sys.argv[2], int(sys.argv[3])
    reponse = interroger(nom, bbox)
    listes = anneaux(reponse)
    print("anneaux recuperes :", len(listes), "taille max :", max((len(a) for a in listes), default=0))
    if not listes:
        sys.exit("aucun polygone")
    principal = max(listes, key=len)
    points, boite = axe(principal, pas)
    print("emprise lon %.5f..%.5f  lat %.5f..%.5f" % (boite[0], boite[2], boite[1], boite[3]))
    print("axe de %d points :" % len(points))
    for p in points:
        print("      - [%.5f, %.5f]" % (p[0], p[1]))
