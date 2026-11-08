<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { BContainer, BModal } from 'bootstrap-vue-next'
import { adminService } from '../../services/adminService'
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
const deleting = ref(false)

const search = ref('')
const sort = ref('pseudo')

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
  const list = q ? players.value.filter((p) => norm(p.pseudo).includes(q)) : [...players.value]

  const by = {
    pseudo: (a, b) => a.pseudo.localeCompare(b.pseudo, 'fr'),
    games: (a, b) => b.games_count - a.games_count,
    wins: (a, b) => b.wins - a.wins || b.score - a.score,
    recent: (a, b) => (b.created_at ?? '').localeCompare(a.created_at ?? ''),
  }
  return list.sort(by[sort.value])
})

const inRoomCount = computed(() => players.value.filter((p) => p.room_code).length)

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

function askDelete(player) {
  confirmTarget.value = player
  withGames.value = false
  showConfirm.value = true
}

async function confirmDelete() {
  const player = confirmTarget.value
  if (!player || deleting.value) return
  deleting.value = true
  error.value = null
  try {
    const { data } = await adminService.deletePlayer(player.id, withGames.value)
    players.value = players.value.filter((p) => p.id !== player.id)
    flash(data.message)
    showConfirm.value = false
    confirmTarget.value = null
  } catch (e) {
    error.value = e.response?.data?.message || 'La suppression a échoué.'
    showConfirm.value = false
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
      et ceux présents dans un salon. Retire un joueur de son salon ou supprime-le.
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
            <button v-if="p.room_code" type="button" class="icon-btn"
                    :disabled="busy[p.id] || p.in_game"
                    :title="p.in_game ? 'En pleine partie' : 'Retirer du salon'"
                    aria-label="Retirer du salon" @click="removeFromRoom(p)">
              <i class="fa-solid fa-right-from-bracket"></i>
            </button>
            <button type="button" class="icon-btn icon-btn--danger"
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
          Sa photo de profil est supprimée, ses emojis restent. Le pseudo redevient libre.
        </p>
        <template v-if="confirmTarget.games_count > 0">
          <p class="text-center mb-2">
            Ce joueur a joué <strong>{{ confirmTarget.games_count }} partie{{ confirmTarget.games_count > 1 ? 's' : '' }}</strong> :
            elles y font référence et doivent être supprimées en même temps — pour
            <strong>tous</strong> leurs joueurs (historique et classement compris).
          </p>
          <label class="confirm-check">
            <input v-model="withGames" type="checkbox" />
            Supprimer aussi ses {{ confirmTarget.games_count }} partie{{ confirmTarget.games_count > 1 ? 's' : '' }}
          </label>
        </template>
      </template>
      <div class="text-center mt-3 btn-modal">
        <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm"
                :disabled="deleting || (confirmTarget?.games_count > 0 && !withGames)" @click="confirmDelete">
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
</style>
