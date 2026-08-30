/**
 * Sert la trace d'une seule journée.
 *
 * La page d'une journée n'a que faire des vingt-deux autres : le fichier
 * complet pèse 246 Ko une fois comprimé, pour une carte cadrée sur un seul
 * jour. Découpé, chacun tient dans quelques kilooctets.
 *
 * Le découpage se fait ici et non dans le pipeline : `data/` garde un seul
 * fichier, qui reste la source, et le site en tire ce dont chaque page a
 * besoin. C'est la même règle que pour `trace.geojson`, servi tel quel.
 */
import type { APIRoute } from "astro";
import { tousLesVoyages } from "../../../../voyages";
import { trace, jours } from "../../../../donnees";

export function getStaticPaths() {
  return tousLesVoyages().flatMap((v) =>
    jours(v.id).map((j) => ({ params: { voyage: v.id, jour: j.jour } })),
  );
}

export const GET: APIRoute = ({ params }) => {
  const complete = trace(params.voyage!);
  const dujour = {
    type: "FeatureCollection",
    features: (complete.features ?? []).filter(
      (f: any) => f.properties?.jour === params.jour,
    ),
  };
  return new Response(JSON.stringify(dujour), {
    headers: {
      "Content-Type": "application/geo+json; charset=utf-8",
      "Cache-Control": "public, max-age=3600",
    },
  });
};
