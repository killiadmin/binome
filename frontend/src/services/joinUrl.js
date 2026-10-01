// Détermine, au moment du partage, l'URL que les autres joueurs du même Wi-Fi
// doivent ouvrir.
//
// - Page ouverte via l'IP LAN (http://192.168.x.x:5173) : l'origine courante
//   est déjà la bonne.
// - Page ouverte en localhost (cas de l'hôte) : on relit public/lan-url.json,
//   tenu à jour par « scripts/update-lan-ip.sh --watch » (lancé par start.sh),
//   puis on vérifie que cette adresse répond vraiment — si l'hôte a changé de
//   réseau sans que le fichier suive, l'ancienne IP n'est plus joignable.
//
// Renvoie { url, verified } ; url vaut null si aucune adresse LAN n'est connue.

const LOCAL_HOSTS = ['localhost', '127.0.0.1', '[::1]'];

export function isOnLocalhost() {
  return LOCAL_HOSTS.includes(window.location.hostname);
}

// Requête « no-cors » : la réponse est opaque mais la promesse ne rejette qu'en
// cas d'échec réseau (IP plus attribuée, port fermé) — exactement ce qu'on teste.
async function isReachable(url, timeoutMs = 2000) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    await fetch(new URL('lan-url.json?probe=' + Date.now(), url), {
      mode: 'no-cors',
      cache: 'no-store',
      signal: ctrl.signal,
    });
    return true;
  } catch (e) {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

export async function resolveJoinUrl() {
  if (!isOnLocalhost()) {
    return {url: window.location.origin + '/', verified: true};
  }
  let url = null;
  try {
    const res = await fetch('/lan-url.json?t=' + Date.now(), {cache: 'no-store'});
    if (res.ok) url = (await res.json())?.url || null;
  } catch (e) {
    /* pas de fichier : pas d'adresse LAN connue */
  }
  if (!url) return {url: null, verified: false};
  return {url, verified: await isReachable(url)};
}
