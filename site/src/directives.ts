/**
 * Résolution des directives du récit : `::photo`, `::galerie`, `::video`.
 *
 * Voir SPEC.md, section 5.2. Les directives ne portent qu'un identifiant :
 * toutes les métadonnées viennent de `media.json`, de sorte qu'un changement
 * de format de dérivé ne demande aucune retouche du texte.
 *
 * Une directive qui cite un identifiant inconnu **fait échouer le build**.
 * Produire un trou silencieux dans un récit relu des années plus tard serait
 * pire que de refuser de construire.
 */
import { visit } from "unist-util-visit";
import type { Media } from "./donnees";
import { parIdentifiant, urlMedia } from "./donnees";

const NOMS = new Set(["photo", "galerie", "video"]);

/** `content/voyages/2026-alpes/jours/2026-08-14.md` donne `2026-alpes`. */
function voyageDepuisChemin(chemin: string): string {
  const morceaux = chemin.replace(/\\/g, "/").split("/");
  const rang = morceaux.lastIndexOf("voyages");
  const voyage = rang >= 0 ? morceaux[rang + 1] : undefined;
  if (!voyage) {
    throw new Error(`Impossible de deviner le voyage depuis « ${chemin} »`);
  }
  return voyage;
}

function echapper(texte: string): string {
  return texte
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * Balise `<img>` complète : trois largeurs en `srcset`, aperçu en fond,
 * dimensions déclarées pour éviter le décalage de mise en page.
 */
function image(media: Media, voyage: string, tailles: string, apercu: boolean): string {
  const d = media.derives!;
  const srcset = [
    `${urlMedia(d.vignette, voyage)} 320w`,
    `${urlMedia(d.moyen, voyage)} 1024w`,
    `${urlMedia(d.grand, voyage)} 2048w`,
  ].join(", ");
  const style = apercu && media.lqip
    ? ` style="background-image:url(${media.lqip});background-size:cover"`
    : "";
  return (
    `<img src="${urlMedia(d.repli, voyage)}" srcset="${srcset}" sizes="${tailles}"` +
    ` width="${media.largeur ?? ""}" height="${media.hauteur ?? ""}"` +
    ` data-grand="${urlMedia(d.grand, voyage)}"` +
    ` loading="lazy" decoding="async" alt=""${style}>`
  );
}

function resoudre(
  identifiant: string,
  index: Map<string, Media>,
  directive: string,
  fichier: string,
): Media {
  const media = index.get(identifiant);
  if (!media) {
    throw new Error(
      `${fichier} : la directive ::${directive} cite « ${identifiant} », ` +
        `qui n'existe pas dans media.json.`,
    );
  }
  if (!media.publie) {
    throw new Error(
      `${fichier} : la directive ::${directive} cite « ${identifiant} », ` +
        `qui est écarté par selection.yaml. Un média cité par le récit doit être retenu (D7).`,
    );
  }
  return media;
}

export function directivesMedias() {
  return (arbre: any, fichier: any) => {
    const chemin: string = fichier.history?.[0] ?? fichier.path ?? "";
    const voyage = voyageDepuisChemin(chemin);
    const index = parIdentifiant(voyage);
    const nom = chemin.split(/[\\/]/).pop() ?? chemin;

    visit(arbre, (noeud: any, rang: number | undefined, parent: any) => {
      if (noeud.type !== "leafDirective" && noeud.type !== "textDirective") return;
      if (!NOMS.has(noeud.name) || !parent || rang === undefined) return;

      const attributs = noeud.attributes ?? {};
      let html: string;

      if (noeud.name === "photo") {
        const media = resoudre(attributs.id, index, "photo", nom);
        const legende = attributs.legende
          ? `<figcaption>${echapper(attributs.legende)}</figcaption>`
          : "";
        html = `<figure class="photo">${image(media, voyage, "(min-width: 48rem) 44rem, 100vw", true)}${legende}</figure>`;
      } else if (noeud.name === "galerie") {
        const identifiants = String(attributs.ids ?? "")
          .split(",")
          .map((i) => i.trim())
          .filter(Boolean);
        if (identifiants.length === 0) {
          throw new Error(`${nom} : la directive ::galerie ne cite aucun identifiant.`);
        }
        const images = identifiants
          .map((i) => image(resoudre(i, index, "galerie", nom), voyage, "(min-width: 48rem) 22rem, 45vw", false))
          .join("");
        html = `<div class="galerie">${images}</div>`;
      } else {
        const media = resoudre(attributs.id, index, "video", nom);
        if (!media.derives) {
          // D8 : le transcodage attend la sélection des vidéos. Le récit peut
          // déjà les citer, la page le dit plutôt que de montrer un vide.
          html = `<p class="video-en-attente">Vidéo « ${echapper(media.id)} », pas encore encodée.</p>`;
        } else {
          html =
            `<video controls preload="none" poster="${urlMedia(media.derives.vignette, voyage)}">` +
            `<source src="${urlMedia(media.derives.moyen, voyage)}" type="video/mp4"></video>`;
        }
      }

      parent.children[rang] = { type: "html", value: html };
    });
  };
}
