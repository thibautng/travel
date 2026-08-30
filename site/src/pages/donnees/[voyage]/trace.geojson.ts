/**
 * Sert `data/<voyage>/trace.geojson` au site.
 *
 * Un point de sortie plutôt qu'un fichier copié dans `public/` : la trace
 * vit dans `data/`, produit par le pipeline, et la section 4 veut que le
 * site la lise là où elle est plutôt qu'en tenir une copie. Astro l'écrit
 * dans `dist/` au build et la sert telle quelle en développement.
 */
import type { APIRoute } from "astro";
import { tousLesVoyages } from "../../../voyages";
import { traceBrute } from "../../../donnees";

export function getStaticPaths() {
  return tousLesVoyages().map((v) => ({ params: { voyage: v.id } }));
}

export const GET: APIRoute = ({ params }) =>
  new Response(traceBrute(params.voyage!), {
    headers: {
      "Content-Type": "application/geo+json; charset=utf-8",
      // Le contenu change avec le pipeline, pas avec la navigation.
      "Cache-Control": "public, max-age=3600",
    },
  });
