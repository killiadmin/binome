#!/usr/bin/env bash
# Sauvegarde de la base Railway sur le Mac (checklist point 4).
#
#   scripts/backup-railway.sh              # une sauvegarde maintenant
#   scripts/backup-railway.sh install      # tâche launchd quotidienne (12:30 par défaut)
#   scripts/backup-railway.sh install 08:15
#   scripts/backup-railway.sh uninstall
#   scripts/backup-railway.sh status       # dernière réussite, dernières lignes du journal
#
# Déroulé : sur Railway, `binome backup` (VACUUM INTO, à chaud) dans /data, `binome check`
# de la copie et empreinte SHA-256 ; rapatriement par `railway ssh`, contrôle de l'empreinte,
# effacement de la copie distante ; gzip dans backups/railway/ :
#   quotidiennes/   binome-railway-AAAAMMJJ-HHMMSS.sqlite.gz   (7 gardées)
#   hebdomadaires/  binome-railway-AAAA-Wss.sqlite.gz          (4, première de la semaine)
#   mensuelles/     binome-railway-AAAA-MM.sqlite.gz           (6, première du mois)
# Échec → notification macOS (le jeton `railway login` peut expirer : `railway login`).
#
# Variables : BINOME_BACKUP_DIR (défaut backups/railway du dépôt), BINOME_BACKUP_MIRROR
# (copie hors du dossier, ex. iCloud Drive ; même rétention). Restauration : procédure
# « Envoyer une base vers Railway » de docs/rust-migration/06-deploiement.md.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="${BINOME_BACKUP_DIR:-$root/backups/railway}"
mirror="${BINOME_BACKUP_MIRROR:-}"
label="com.binome.backup-railway"
plist="$HOME/Library/LaunchAgents/$label.plist"
# launchd fournit un PATH minimal
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

log() { printf '%s %s\n' "$(date '+%F %T')" "$*"; }
notify() {
  osascript -e "display notification \"$1\" with title \"Binome : sauvegarde Railway\"" >/dev/null 2>&1 || true
}
fail() { log "ÉCHEC : $*"; notify "Échec : $*"; exit 1; }

# Garde les $2 fichiers les plus récents du dossier $1 (noms horodatés, tri lexical).
prune() {
  local dir="$1" keep="$2" f
  [[ -d "$dir" ]] || return 0
  # shellcheck disable=SC2012
  ls -1 "$dir" | { grep -E '^binome-railway-.*\.sqlite\.gz$' || true; } | sort -r | tail -n +"$((keep + 1))" | while read -r f; do
    rm -f "$dir/$f" && log "rotation : $dir/$f supprimée"
  done
}

backup() {
  command -v railway >/dev/null || fail "CLI railway introuvable"
  cd "$root" # le projet Railway est lié à ce dossier
  local stamp out sum local_sum attempt
  stamp=$(date +%Y%m%d-%H%M%S)
  # globales : relues par le trap après la sortie de la fonction
  remote="/data/sauvegarde-$stamp.sqlite"
  mkdir -p "$dest"/{quotidiennes,hebdomadaires,mensuelles}
  tmp="$dest/.en-cours-$stamp.sqlite"
  trap 'rm -f "$tmp"; railway ssh -- sh -c "rm -f $remote $remote-*" </dev/null >/dev/null 2>&1 || true' EXIT

  # 1. Copie cohérente et vérifiée côté Railway (3 essais : le réseau tarde parfois au réveil)
  for attempt in 1 2 3; do
    if out=$(railway ssh -- sh -c "binome backup $remote >/dev/null && BINOME_DATABASE=$remote binome check && sha256sum $remote" </dev/null 2>/dev/null); then
      break
    fi
    out=""
    [[ $attempt -lt 3 ]] && { log "essai $attempt échoué, nouvel essai dans 60 s"; sleep 60; }
  done
  [[ -n "$out" ]] || fail "copie ou contrôle impossible sur Railway (réseau, railway login ?)"
  sum=$(printf '%s\n' "$out" | grep -Eo '^[0-9a-f]{64}' | head -1 || true)
  [[ -n "$sum" ]] || fail "empreinte distante absente"

  # 2. Rapatriement puis effacement de la copie distante
  railway ssh -- sh -c "cat $remote && rm -f $remote $remote-*" </dev/null >"$tmp" 2>/dev/null \
    || fail "rapatriement impossible"
  local_sum=$(shasum -a 256 "$tmp" | cut -d' ' -f1)
  [[ "$local_sum" == "$sum" ]] || fail "empreinte différente après transfert"

  # 3. Compression et rangement
  local name="binome-railway-$stamp.sqlite.gz" week month
  gzip -9 -c "$tmp" >"$dest/quotidiennes/.$name" && mv "$dest/quotidiennes/.$name" "$dest/quotidiennes/$name"
  rm -f "$tmp"
  week="$dest/hebdomadaires/binome-railway-$(date +%G-W%V).sqlite.gz"
  month="$dest/mensuelles/binome-railway-$(date +%Y-%m).sqlite.gz"
  [[ -e "$week" ]] || cp "$dest/quotidiennes/$name" "$week"
  [[ -e "$month" ]] || cp "$dest/quotidiennes/$name" "$month"
  prune "$dest/quotidiennes" 7
  prune "$dest/hebdomadaires" 4
  prune "$dest/mensuelles" 6

  # 4. Copie hors du dossier (facultative), même rétention
  if [[ -n "$mirror" ]]; then
    mkdir -p "$mirror" && rsync -a --exclude '.*' --exclude '*.log' "$dest/" "$mirror/" || fail "copie vers $mirror impossible"
    prune "$mirror/quotidiennes" 7
    prune "$mirror/hebdomadaires" 4
    prune "$mirror/mensuelles" 6
  fi

  trap - EXIT # copie distante déjà effacée
  date '+%F %T' >"$dest/derniere-reussite"
  log "OK : $name ($(du -h "$dest/quotidiennes/$name" | cut -f1)) ; $(printf '%s\n' "$out" | grep -E '^(players|games) ' | tr -s ' ' | paste -sd ';' - || true)"
}

install() {
  local at="${1:-12:30}" hour minute mirror_xml=""
  [[ "$at" =~ ^([01]?[0-9]|2[0-3]):([0-5][0-9])$ ]] || { echo "Heure invalide : $at (HH:MM)" >&2; exit 2; }
  hour=$((10#${BASH_REMATCH[1]})) minute=$((10#${BASH_REMATCH[2]}))
  mkdir -p "$dest" "$(dirname "$plist")"
  [[ -n "$mirror" ]] && mirror_xml="<key>EnvironmentVariables</key><dict><key>BINOME_BACKUP_MIRROR</key><string>$mirror</string></dict>"
  cat >"$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array><string>$root/scripts/backup-railway.sh</string></array>
  <key>WorkingDirectory</key><string>$root</string>
  <!-- Mac en veille à l'heure dite : lancée au réveil -->
  <key>StartCalendarInterval</key><dict><key>Hour</key><integer>$hour</integer><key>Minute</key><integer>$minute</integer></dict>
  <key>StandardOutPath</key><string>$dest/sauvegarde.log</string>
  <key>StandardErrorPath</key><string>$dest/sauvegarde.log</string>
  $mirror_xml
</dict>
</plist>
EOF
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$plist"
  echo "Tâche installée : chaque jour à $(printf '%02d:%02d' "$hour" "$minute") ($plist)"
  echo "Journal : $dest/sauvegarde.log"
}

uninstall() {
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  rm -f "$plist"
  echo "Tâche retirée (les sauvegardes restent dans $dest)"
}

status() {
  echo "Dernière réussite : $(cat "$dest/derniere-reussite" 2>/dev/null || echo jamais)"
  launchctl print "gui/$(id -u)/$label" >/dev/null 2>&1 && echo "Tâche launchd : installée" || echo "Tâche launchd : absente"
  tail -n 5 "$dest/sauvegarde.log" 2>/dev/null || true
}

case "${1:-}" in
  "") backup ;;
  install) install "${2:-}" ;;
  uninstall) uninstall ;;
  status) status ;;
  *) echo "usage: $0 [install [HH:MM] | uninstall | status]" >&2; exit 2 ;;
esac
