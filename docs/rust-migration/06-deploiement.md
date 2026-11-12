# Migration Rust — 06. Déploiement

Binome tourne dans **un seul conteneur** de 9 Mo (binaire Rust + front compilé, image vide
`scratch`, utilisateur non root, système de fichiers en lecture seule). Données : un fichier
SQLite dans le volume `binome_data`. Au repos : ~5 Mo de RAM, 0 % de CPU.

| Fichier | Rôle |
|---|---|
| `Dockerfile` | Image de production (front → binaire musl → `scratch`) |
| `docker-compose.yml` | Production, HTTP sur le port 8080 |
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

cp .env.example .env          # renseigner BINOME_DOMAIN
# empreinte du mot de passe admin, et (facultatif) de suppression : voir « Mots de passe » plus bas
docker compose -f docker-compose.yml -f docker-compose.https.yml up -d --build
```

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
