import { defineConfig, passthroughImageService } from "astro/config";

// Voir SPEC.md, D2 et section 9.4 : site statique, JavaScript uniquement là
// où il est nécessaire. Aucune intégration n'est ajoutée tant qu'une page
// n'en a pas besoin ; la carte, au lot 5, en amènera une.
export default defineConfig({
  // Le site est public mais non indexé. Voir section 10.
  site: "https://voyages.exemple",
  output: "static",
  image: {
    // Astro n'a rien à retoucher : `carnet` a déjà produit les trois tailles,
    // le repli et l'aperçu. Le service de passage évite d'embarquer `sharp`,
    // qui referait le travail plus mal et alourdirait l'installation.
    service: passthroughImageService(),
  },
  build: {
    // Une page par dossier, pour des URL en /voyages/2026-alpes/ plutôt
    // qu'en /voyages/2026-alpes.html.
    format: "directory",
  },
  vite: {
    server: {
      // `media/` est hors de site/, atteint par une jonction en développement.
      fs: { allow: [".."] },
    },
  },
});
