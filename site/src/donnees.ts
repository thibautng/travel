/**
 * Lecture des sorties du pipeline, `data/<voyage>/`.
 *
 * C'est le seul endroit du site qui touche au JSON brut. Tout le reste passe
 * par les types déclarés ici, de sorte qu'un changement de forme dans
 * `media.json` se voie à la compilation plutôt qu'à l'affichage.
 *
 * Voir SPEC.md, sections 5.3 à 5.6.
 */
import { readFileSync } from "node:fs";
import path from "node:path";

/**
 * Racine du dépôt.
 *
 * Déduite du dossier de travail et non de `import.meta.url` : une fois le
 * site compilé, le module vit dans `dist/pages/` et non plus dans `src/`,
 * et un chemin relatif à lui-même ne désigne plus le même endroit. Astro
 * s'exécute toujours depuis `site/`, en développement comme au build.
 */
const RACINE = process.env.VOYAGES_RACINE ?? path.resolve(process.cwd(), "..");

export type TypeMedia = "photo" | "video";
export type Fiabilite = "haute" | "basse" | "absente";
export type OriginePosition = "exif" | "override" | "heritee" | "interpolee";
export type Mode = "route" | "marche" | "velo" | "bateau" | "train" | "telepherique";
export type SourceTrace = "mesuree" | "calculee" | "manuelle" | "heritee";

export interface Position {
  lat: number;
  lon: number;
  alt?: number;
}

export interface Derives {
  vignette: string;
  moyen: string;
  grand: string;
  /** Filet de sécurité JPEG, en une seule taille. */
  repli: string;
}

export interface Media {
  id: string;
  type: TypeMedia;
  fichier_source: string;
  prise_le?: string;
  origine_date: "exif" | "nom" | "absente";
  jour?: string;
  position?: Position;
  fiabilite: Fiabilite;
  origine_position?: OriginePosition;
  lieu?: string;
  publie: boolean;
  derives?: Derives;
  /** Aperçu inline, chaîne `data:`. Voir SPEC.md, section 6.2, étape 10. */
  lqip?: string;
  anomalies: string[];
  largeur?: number;
  hauteur?: number;
  orientation?: number;
  appareil?: string;
}

export interface Comptes {
  photo: number;
  video: number;
  total: number;
}

export interface JourAgrege {
  jour: string;
  lieu?: string;
  camp?: string;
  premiere_prise?: string;
  derniere_prise?: string;
  medias: Comptes;
  couverture?: string;
  /** `[lon_min, lat_min, lon_max, lat_max]`. */
  bbox?: [number, number, number, number];
  distance_trace_km: number;
  modes: Mode[];
  anomalies: string[];
}

function lire<T>(voyage: string, fichier: string): T {
  const chemin = path.join(RACINE, "data", voyage, fichier);
  return JSON.parse(readFileSync(chemin, "utf-8")) as T;
}

/**
 * Les lectures sont mises en cache par voyage : Astro construit les pages
 * une à une, et `media.json` pèse plus d'un mégaoctet.
 */
const cache = new Map<string, unknown>();

function memoise<T>(cle: string, produire: () => T): T {
  if (!cache.has(cle)) {
    cache.set(cle, produire());
  }
  return cache.get(cle) as T;
}

export function medias(voyage: string): Media[] {
  return memoise(`medias:${voyage}`, () => lire<Media[]>(voyage, "media.json"));
}

export function jours(voyage: string): JourAgrege[] {
  return memoise(`jours:${voyage}`, () => lire<JourAgrege[]>(voyage, "jours.json"));
}

/**
 * Trace du voyage, telle quelle.
 *
 * Rendue en chaîne et non analysée : le site ne fait que la transmettre au
 * navigateur, qui la donnera à MapLibre. L'analyser ici coûterait une
 * sérialisation pour rien.
 */
export function traceBrute(voyage: string): string {
  return memoise(`trace:${voyage}`, () =>
    readFileSync(path.join(RACINE, "data", voyage, "trace.geojson"), "utf-8"),
  );
}

/** Index des médias par identifiant, pour résoudre les directives du récit. */
export function parIdentifiant(voyage: string): Map<string, Media> {
  return memoise(
    `index:${voyage}`,
    () => new Map(medias(voyage).map((m) => [m.id, m])),
  );
}

/** Médias d'une journée, dans l'ordre de prise de vue. */
export function mediasDuJour(voyage: string, jour: string): Media[] {
  return medias(voyage)
    .filter((m) => m.jour === jour && m.publie)
    .sort((a, b) => (a.prise_le ?? "").localeCompare(b.prise_le ?? ""));
}

/**
 * URL d'un dérivé.
 *
 * Les dérivés vivent dans `media/`, hors du dépôt, et sont poussés vers R2.
 * En développement, une jonction `site/public/media` pointe sur le dossier
 * local ; en production, `PUBLIC_MEDIA_URL` porte l'URL du seau.
 */
export function urlMedia(chemin: string, voyage: string): string {
  const base = import.meta.env.PUBLIC_MEDIA_URL ?? "/media";
  return `${base.replace(/\/$/, "")}/${voyage}/${chemin}`;
}
