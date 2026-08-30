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
 *
 * Le cache ne vaut qu'au build. En développement, `data/` vit hors des
 * sources surveillées par Vite : un `carnet build` ne provoque aucun
 * rechargement du module, et le serveur continuait de servir ce qu'il avait
 * lu à son démarrage. Une trace regeneree n'arrivait donc jamais à l'écran.
 */
const cache = new Map<string, unknown>();

function memoise<T>(cle: string, produire: () => T): T {
  if (!import.meta.env.PROD) return produire();
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

/**
 * Trace analysée, pour les découpages faits à la construction.
 *
 * Séparée de `traceBrute` à dessein : la version brute sert le fichier
 * complet sans le relire, celle-ci ne sert qu’à en extraire une journée.
 */
export function trace(voyage: string): { features?: any[] } {
  return memoise(`trace-lue:${voyage}`, () => JSON.parse(traceBrute(voyage)));
}

export interface EntreeLegende {
  cle: string;
  couleur: string;
  km: number;
}

export interface Legende {
  modes: EntreeLegende[];
  sources: string[];
}

/** Distance orthodromique en kilomètres, pour totaliser une polyligne. */
function distanceKm(a: number[], b: number[]): number {
  const R = 6371.0088;
  const rad = Math.PI / 180;
  const dphi = (b[1] - a[1]) * rad;
  const dlam = (b[0] - a[0]) * rad;
  const h =
    Math.sin(dphi / 2) ** 2 +
    Math.cos(a[1] * rad) * Math.cos(b[1] * rad) * Math.sin(dlam / 2) ** 2;
  return 2 * R * Math.asin(Math.sqrt(h));
}

/**
 * Modes et origines effectivement présents dans la trace, avec leurs
 * kilomètres. Rien n'est codé en dur : les couleurs viennent de la trace
 * elle-même, où le pipeline les a déjà déduites du mode. Une légende qui
 * répéterait la palette finirait par mentir.
 */
export function legende(voyage: string): Legende {
  return memoise(`legende:${voyage}`, () => {
    const trace = JSON.parse(traceBrute(voyage));
    const modes = new Map<string, EntreeLegende>();
    const sources = new Set<string>();

    for (const entite of trace.features ?? []) {
      if (entite.geometry?.type !== "LineString") continue;
      const p = entite.properties ?? {};
      sources.add(p.source);
      const entree = modes.get(p.mode) ?? { cle: p.mode, couleur: p.couleur, km: 0 };
      const points: number[][] = entite.geometry.coordinates;
      for (let i = 1; i < points.length; i += 1) {
        entree.km += distanceKm(points[i - 1], points[i]);
      }
      modes.set(p.mode, entree);
    }

    return {
      modes: [...modes.values()]
        .map((m) => ({ ...m, km: Math.round(m.km) }))
        .sort((a, b) => b.km - a.km),
      sources: ["mesuree", "calculee", "manuelle", "heritee"].filter((s) => sources.has(s)),
    };
  });
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
