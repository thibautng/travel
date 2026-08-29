/**
 * Collection des journées, lue dans `content/voyages/<id>/jours/`.
 *
 * Le contenu vit hors de `site/`, conformément à l'arborescence de la
 * section 4 : `content/` s'écrit à la main et ne dépend pas du générateur.
 *
 * Le schéma reprend le frontmatter de la section 5.2. Les champs chiffrés
 * sont facultatifs : Notion ne les portait pas, et la spec veut qu'ils
 * restent vides plutôt qu'inventés.
 */
import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const journees = defineCollection({
  // Le motif englobe tous les voyages, pour que le lot 7 n'ait rien à changer.
  loader: glob({ pattern: "*/jours/*.md", base: "../content/voyages" }),
  schema: z.object({
    date: z.coerce.date(),
    titre: z.string(),
    lieu: z.string().optional(),
    camp: z.string().optional(),
    couverture: z.string().optional(),
    etiquettes: z.array(z.string()).default([]),
    distance_marche_km: z.number().optional(),
    denivele_m: z.number().optional(),
    altitude_max_m: z.number().optional(),
    temps_fort: z.boolean().default(false),
  }),
});

export const collections = { journees };
