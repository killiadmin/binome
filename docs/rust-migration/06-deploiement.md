# Migration Rust — 06. Déploiement

Binome tourne dans **un seul conteneur** de 9 Mo (binaire Rust + front compilé, image vide
`scratch`, utilisateur non root, système de fichiers en lecture seule). Données : un fichier
SQLite dans le volume `binome_data`. Au repos : ~5 Mo de RAM, 0 % de CPU.

| Fichier | Rôle |
|---|---|
| `Dockerfile` | Image de production (front → binaire musl → `scratch`) |
| `docker-compose.yml` | Production, HTTP sur le port 8080 — **jamais seul sur le VPS** |
| `docker-compose.https.yml` | + Caddy : HTTPS automatique (Let's Encrypt) — **VPS** |
| `docker-compose.portail.yml` | Réseau local derrière `../portail` (utilisé par `./start.sh`) |
| `docker-compose.dev.yml` | Développement : Vite (HMR) + `cargo run` |

## VPS (Hostinger KVM ou équivalent)

**Prérequis.** Un VPS Linux (pas un hébergement mutualisé : il faut lancer un processus
serveur et Docker). La plus petite offre suffit : Binome utilise quelques dizaines de Mo,
Caddy autant. Prévoir ~2 Go de RAM **pour la compilation** si l'image est construite sur le
VPS (optimisations à l'édition de liens) ; sinon la construire ailleurs (voir plus bas).
Un nom de domaine dont l'enregistrement DNS `A` pointe vers l'IP du VPS ; ports 80 et 443
ouverts dans le pare-feu.

**Première installation**

```bash
# Sur le VPS : Docker (script officiel), puis le code
curl -fsSL https://get.docker.com | sh
git clone <dépôt> binome && cd binome && git checkout <branche>

cp .env.example .env && chmod 600 .env
# renseigner BINOME_DOMAIN, décommenter COMPOSE_FILE et BINOME_PORT (section « VPS »), puis
# l'empreinte du mot de passe admin, et (facultatif) de suppression : voir « Mots de passe » plus bas
docker compose up -d --build  # = docker-compose.yml + docker-compose.https.yml (COMPOSE_FILE)
```

**HTTP en clair impossible.** `docker-compose.yml` seul publie le port 8080 en HTTP, et Docker
publie ses ports **en contournant UFW** : le pare-feu de l'hôte ne le bloquerait pas. D'où deux
lignes dans le `.env` du VPS :

```bash
COMPOSE_FILE=docker-compose.yml:docker-compose.https.yml   # toute commande docker compose sans -f
BINOME_PORT=127.0.0.1:8080    # filet : si docker-compose.yml est lancé seul, port local au VPS
```

Contrôle depuis une autre machine : `curl -m 5 http://<IP du VPS>:8080/` doit échouer, et
`http://<domaine>/` doit rediriger vers `https://`. Toutes les commandes `docker compose` de ce
document s'écrivent alors sans `-f` sur le VPS (elles sont données en forme longue, valable partout).

L'application répond sur `https://<domaine>/binome/` (la racine redirige). Pour la servir à la
racine du domaine : `BINOME_PUBLIC_PREFIX=/` dans `.env` avant le build.

**Transférer les données actuelles** (depuis la machine où Binome tourne aujourd'hui)

```bash
docker exec binome /binome backup /data/transfert.sqlite        # copie cohérente, à chaud
docker run --rm -v binome_binome_data:/data -v "$PWD":/out alpine mv /data/transfert.sqlite /out/binome.sqlite
scp binome.sqlite vps:binome/
# Sur le VPS, conteneur arrêté :
docker compose -f docker-compose.yml -f docker-compose.https.yml stop binome
docker run --rm -v binome_binome_data:/data -v "$PWD":/in alpine \
  sh -c 'cp /in/binome.sqlite /data/ && rm -f /data/binome.sqlite-* && chown -R 10001:10001 /data'
docker compose -f docker-compose.yml -f docker-compose.https.yml start binome
docker exec binome /binome check
```

Le catalogue de personnages à importer (`data/characters/`, hors dépôt) se copie à part s'il
doit servir sur le VPS (voir `docs/generation-personnages.md`).

**Mettre à jour**

```bash
git pull && docker compose -f docker-compose.yml -f docker-compose.https.yml up -d --build
```

Redémarrer coupe les WebSockets quelques secondes : les joueurs se reconnectent seuls et
retrouvent leur partie ; un tirage de carte en cours est réarmé au démarrage.

**Construire l'image ailleurs que sur le VPS** (VPS à 1 Go de RAM, par exemple) — sur une
machine x86_64, ou en émulation (lent depuis un Mac Apple Silicon) :

```bash
docker buildx build --platform linux/amd64 -t binome:latest --load .
docker save binome:latest | gzip | ssh vps 'gunzip | docker load'
# puis, sur le VPS : docker compose ... up -d   (sans --build)
```

## Railway (en service depuis le 2026-11-13)

Projet `eloquent-illumination`, service `binome`, https://binome-production.up.railway.app/.
Railway construit l'image depuis la branche `main` du dépôt GitHub (**chaque push redéploie**) et
fournit le HTTPS : pas de Caddy, pas de pare-feu, pas de serveur à administrer. L'image vient de
`Dockerfile.railway`, qui reprend les étapes de build du `Dockerfile`
(**à garder synchronisées**), mais en diffère à l'exécution :

| | `Dockerfile` (Mac, VPS) | `Dockerfile.railway` |
|---|---|---|
| Base | `scratch` | `alpine` (shell pour `railway ssh`) |
| `VOLUME /data` | oui | non (refusé par Railway) |
| Utilisateur | 10001 | root (volumes Railway montés en root) |
| Préfixe par défaut | `/binome` | `/` (racine du domaine) |

**Réglages du service** — tous dans Railway, aucun fichier de config dans le dépôt (le Config as
Code `railway.toml` est abandonné par Railway au 2026-12-01 ; son remplaçant, l'IaC
`.railway/railway.ts`, demande un paquet npm et une reprise en main du projet : écarté) :
- variable `RAILWAY_DOCKERFILE_PATH=Dockerfile.railway` (sans elle, Railway prendrait le
  `Dockerfile` scratch, dont l'instruction `VOLUME` fait échouer le build) ;
- *Settings → Deploy* : *Healthcheck Path* `/up`, *Restart Policy* « On Failure » ;
- volume monté sur `/data` ;
- **1 seule réplique** : SQLite n'a qu'un écrivain et le hub WebSocket est en mémoire ;
- « App Sleeping » **désactivé** : la mise en veille coupe les WebSockets et les minuteries ;
- région Europe ; port cible du domaine : **8080** (le serveur lit `BINOME_BIND`, pas `PORT`) ;
- variables : `BINOME_ADMIN_PASSWORD_HASH`, `BINOME_ADMIN_PSEUDO`, `BINOME_DELETE_PASSWORD_HASH`
  (pas de `.env`). `BINOME_PUBLIC_PREFIX`, si on la définit, sert au build et à l'exécution ;
  adapter alors le *Healthcheck Path* (`<préfixe>/up`).

**En-têtes de sécurité** (pas de Caddy sur Railway : posés par le serveur, `server/src/http/security.rs`,
sur toutes les réponses) : `nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy`,
`Permissions-Policy` (caméra autorisée pour les photos), HSTS via `BINOME_HSTS=1` (fixé dans
`Dockerfile.railway` ; jamais sur le Mac en HTTP). Pages HTML : CSP, en **Report-Only** tant que
`CSP_ENFORCE` vaut `false` — rien n'est bloqué, les violations sont envoyées à `/api/csp-report`
(30 rapports/min au plus) et journalisées : `railway logs | grep CSP`. Passer `CSP_ENFORCE` à
`true` une fois les journaux propres après de vraies parties sur téléphone. Les polices sont
servies par l'application (`frontend/src/assets/fonts/`), plus par Google Fonts.

**Administration** : `railway ssh`, puis `binome check`, `binome backup /data/x.sqlite`,
`binome prune-rooms`… (le binaire est dans le `PATH`).

`railway ssh` exige une clé SSH enregistrée (`railway ssh keys add`) et une première connexion
interactive pour accepter l'empreinte du serveur ; ensuite il transmet l'entrée standard, ce qui
sert aux transferts ci-dessous (testés le 2026-11-13).

**Envoyer une base vers Railway** (remplace la base en service) :

```bash
# 1. Sauvegarde cohérente sur la machine source (procédure « Transférer les données actuelles »)
# 2. Envoi sous un nom temporaire, contrôle d'empreinte et d'intégrité
shasum -a 256 binome.sqlite
railway ssh -- sh -c 'cat > /data/import.sqlite && sha256sum /data/import.sqlite && BINOME_DATABASE=/data/import.sqlite binome check' < binome.sqlite
# 3. Échange puis redémarrage immédiat (le serveur en marche garde l'ancien fichier ouvert)
railway ssh -- sh -c 'cd /data && rm -f import.sqlite-* binome.sqlite-wal binome.sqlite-shm && mv import.sqlite binome.sqlite' < /dev/null
railway restart -y
railway ssh -- binome check < /dev/null
```

**Récupérer la base depuis Railway** (sauvegarde, ou sortie de Railway) :

```bash
# le message de `binome backup` et ceux de la CLI ne doivent pas se mêler au fichier
railway ssh -- sh -c 'binome backup /data/sortie.sqlite >/dev/null && cat /data/sortie.sqlite && rm /data/sortie.sqlite' < /dev/null > binome-railway.sqlite 2>/dev/null
docker run --rm -v "$PWD":/b -e BINOME_DATABASE=/b/binome-railway.sqlite binome:latest check
```

Chaque redéploiement coupe le service quelques secondes (volume attaché : pas de recouvrement).
**Sauvegardes** : copie quotidienne sur le Mac par `scripts/backup-railway.sh` (tâche launchd,
voir « Sauvegardes » plus bas), en plus des sauvegardes de volume natives de Railway (onglet
*Backups* du volume) qui, elles, restent chez Railway et disparaissent avec le volume.

## Mots de passe (administration, suppression de parties)

Le serveur ne stocke que des empreintes (PBKDF2-SHA256), jamais les mots de passe :
- **administration** (`BINOME_ADMIN_PASSWORD_HASH`) : sans elle, l'administration est fermée.
  Connexion limitée à 2 tentatives par minute, tous clients confondus (429 au-delà) ;
- **suppression de parties** (`BINOME_DELETE_PASSWORD_HASH`) : seconde clé, distincte ; sans
  elle, la suppression de parties est désactivée (les autres actions admin restent possibles).

Pour les définir (ou les changer), image construite :

```bash
# Linux / VPS (bash) — sur macOS (zsh), la 1re ligne s'écrit : read -rs "P?Mot de passe : "; echo
read -rs -p "Mot de passe : " P; echo
printf '%s\n' "$P" | docker run --rm -i binome:latest hash-password --admin; unset P   # administration
printf '%s\n' "$P" | docker run --rm -i binome:latest hash-password; unset P           # suppression
# → copier la ligne BINOME_…_HASH=… affichée dans .env, puis :
docker compose up -d          # ou ./start.sh -d en local
```

Changer le mot de passe admin invalide les sessions admin ouvertes (le jeton dérive de
l'empreinte). L'ancienne variable en clair `BINOME_ADMIN_PASSWORD` est ignorée : la retirer du `.env`.

## Mises à jour du schéma

Les évolutions de la base sont des migrations numérotées (`server/migrations/000N_*.sql`),
appliquées une seule fois au démarrage (version dans `PRAGMA user_version`, affichée par
`binome check`). **Faire une sauvegarde avant toute mise à jour** qui en contient une.

## Sauvegardes

```bash
docker exec binome /binome backup /data/sauvegarde-$(date +%F).sqlite   # à chaud, cohérent
docker run --rm -v binome_binome_data:/data -v "$PWD":/out alpine sh -c 'mv /data/sauvegarde-*.sqlite /out/'
docker exec binome /binome check                                         # intégrité
```

À planifier par `cron` sur le VPS (une fois par jour suffit) et à copier hors du VPS. La base
complète fait ~14 Mo, dont 13 Mo d'images de personnages.

L'outil `sqlite3` standard peut lire la base mais pas la contrôler ni requêter les pseudos :
la collation `BINOME_CI` (insensible à la casse et aux accents) n'existe que dans le binaire.

### Railway → Mac (en service depuis le 2026-11-14)

`scripts/backup-railway.sh` : `binome backup` + `binome check` + SHA-256 sur Railway, rapatriement
par `railway ssh`, contrôle de l'empreinte, effacement de la copie distante, gzip dans
`backups/railway/` (ignoré par Git) avec rotation : 7 quotidiennes, 4 hebdomadaires, 6 mensuelles.

```bash
scripts/backup-railway.sh                 # une sauvegarde maintenant (~6 s, ~10 Mo compressée)
scripts/backup-railway.sh install [HH:MM] # tâche launchd quotidienne (12:30 par défaut ; au réveil si le Mac dormait)
scripts/backup-railway.sh status          # dernière réussite + fin du journal (backups/railway/sauvegarde.log)
scripts/backup-railway.sh uninstall
```

Un échec (réseau, jeton `railway login` expiré, contrôle raté) affiche une notification macOS ;
`railway login` puis relancer le script. Mac éteint toute la journée = pas de sauvegarde ce
jour-là. Copie hors du Mac : `BINOME_BACKUP_MIRROR=<dossier> scripts/backup-railway.sh install`
(même rotation dans ce dossier). Contrôle mensuel d'une sauvegarde :

```bash
mkdir -p /tmp/r && gunzip -c backups/railway/mensuelles/<fichier>.sqlite.gz > /tmp/r/b.sqlite
docker run --rm -v /tmp/r:/r -e BINOME_DATABASE=/r/b.sqlite binome:latest check
```

Restauration : décompresser, puis « Envoyer une base vers Railway » ci-dessus.

## Réseau local (comme aujourd'hui)

```bash
./start.sh        # http://killiadmin.local:8095/binome/ via ../portail, et localhost:8080/binome/
```

Le conteneur prend l'alias `binome_frontend:5173` sur le réseau `meszappli` : le portail n'a
rien à changer. Le lien « Partager le lien » suit les changements de Wi-Fi comme avant.

## Ancienne version (Laravel)

L'ancien backend (Laravel + Reverb + MySQL) a été supprimé en phase 7 ; sa dernière version est
l'étiquette git `laravel-final`, et sa base MySQL est conservée en dump
(`~/Meszappli/backups/binome/2026-11-11-bascule/binome.sql`). Les parties jouées depuis la
bascule n'existent que dans la base SQLite.
