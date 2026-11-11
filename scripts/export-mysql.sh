#!/usr/bin/env bash
# Export LECTURE SEULE des tables métier de l'ancienne base MySQL en NDJSON
# (une ligne JSON par enregistrement), pour `binome import-mysql <dossier>`.
#
#   scripts/export-mysql.sh <dossier-de-sortie> [conteneur-mysql]   (défaut : binome_db)
#
# Seules des requêtes SELECT sont exécutées. Les dates sont lues en UTC.
set -euo pipefail

out="${1:?usage: $0 <dossier-de-sortie> [conteneur-mysql]}"
container="${2:-binome_db}"
mkdir -p "$out"

tables=(players cosmos universes characters rooms room_player games binomes binome_player rounds actions
        question_votes game_stats chat_messages player_avatars custom_reactions)

mysql_q() {
  docker exec -i "$container" sh -c 'mysql -uroot -p"$MYSQL_ROOT_PASSWORD" --default-character-set=utf8mb4 --raw -N -B binome 2>/dev/null'
}

for t in "${tables[@]}"; do
  # Requête JSON_OBJECT générée depuis information_schema (toutes les colonnes de la table)
  select=$(mysql_q <<SQL
SET SESSION group_concat_max_len = 100000;
SELECT CONCAT('SELECT JSON_OBJECT(',
              GROUP_CONCAT(CONCAT('''', column_name, ''', \`', column_name, '\`') ORDER BY ordinal_position),
              ') FROM \`', table_name, '\` ORDER BY id;')
FROM information_schema.columns
WHERE table_schema = 'binome' AND table_name = '$t'
GROUP BY table_name;
SQL
)
  printf "SET time_zone = '+00:00';\n%s\n" "$select" | mysql_q > "$out/$t.ndjson"
  printf '%-18s %6s lignes\n' "$t" "$(wc -l < "$out/$t.ndjson" | tr -d ' ')"
done

# Contrôle à comparer avec le rapport de `binome import-mysql`
printf 'octets d images de personnages : %s\n' \
  "$(echo 'SELECT COALESCE(SUM(LENGTH(image)), 0) FROM characters;' | mysql_q)"
