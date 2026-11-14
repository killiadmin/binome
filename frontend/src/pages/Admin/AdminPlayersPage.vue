<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { BContainer, BModal } from 'bootstrap-vue-next'
import { adminService } from '../../services/adminService'
import { isAdminProfile } from '../../services/profile'
import PlayerAvatar from '../../components/player/PlayerAvatar.vue'

// ─── STATE ────────────────────────────────────────────────────────────────────

const players = ref([])
const loading = ref(true)
const error = ref(null)
const success = ref(null)
const busy = reactive({})

// Suppression
const showConfirm = ref(false)
const confirmTarget = ref(null)
const withGames = ref(false)
// Exigé quand la suppression emporte des parties ; jamais conservé
const deletePassword = ref('')
const deleting = ref(false)

const search = ref('')
const sort = ref('pseudo')
// Seulement les profils en attente de validation
const onlyPending = ref(false)

const SORTS = {
  pseudo: 'Pseudo',
  games: 'Parties',
  wins: 'Victoires',
  recent: 'Récents',
}

const filteredPlayers = computed(() => {
  // Même tolérance que la base : casse et accents ignorés.
  const norm = (s) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
  const q = norm(search.value.trim())
  const list = players.value.filter((p) => (!q || norm(p.pseudo).includes(q)) && (!onlyPending.value || !p.is_validated))

  const by = {
    pseudo: (a, b) => a.pseudo.localeCompare(b.pseudo, 'fr'),
    games: (a, b) => b.games_count - a.games_count,
    wins: (a, b) => b.wins - a.wins || b.score - a.score,
    recent: (a, b) => (b.created_at ?? '').localeCompare(a.created_at ?? ''),
  }
  return list.sort(by[sort.value])
})

const inRoomCount = computed(() => players.value.filter((p) => p.room_code).length)
const pendingCount = computed(() => players.value.filter((p) => !p.is_validated).length)

// ─── CHARGEMENT ───────────────────────────────────────────────────────────────

async function loadPlayers() {
  loading.value = true
  error.value = null
  try {
    const { data } = await adminService.listPlayers()
    players.value = data.players
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les joueurs.'
  } finally {
    loading.value = false
  }
}

function flash(message) {
  success.value = message
  setTimeout(() => (success.value = null), 3000)
}

// ─── ACTIONS ──────────────────────────────────────────────────────────────────

async function removeFromRoom(player) {
  if (busy[player.id]) return
  busy[player.id] = true
  error.value = null
  try {
    const { data } = await adminService.removePlayerFromRoom(player.id)
    player.room_code = null
    flash(data.message)
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de retirer le joueur du salon.'
  } finally {
    busy[player.id] = false
  }
}

// Validation manuelle : sans elle, pas d'historique ni d'emojis personnalisés.
async function toggleValidation(player) {
  if (busy[player.id] || player.admin_role) return
  busy[player.id] = true
  error.value = null
  try {
    const { data } = await adminService.setPlayerValidation(player.id, !player.is_validated)
    player.is_validated = data.player.is_validated
    player.validated_at = data.player.validated_at
    flash(data.message)
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de modifier la validation.'
  } finally {
    busy[player.id] = false
  }
}

// Administrateurs désignés : le propriétaire seul les nomme ou les retire
// (le serveur refuse les autres). Le profil doit avoir un mot de passe.
async function toggleAdmin(player) {
  if (busy[player.id] || !isAdminProfile.value || player.is_admin) return
  busy[player.id] = true
  error.value = null
  try {
    const { data } = await adminService.setPlayerAdmin(player.id, player.admin_role !== 'admin')
    player.admin_role = data.player.admin_role
    player.is_validated = data.player.is_validated
    flash(data.message)
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de modifier le rôle d\'administrateur.'
  } finally {
    busy[player.id] = false
  }
}

// Un administrateur désigné ne supprime pas un autre administrateur
const canDelete = (p) => isAdminProfile.value || !p.admin_role

function askDelete(player) {
  confirmTarget.value = player
  withGames.value = false
  deletePassword.value = ''
  showConfirm.value = true
}

async function confirmDelete() {
  const player = confirmTarget.value
  if (!player || deleting.value) return
  deleting.value = true
  error.value = null
  try {
    const { data } = await adminService.deletePlayer(player.id, withGames.value, deletePassword.value)
    deletePassword.value = ''
    players.value = players.value.filter((p) => p.id !== player.id)
    flash(data.message)
    showConfirm.value = false
    confirmTarget.value = null
  } catch (e) {
    error.value = e.response?.data?.message || 'La suppression a échoué.'
    // Mot de passe refusé : la fenêtre reste ouverte pour réessayer
    if (e.response?.status !== 403) showConfirm.value = false
  } finally {
    deleting.value = false
  }
}

function fmtDate(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', year: 'numeric' })
}

onMounted(loadPlayers)
</script>

<template>
  <BContainer class="admin-players py-4">
    <h1 class="page-title">Joueurs</h1>
    <p class="page-subtitle">
      Pseudos inscrits sur l'application : ceux qui ont joué au moins une partie
      et ceux présents dans un salon. Valide un profil pour lui ouvrir l'historique
      et les emojis personnalisés, retire un joueur de son salon ou supprime-le.
      <template v-if="isAdminProfile">Tu peux aussi désigner un administrateur parmi les profils sécurisés ;
      il ne pourra pas en désigner d'autres.</template>
    </p>

    <div class="toolbar">
      <input v-model="search" type="search" class="input" placeholder="Rechercher un pseudo" />
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="loadPlayers">
        <i class="fa-solid fa-rotate-right"></i> Rafraîchir
      </button>
    </div>

    <div class="filters">
      <button
        v-for="(label, key) in SORTS"
        :key="key"
        type="button"
        class="filter-chip"
        :class="{ active: sort === key }"
        @click="sort = key"
      >
        {{ label }}
      </button>
      <button type="button" class="filter-chip filter-chip--pending" :class="{ active: onlyPending }" @click="onlyPending = !onlyPending">
        À valider ({{ pendingCount }})
      </button>
    </div>

    <p v-if="error" class="alert-line alert-line--error">{{ error }}</p>
    <p v-if="success" class="alert-line alert-line--success">{{ success }}</p>

    <p v-if="loading" class="muted">Chargement…</p>
    <template v-else>
      <p class="muted count">
        {{ players.length }} joueur{{ players.length > 1 ? 's' : '' }}
        · {{ inRoomCount }} en salon
        <template v-if="search.trim()"> · {{ filteredPlayers.length }} résultat{{ filteredPlayers.length > 1 ? 's' : '' }}</template>
      </p>
      <p v-if="filteredPlayers.length === 0" class="muted">Aucun joueur.</p>

      <ul v-else class="player-list">
        <li v-for="p in filteredPlayers" :key="p.id" class="player-card">
          <div class="avatar">
            <PlayerAvatar :pseudo="p.pseudo" :url="p.avatar_url" :letters="1" />
          </div>
          <div class="player-main">
            <div class="player-name">
              {{ p.pseudo }}
              <span v-if="p.room_code" class="tag tag--room">salon {{ p.room_code }}</span>
              <span v-if="p.in_game" class="tag tag--game">en partie</span>
            </div>
            <div class="player-status">
              <span v-if="p.admin_role === 'owner'" class="tag tag--admin"><i class="fa-solid fa-crown"></i> propriétaire</span>
              <span v-else-if="p.admin_role === 'admin'" class="tag tag--admin"><i class="fa-solid fa-user-shield"></i> admin</span>
              <span v-if="p.has_password" class="tag tag--secure"><i class="fa-solid fa-lock"></i> sécurisé</span>
              <span v-else class="tag tag--public"><i class="fa-solid fa-lock-open"></i> public</span>
              <span v-if="p.is_validated" class="tag tag--validated"><i class="fa-solid fa-circle-check"></i> validé</span>
              <span v-else class="tag tag--pending">à valider</span>
            </div>
            <div class="player-meta">
              <span><i class="fa-solid fa-gamepad"></i> {{ p.games_count }} partie{{ p.games_count > 1 ? 's' : '' }}</span>
              <span><i class="fa-solid fa-trophy"></i> {{ p.wins }}</span>
              <span><i class="fa-solid fa-star"></i> {{ p.score }} pts</span>
              <span><i class="fa-solid fa-calendar"></i> {{ fmtDate(p.created_at) }}</span>
            </div>
          </div>
          <!-- Un joueur en pleine partie n'est ni retirable ni supprimable :
               supprimer ou terminer d'abord la partie (onglet Parties). -->
          <div class="player-actions">
            <button v-if="isAdminProfile && !p.is_admin" type="button" class="icon-btn"
                    :class="{ 'icon-btn--admin': p.admin_role === 'admin' }"
                    :disabled="busy[p.id]"
                    :title="p.admin_role === 'admin' ? 'Retirer le rôle d\'administrateur' : 'Désigner administrateur'"
                    :aria-label="p.admin_role === 'admin' ? 'Retirer le rôle d\'administrateur' : 'Désigner administrateur'"
                    :aria-pressed="p.admin_role === 'admin'"
                    @click="toggleAdmin(p)">
              <i class="fa-solid fa-user-shield"></i>
            </button>
            <button v-if="!p.admin_role" type="button" class="icon-btn"
                    :class="p.is_validated ? 'icon-btn--validated' : 'icon-btn--validate'"
                    :disabled="busy[p.id]"
                    :title="p.is_validated ? 'Retirer la validation' : 'Valider le profil'"
                    :aria-label="p.is_validated ? 'Retirer la validation' : 'Valider le profil'"
                    :aria-pressed="p.is_validated"
                    @click="toggleValidation(p)">
              <i class="fa-solid" :class="p.is_validated ? 'fa-user-check' : 'fa-user-clock'"></i>
            </button>
            <button v-if="p.room_code" type="button" class="icon-btn"
                    :disabled="busy[p.id] || p.in_game"
                    :title="p.in_game ? 'En pleine partie' : 'Retirer du salon'"
                    aria-label="Retirer du salon" @click="removeFromRoom(p)">
              <i class="fa-solid fa-right-from-bracket"></i>
            </button>
            <button v-if="canDelete(p)" type="button" class="icon-btn icon-btn--danger"
                    :disabled="busy[p.id] || p.in_game"
                    :title="p.in_game ? 'En pleine partie' : 'Supprimer le joueur'"
                    aria-label="Supprimer le joueur" @click="askDelete(p)">
              <i class="fa-solid fa-trash"></i>
            </button>
          </div>
        </li>
      </ul>
    </template>

    <BModal v-model="showConfirm" title="Supprimer le joueur" no-footer class="arcade-modal">
      <template v-if="confirmTarget">
        <p class="text-center">
          Supprimer <strong>{{ confirmTarget.pseudo }}</strong> ?
          <template v-if="confirmTarget.room_code">Ce joueur sera retiré du salon {{ confirmTarget.room_code }}.</template>
          Sa photo, ses emojis, ses messages et ses +1 sont effacés. Le pseudo redevient libre.
        </p>
        <template v-if="confirmTarget.games_count > 0">
          <p class="text-center mb-2">
            Ce joueur a joué <strong>{{ confirmTarget.games_count }} partie{{ confirmTarget.games_count > 1 ? 's' : '' }}</strong> :
            {{ confirmTarget.games_count > 1 ? 'elles restent' : 'elle reste' }} dans l'historique des autres joueurs, sous « Joueur supprimé ».
          </p>
          <label class="confirm-check">
            <input v-model="withGames" type="checkbox" />
            <span>
              Supprimer aussi {{ confirmTarget.games_count > 1 ? `ses ${confirmTarget.games_count} parties` : 'sa partie' }}
              pour <strong>tous</strong> les joueurs (historique et classement compris)
            </span>
          </label>
          <label v-if="withGames" class="delete-password">
            Mot de passe de suppression
            <input v-model="deletePassword" type="password" class="form-control" autocomplete="off" :disabled="deleting" />
          </label>
        </template>
        <p v-if="error && showConfirm" class="text-center text-danger small mb-0">{{ error }}</p>
      </template>
      <div class="text-center mt-3 btn-modal">
        <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm"
                :disabled="deleting || (withGames && !deletePassword)" @click="confirmDelete">
          <i class="fa-solid fa-trash"></i> {{ deleting ? 'Suppression…' : 'Supprimer' }}
        </button>
        <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" :disabled="deleting" @click="showConfirm = false">
          Annuler
        </button>
      </div>
    </BModal>
  </BContainer>
</template>

<style scoped>
/* Mot de passe de suppression : seconde clé, distincte de l'accès admin */
.delete-password {
  display: block;
  max-width: 280px;
  margin: 0.75rem auto 0;
  font-size: 0.85rem;
  text-align: left;
}
.delete-password .form-control {
  margin-top: 0.25rem;
}

.admin-players {
  max-width: 900px;
  color: var(--arcade-beige);
}

.page-title {
  font-family: 'Press Start 2P', cursive;
  font-size: 1rem;
  margin-bottom: 0.5rem;
}

.page-subtitle {
  color: var(--arcade-taupe);
  margin-bottom: 1.25rem;
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}

.input {
  flex: 1 1 180px;
  min-width: 0;
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 8px;
  color: var(--arcade-beige);
  padding: 0.4rem 0.6rem;
}

.input:focus {
  outline: none;
  border-color: var(--arcade-blue-grey);
}

.filters {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-bottom: 1rem;
}

.filter-chip {
  background: transparent;
  border: 2px solid var(--arcade-blue-grey);
  color: var(--arcade-taupe);
  border-radius: 999px;
  padding: 0.3rem 0.8rem;
  font-weight: 700;
  cursor: pointer;
}

.filter-chip.active {
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
}

.alert-line {
  border-radius: 8px;
  padding: 0.5rem 0.75rem;
  font-weight: 700;
}

.alert-line--error {
  background: var(--arcade-danger-dark);
  color: #fff;
}

.alert-line--success {
  background: var(--arcade-success-dark);
  color: #fff;
}

.muted {
  color: var(--arcade-taupe);
}

.count {
  font-size: 0.85rem;
}

.player-list {
  list-style: none;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(260px, 100%), 1fr));
  gap: 0.6rem;
}

.player-card {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-width: 0;
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.6rem 0.8rem;
}

.avatar {
  flex: none;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  overflow: hidden;
  display: grid;
  place-items: center;
  background: var(--arcade-taupe);
  color: #fff;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.9rem;
}

.player-main {
  flex: 1;
  min-width: 0;
  text-align: left;
}

.player-actions {
  flex: none;
  display: flex;
  gap: 0.15rem;
}

.icon-btn {
  width: 40px;
  height: 40px;
  display: grid;
  place-items: center;
  background: transparent;
  border: none;
  border-radius: 8px;
  color: var(--arcade-beige);
}

.icon-btn:hover:not(:disabled) {
  background: rgba(245, 245, 220, 0.15);
}

.icon-btn:disabled {
  opacity: 0.35;
}

.icon-btn--danger {
  color: var(--arcade-danger);
}

.confirm-check {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 0.5rem;
  font-weight: 700;
}

.player-name {
  font-weight: 700;
  overflow-wrap: anywhere;
}

.player-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.1rem 0.8rem;
  color: var(--arcade-taupe);
  font-size: 0.8rem;
}

.tag {
  font-size: 0.65rem;
  text-transform: uppercase;
  padding: 0.1rem 0.4rem;
  border-radius: 5px;
  margin-left: 0.3rem;
  white-space: nowrap;
}

.tag--room { background: var(--arcade-gold); color: var(--arcade-dark); }
.tag--game { background: var(--arcade-success); color: #fff; }

/* Statut du profil : sécurisé / public, validé / à valider */
.player-status {
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem;
  margin: 0.1rem 0 0.15rem;
}
.player-status .tag { margin-left: 0; }
.tag--secure { background: var(--arcade-blue-grey); color: var(--arcade-beige); }
.tag--public { background: transparent; color: var(--arcade-taupe); border: 1px solid var(--arcade-blue-grey); }
.tag--validated { background: var(--arcade-success); color: #fff; }
.tag--pending { background: var(--arcade-gold); color: var(--arcade-dark); }
.tag--admin { background: var(--arcade-dark); color: var(--arcade-gold); border: 1px solid var(--arcade-gold); }

.icon-btn--validate { color: var(--arcade-gold); }
.icon-btn--validated { color: var(--arcade-success); }
.icon-btn--admin { color: var(--arcade-gold); border-color: var(--arcade-gold); }

.filter-chip--pending.active {
  background: var(--arcade-gold);
  border-color: var(--arcade-gold);
  color: var(--arcade-dark);
}
</style>
