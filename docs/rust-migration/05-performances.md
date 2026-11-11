# Migration Rust — 05. Performances avant / après

> Seules des **mesures** figurent ici, prises avec le même outil, sur la même machine et
> avec les mêmes paramètres.

## Protocole

- **Machine** : Apple M1 Max (10 cœurs, 64 Go) ; VM Docker Desktop : 10 CPU, 31 Gio.
- **Pile mesurée** : pile de référence jetable (`tests/e2e/ref-stack.sh up`), restaurée du
  dump du 2026-11-11 : mêmes images et mêmes données que la pile en service.
- **Outil** : `node tests/e2e/load.mjs --target <cible> --games 6 --duration 60 --ws 1000 --idle 20`
  - *repos* : 20 s sans trafic ;
  - *WebSockets* : 1 000 connexions abonnées au canal public `lobby`, maintenues 15 s ;
  - *charge* : 6 tables de 8 joueurs qui enchaînent des parties complètes pendant 60 s
    (création du salon, arrivées, prêts, lancement, vote des personnages, accusations
    jusqu'à la fin, récap, départs), avec tous les events WebSocket attendus par chaque joueur.
- **Mesures** : `docker stats` en continu (~1 échantillon/s/conteneur) ; latences HTTP côté client.
- Rapport brut : `tests/e2e/results/load-<cible>-<date>.json`.

## Résultats

| Indicateur | Laravel (référence, 2026-11-11) | Rust — **provisoire** (phase 2) |
|---|---|---|
| RAM au repos | **932 MiB** (MySQL 739, backend 86, queue 62, Reverb 45) | **2,3 MiB** (moy.), 3,3 max |
| RAM au repos, pile en service complète (+ Vite) | **1 022 MiB** (audit §3 ; Vite 222 MiB) | — (pas de Vite en production) |
| CPU au repos | **6,4 %** moyen, pics 17,7 % | **0 %** moyen, pics 0,05 % |
| Processus | 55 PIDs, 4 conteneurs (+ 34 PIDs pour Vite) | 1 processus, 4 threads |
| Charge : parties jouées en 60 s | **150** | **1 220** |
| Charge : débit | **212 req/s** | **1 755 req/s** |
| Charge : latence API | p50 **22,2 ms**, p95 **37,3 ms**, p99 49,2 ms | p50 **2,7 ms**, p95 **7,9 ms**, p99 11 ms |
| Charge : CPU | **332 %** moyen (backend PHP 242 %, MySQL 76 %) | **97 %** moyen (≈ 1 cœur) |
| Charge : RAM | **961 MiB** moyen, 977 max | **31,5 MiB** moyen, 33,5 max |
| WebSockets : 1 000 connexions tenues | oui ; Reverb +11 MiB | oui ; +13 MiB (après réduction des tampons, voir ci-dessous) |
| Taille des images Docker | 932 MB (×3) + 264 MB + 793 MB | **9,3 MB** (une image `scratch` : binaire 6,6 MB + front) |
| Conteneurs / processus en production | 5 conteneurs, 86 PIDs | **1 conteneur** (2 avec HTTPS : + Caddy) |

> **Provisoire** : serveur Rust compilé en `release` mais lancé dans une image de
> compilation (pas l'image finale), sur une copie de la base. Ces mesures seront
> refaites en phase 6, machine maintenue éveillée (`caffeinate`) : une partie des
> exécutions de l'après-midi du 2026-11-11 a été faussée par la mise en veille du Mac
> (capot fermé) et a été écartée. Seule la mesure de 15 h 16 – 15 h 18 (heure locale,
> machine éveillée) est retenue pour la charge ; les valeurs de RAM (au repos, avec
> 1 000 WebSockets) ne dépendent pas du temps écoulé.
> Le rapport brut de cette mesure n'a pas été conservé (supprimé par erreur lors d'un
> nettoyage) : les chiffres ci-dessus viennent de la sortie de l'outil, et seront
> remplacés par une mesure complète et archivée en phase 6.

### Lecture

- Le débit est limité par le backend PHP (`artisan serve`, 4 workers), saturé à ~2,4 cœurs :
  à 6 tables, chaque requête attend son tour. La latence elle-même (≈ 22 ms) est surtout
  le coût d'amorçage de Laravel à chaque requête.
- Au repos, `queue:listen` (3,6 %) et MySQL (2,7 %) consomment du CPU sans aucun trafic.
- Reverb est déjà léger (11 MiB pour 1 000 connexions) : le gain WebSocket attendu porte sur
  la suppression d'un processus et des appels HTTP backend → Reverb, pas sur la RAM par connexion.
- Première mesure Rust : 1 000 connexions coûtaient 112 MiB (tampons de 128 Kio par connexion
  par défaut). Réduits à 4 Kio (les messages de Binome font quelques Ko) : 13 MiB.
- En charge, Rust joue 8 fois plus de parties avec 3,4 fois moins de CPU : la latence ne
  porte plus le démarrage du framework à chaque requête, et SQLite évite un aller-retour
  réseau vers MySQL.
