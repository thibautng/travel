/**
 * Sert les dérivés d'images depuis R2, sous la même adresse que le site.
 *
 * L'alternative était l'adresse publique `r2.dev` que Cloudflare attache à un
 * dépôt. Elle est bridée et présentée par Cloudflare comme un outil d'essai,
 * ce qui va mal à une page de journée qui demande soixante images d'un coup.
 *
 * En passant par le Worker, les médias sont sur le domaine du site : pas de
 * seconde origine, pas de CORS, rien à configurer côté site. `urlMedia()`
 * garde sa valeur par défaut, `/media`, la même qu'en développement, et
 * `PUBLIC_MEDIA_URL` reste inutile.
 *
 * Tout ce qui n'est pas `/media/` retombe sur les fichiers statiques, servis
 * par la plateforme sans passer par ce code.
 *
 * Écrit en JavaScript et non en TypeScript : la racine du dépôt n'a pas de
 * chaîne de compilation, et Wrangler ne vérifie pas les types de toute façon.
 * Des types ici seraient une promesse que rien ne tient.
 */

const PREFIXE = "/media/";

/**
 * L'ancien lecteur jour par jour, fondu dans la page du voyage.
 *
 * Une seule carte interactive par voyage, une seule adresse. La redirection
 * est permanente : les liens déjà partagés continuent de fonctionner, et le
 * paramètre `?jour=` est conservé, le lecteur sachant l'ouvrir.
 */
const ANCIEN_LECTEUR = /^\/voyages\/([^/]+)\/carte\/?$/;

/** Les dérivés ne changent jamais sous un même nom : le pipeline en écrit de nouveaux. */
const CACHE_MEDIA = "public, max-age=31536000, immutable";

export default {
  async fetch(requete, env) {
    const url = new URL(requete.url);

    const ancien = ANCIEN_LECTEUR.exec(url.pathname);
    if (ancien) {
      const cible = new URL(`/voyages/${ancien[1]}/`, url);
      cible.search = url.search;
      cible.hash = "lecteur";
      return Response.redirect(cible.toString(), 301);
    }

    if (!url.pathname.startsWith(PREFIXE)) {
      return env.ASSETS.fetch(requete);
    }
    if (requete.method !== "GET" && requete.method !== "HEAD") {
      return new Response("Méthode non autorisée", { status: 405 });
    }

    // Les clés sont rangées en clair dans R2, l'adresse les porte encodées.
    const cle = decodeURIComponent(url.pathname.slice(PREFIXE.length));
    if (!cle || cle.includes("..")) {
      return new Response("Chemin invalide", { status: 400 });
    }

    if (requete.method === "HEAD") {
      const entete = await env.MEDIAS.head(cle);
      if (entete === null) return new Response(null, { status: 404 });
      return new Response(null, { headers: entetes(entete) });
    }

    // `range` sert les requêtes de plage sans rapatrier l'objet entier, ce
    // dont une vidéo a besoin pour se lire sans être téléchargée d'abord.
    // `onlyIf` répond 304 quand le navigateur a déjà la bonne version.
    const objet = await env.MEDIAS.get(cle, {
      range: requete.headers,
      onlyIf: requete.headers,
    });
    if (objet === null) {
      return new Response("Média introuvable", { status: 404 });
    }
    if (objet.body === undefined || objet.body === null) {
      return new Response(null, { status: 304, headers: entetes(objet) });
    }

    if (objet.range && requete.headers.has("range")) {
      const debut = objet.range.offset ?? 0;
      const longueur = objet.range.length ?? objet.size - debut;
      const enTetes = entetes(objet);
      enTetes.set("content-range", `bytes ${debut}-${debut + longueur - 1}/${objet.size}`);
      return new Response(objet.body, { status: 206, headers: enTetes });
    }

    return new Response(objet.body, { headers: entetes(objet) });
  },
};

/** Type de contenu et validateurs, tels que R2 les a enregistrés. */
function entetes(objet) {
  const en = new Headers();
  objet.writeHttpMetadata(en);
  en.set("etag", objet.httpEtag);
  en.set("cache-control", CACHE_MEDIA);
  en.set("accept-ranges", "bytes");
  return en;
}
