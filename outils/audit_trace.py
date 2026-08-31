# -*- coding: utf-8 -*-
"""Relit la trace journee par journee et signale ce qui n'a pas l'air logique.

Ce que le pipeline ne peut pas juger seul : une droite de dix kilometres est
valide pour lui, absurde pour qui connait le voyage. Cet outil ne corrige
rien, il pose les questions.

    python outils/audit_trace.py 2026-alpes
"""
import json, io, math, sys, os

RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Une droite plus longue que cela entre deux points est un trajet non calcule.
DROITE_SUSPECTE_KM = 1.5
# Au-dela, une journee de marche merite verification.
MARCHE_LONGUE_KM = 20.0
# Distance a laquelle la trace doit approcher le camp du soir.
LOIN_DU_CAMP_KM = 1.5
# Ecart tolere entre la fin d'un troncon et le debut du suivant, dans l'ordre
# chronologique. Au-dela, la trace saute : on ne se teleporte pas.
SAUT_KM = 0.5


def km(a, b):
    p = math.radians((a[1] + b[1]) / 2)
    return math.hypot((b[0] - a[0]) * 111.32 * math.cos(p), (b[1] - a[1]) * 110.57)


def longueur(points):
    return sum(km(points[i - 1], points[i]) for i in range(1, len(points)))


def charger(voyage):
    trace = json.load(io.open(os.path.join(RACINE, "data", voyage, "trace.geojson"), encoding="utf-8"))
    jours = json.load(io.open(os.path.join(RACINE, "data", voyage, "jours.json"), encoding="utf-8"))
    return trace, jours if isinstance(jours, list) else jours.get("jours", [])


def main(voyage):
    trace, _ = charger(voyage)
    lignes, points = {}, {}
    for f in trace["features"]:
        jour = f["properties"].get("jour")
        if f["geometry"]["type"] == "LineString":
            lignes.setdefault(jour, []).append(f)
        else:
            points.setdefault(jour, []).append(f)

    total_alertes = 0
    for jour in sorted(lignes):
        troncons = lignes[jour]
        par_mode = {}
        alertes = []
        for f in troncons:
            p = f["properties"]
            c = f["geometry"]["coordinates"]
            d = longueur(c)
            par_mode[p["mode"]] = par_mode.get(p["mode"], 0) + d
            # Une droite est un tronçon a deux points : rien ne l'a calcule.
            if len(c) == 2 and d >= DROITE_SUSPECTE_KM:
                alertes.append("droite de %.1f km en %s" % (d, p["mode"]))
        # Continuite. L'ordre des Features n'est pas chronologique : le trajet
        # entre camps est ajoute en dernier, et un test sur les voisins
        # immediats criait au saut sur toutes les journees de deplacement.
        #
        # On regarde donc si les troncons du jour forment un seul morceau :
        # deux troncons se touchent quand une de leurs extremites en rejoint
        # une autre. Une journee en deux morceaux, c'est une teleportation.
        parent = list(range(len(troncons)))

        def racine(i):
            while parent[i] != i:
                parent[i] = parent[parent[i]]
                i = parent[i]
            return i

        bouts = []
        for f in troncons:
            c = f["geometry"]["coordinates"]
            bouts.append((c[0], c[-1]))
        for i in range(len(bouts)):
            for j in range(i + 1, len(bouts)):
                if any(km(a, b) < SAUT_KM for a in bouts[i] for b in bouts[j]):
                    ri, rj = racine(i), racine(j)
                    if ri != rj:
                        parent[ri] = rj
        morceaux = {}
        for i in range(len(troncons)):
            morceaux.setdefault(racine(i), []).append(i)
        if len(morceaux) > 1:
            details = []
            for indices in morceaux.values():
                modes = sorted({troncons[i]["properties"]["mode"] for i in indices})
                bout = bouts[indices[0]][0]
                details.append("%s pres de %.4f,%.4f" % ("/".join(modes), bout[1], bout[0]))
            alertes.append("trace en %d morceaux : %s" % (len(morceaux), " | ".join(details)))

        if par_mode.get("marche", 0) > MARCHE_LONGUE_KM:
            alertes.append("marche de %.1f km" % par_mode["marche"])
        for mode in ("velo", "bateau", "train", "telepherique"):
            if par_mode.get(mode, 0) > 0:
                alertes.append("%s : %.1f km" % (mode, par_mode[mode]))

        resume = ", ".join("%s %.1f" % (m, v) for m, v in sorted(par_mode.items(), key=lambda x: -x[1]))
        print("%s  %s" % (jour, resume))
        for a in alertes:
            print("    ! %s" % a)
        total_alertes += len(alertes)
    print()
    print("%d points d'attention" % total_alertes)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "2026-alpes")
