import { defineConfig, passthroughImageService } from "astro/config";
import { createReadStream, existsSync, statSync } from "node:fs";
import path from "node:path";

const RACINE = process.env.VOYAGES_RACINE ?? path.resolve(process.cwd(), "..");

const TYPES = {
  ".avif": "image/avif",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".png": "image/png",
  ".webp": "image/webp",
  ".mp4": "video/mp4",
};

/**
 * Sert `media/` sous `/media` pendant le développement.
 *
 * Une jonction dans `public/` ferait l'affaire à l'écran, mais Astro copie
 * `public/` dans `dist/` au build : les 838 Mo de dérivés se retrouveraient
 * dans le déploiement, alors qu'ils sont destinés à R2. D'où ce service
 * limité au serveur de développement, `apply: "serve"`.
 */
function mediasLocaux() {
  const dossier = path.join(RACINE, "media");
  return {
    name: "medias-locaux",
    apply: "serve",
    configureServer(serveur) {
      serveur.middlewares.use("/media", (requete, reponse, suivant) => {
        const relatif = decodeURIComponent((requete.url ?? "/").split("?")[0]);
        const chemin = path.normalize(path.join(dossier, relatif));
        // Garde contre la remontée de chemin.
        if (!chemin.startsWith(dossier) || !existsSync(chemin) || !statSync(chemin).isFile()) {
          return suivant();
        }
        reponse.setHeader(
          "Content-Type",
          TYPES[path.extname(chemin).toLowerCase()] ?? "application/octet-stream",
        );
        reponse.setHeader("Cache-Control", "public, max-age=3600");
        createReadStream(chemin).pipe(reponse);
      });
    },
  };
}

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
    plugins: [mediasLocaux()],
  },
});
