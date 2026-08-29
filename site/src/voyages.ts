/**
 * Lecture de `content/voyages/<id>/voyage.yaml`.
 *
 * Séparé de `donnees.ts` à dessein : celui-ci lit `data/`, produit par le
 * pipeline, celui-là lit `content/`, écrit à la main. La section 4 tient les
 * deux pour des sources distinctes, et le site ne doit pas les confondre.
 */
import { readFileSync, readdirSync, existsSync } from "node:fs";
import path from "node:path";
import { parse } from "yaml";

const RACINE = process.env.VOYAGES_RACINE ?? path.resolve(process.cwd(), "..");
const DOSSIER = path.join(RACINE, "content", "voyages");

export type TypeLieu = "camp" | "etape";

export interface Lieu {
  id: string;
  nom: string;
  type?: TypeLieu;
  position: { lat: number; lon: number; alt?: number };
  /** Jour d'arrivée, pour un camp. */
  du?: string;
  /** Jour de départ, exclu : arriver le 12 et repartir le 15, c'est y dormir trois nuits. */
  au?: string;
}

export interface Voyage {
  id: string;
  titre: string;
  sous_titre?: string;
  date_debut: string;
  date_fin: string;
  pays: string[];
  distance_km?: number;
  nuits?: number;
  mode?: string;
  fuseau: string;
  depart?: string;
  arrivee?: string;
  lieux: Lieu[];
}

const cache = new Map<string, Voyage>();

export function voyage(id: string): Voyage {
  const connu = cache.get(id);
  if (connu) return connu;
  const chemin = path.join(DOSSIER, id, "voyage.yaml");
  const lu = parse(readFileSync(chemin, "utf-8")) as Voyage;
  // `lieux` peut manquer sur un voyage encore en préparation.
  const complet: Voyage = { ...lu, lieux: lu.lieux ?? [] };
  cache.set(id, complet);
  return complet;
}

/** Tous les voyages déclarés, du plus récent au plus ancien. */
export function tousLesVoyages(): Voyage[] {
  return readdirSync(DOSSIER, { withFileTypes: true })
    .filter((e) => e.isDirectory() && existsSync(path.join(DOSSIER, e.name, "voyage.yaml")))
    .map((e) => voyage(e.name))
    .sort((a, b) => b.date_debut.localeCompare(a.date_debut));
}

/** Retrouve un lieu par son identifiant. */
export function lieu(v: Voyage, id: string | undefined): Lieu | undefined {
  if (!id) return undefined;
  return v.lieux.find((l) => l.id === id);
}

/** Les camps du voyage, dans l'ordre du séjour. */
export function camps(v: Voyage): Lieu[] {
  return v.lieux
    .filter((l) => l.type === "camp")
    .sort((a, b) => (a.du ?? "").localeCompare(b.du ?? ""));
}
