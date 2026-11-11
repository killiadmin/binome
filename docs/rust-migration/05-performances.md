# Migration Rust — 05. Performances avant / après

> Seules des **mesures** figurent ici : même machine, même moment, même outil, mêmes
> paramètres, chaque pile seule (l'autre arrêtée), machine maintenue éveillée (`caffeinate`).
> Mesures finales du 2026-11-11 (17 h 24 – 17 h 33 UTC), sur l'**image de production** Rust.

## Protocole

- **Machine** : Apple M1 Max (10 cœurs, 64 Go) ; VM Docker Desktop : 10 CPU, 31 Gio. La pile
  Laravel en service tournait en arrière-plan pendant les deux mesures (conditions identiques).
- **Laravel** : pile de référence (`tests/e2e/ref-stack.sh up`), mêmes images et mêmes données
  que la pile en service (dump du 2026-11-11) : conteneurs MySQL, backend, Reverb, queue.
- **Rust** : image de production `binome:latest` (`docker compose`), sur une copie SQLite des
  mêmes données.
- **Outil** : `tests/e2e/load.mjs`. Chaque « partie » est jouée de bout en bout par 8 joueurs
  simulés (HTTP + WebSocket, chacun attend les events qu'il doit recevoir) : création du salon,
  arrivées, prêts, lancement, vote, accusations jusqu'à la fin, récap, départs.
  - *repos* : 20 s sans trafic ;
  - *WebSockets* : N connexions abonnées au canal public `lobby`, maintenues 15 s ;
  - *charge maximale* : 6 tables enchaînent les parties sans pause pendant 60 s ; chaque
    serveur est poussé à saturation, le nombre de parties jouées dépend de lui ;
  - *charge égale* (`--rate 120`) : exactement 120 parties par minute pour les deux serveurs.
- **Mesures** : `docker stats` en continu (~1 échantillon/s/conteneur) ; latences côté client.

## Résultats

| Indicateur | Laravel | Rust | Rapport |
|---|---|---|---|
| **RAM au repos** | 857 MiB (MySQL 696, queue 63, backend 66, Reverb 33) | **3,0 MiB** | ÷ 285 |
| RAM au repos, pile complète en service (+ serveur Vite) | 1 022 MiB (audit, §3 du doc 01) | — (pas de Node en production) | |
| **CPU au repos** | 5,8 % moyen, pics 17 % (queue 3,5 %, MySQL 2,2 %) | **0 %** | |
| **Charge égale** (120 parties/min) : CPU | 262 % (backend 188 %, MySQL 62 %) | **14,5 %** | ÷ 18 |
| Charge égale : RAM | 944 MiB | **9,5 MiB** | ÷ 99 |
| Charge égale : latence API p50 / p95 | 20,3 / 35,6 ms | **1,9 / 5,9 ms** | ÷ 10 |
| **Charge maximale** : parties jouées en 60 s | 162 | **1 800** | × 11 |
| Charge maximale : débit | 231 req/s | **2 592 req/s** | × 11 |
| Charge maximale : latence p50 / p95 / p99 | 20,3 / 35,3 / 46,3 ms | **1,7 / 4,5 / 8,1 ms** | |
| Charge maximale : CPU / RAM | 349 % / 947 MiB | 193 % / **10,4 MiB** | |
| Charge maximale : CPU par partie jouée | 1,31 CPU·s | **0,065 CPU·s** | ÷ 20 |
| **WebSockets** : 1 000 connexions | toutes tenues ; Reverb +23 MiB | toutes tenues ; **+12 MiB** | |
| WebSockets : 5 000 connexions | 151 refusées, **800 encore ouvertes** après 15 s (voir plus bas) | **5 000 tenues** ; +52 MiB | |
| Échecs (parties, vérifications) | 0 | 0 | |
| **Taille des images** | 932 Mo × 3 (backend, Reverb, queue) + 264 Mo (Vite) + 793 Mo (MySQL) | **9,3 Mo** (une image `scratch`) | |
| **Conteneurs / processus au repos** | 4 conteneurs, 51 PIDs (+ Vite : 1 conteneur, 34 PIDs) | **1 conteneur**, 3 PIDs | |
| Démarrage (jusqu'à la première réponse) | 0,4 s (backend seul, MySQL déjà démarré) | **0,13 s** (tout compris) | |
| Dépendances d'exécution | PHP 8.2 + 5 extensions, 90 paquets Composer, MySQL 8, Node 20 | **aucune** (binaire statique) | |
| Front : paquets npm (lockfile, au build seulement) | 240 | **141** | |
| Front : JavaScript principal (gzip) | 244 Ko | **159 Ko** | − 35 % |

Rapports bruts (`tests/e2e/results/`) : Laravel `load-laravel-ref-2026-11-11T172411.json`
(charge maximale), `…T173027.json` (charge égale), `…T173242.json` (5 000 WebSockets) ; Rust
`load-rust-2026-11-11T172626.json`, `…T172820.json`, `…T173117.json`. La mesure Laravel du
matin (`…T102700.json` : 150 parties, 932 MiB au repos) est cohérente avec celle-ci.

## Lecture et limites

- **RAM** : l'essentiel du gain vient de la disparition de MySQL (696 MiB à lui seul) et des
  processus PHP. SQLite vit dans le processus Rust ; la base (14 Mo) n'est pas chargée entière
  en mémoire, seules les pages lues le sont.
- **CPU à charge égale** : chaque requête Laravel redémarre le framework et fait des
  allers-retours réseau vers MySQL (≈ 20 ms par requête) ; Rust répond en ~2 ms. À l'échelle de
  Binome (quelques parties simultanées), les deux suffisent largement : le gain se traduit
  surtout par un serveur plus petit et moins cher, et des réponses plus vives sur téléphone.
- **WebSockets à 5 000** : Reverb utilise la boucle `stream_select` de ReactPHP, limitée par
  construction à 1 024 descripteurs, faute d'extension PHP `ev` / `event` / `uv` dans l'image
  actuelle (vérifié ; `ulimit -n` vaut 1 048 576 et n'est pas en cause). C'est **la
  configuration actuelle** qui plafonne vers 1 000 connexions ; installer `ext-event` relèverait
  ce plafond. Rust n'a pas cette limite.
- **Charge maximale** : la boucle de test est fermée (chaque joueur attend la réponse pour
  continuer) ; le débit mesuré est la capacité du serveur sur cette machine, pas une prévision
  de trafic. Rust y plafonne à ~2 cœurs (2 threads réseau configurés, plus les écritures
  SQLite sérialisées).
- Les écarts de quelques % entre deux exécutions relèvent du bruit (Laravel : 150 parties le
  matin, 162 l'après-midi).

## Validation fonctionnelle (image de production)

| Contrôle | Résultat |
|---|---|
| Banc API + WebSocket (`run.mjs` : lobby, catalogue, partie à 8 avec spectateur et déconnexions, partie à 7 avec orphelin) | 361 / 361 |
| Contrat JSON comparé à Laravel (`compare.mjs`, réponses et events cumulés sur plusieurs exécutions) | identique |
| Navigateur réel (`browser.mjs` : 4 joueurs dans Chromium, redémarrage du serveur en pleine partie) | 11 / 11 |
| Tests unitaires Rust (règles, parité du détecteur de mots interdits sur 416 cas du JS) | 10 / 10 |
| Migration des données (sur copie) : comptes par table, octets d'images, intégrité | identiques, `ok` |
