<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { BContainer, BModal } from 'bootstrap-vue-next'
import { adminService } from '../../services/adminService'

// ─── STATE ────────────────────────────────────────────────────────────────────

const games = ref([])
const loading = ref(true)
const error = ref(null)
const success = ref(null)

const statusFilter = reactive({ in_progress: true, finished: true, waiting: true })

const STATUS_LABEL = {
  in_progress: 'En cours',
  finished: 'Terminée',
  waiting: 'En attente',
}

// Détail chargé à la demande, indexé par game id.
const details = reactive({})
const detailLoading = reactive({})
const expanded = ref(null)

// Suppression
const showConfirm = ref(false)
const confirmTarget = ref(null)
const deleting = ref(false)

const filteredGames = computed(() =>
  games.value.filter((g) => statusFilter[g.status] ?? true)
)

// ─── CHARGEMENT ───────────────────────────────────────────────────────────────

async function loadGames() {
  loading.value = true
  error.value = null
  try {
    const { data } = await adminService.listGames()
    games.value = data.games
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les parties.'
  } finally {
    loading.value = false
  }
}

async function toggleDetail(game) {
  if (expanded.value === game.id) {
    expanded.value = null
    return
  }
  expanded.value = game.id
  if (details[game.id] || detailLoading[game.id]) return

  detailLoading[game.id] = true
  try {
    const { data } = await adminService.getGame(game.id)
    details[game.id] = data
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger le détail de la partie.'
    expanded.value = null
  } finally {
    detailLoading[game.id] = false
  }
}

// ─── SUPPRESSION ──────────────────────────────────────────────────────────────

function askDelete(game) {
  confirmTarget.value = game
  showConfirm.value = true
}

async function confirmDelete() {
  if (!confirmTarget.value || deleting.value) return
  deleting.value = true
  error.value = null
  const id = confirmTarget.value.id
  try {
    await adminService.deleteGame(id)
    games.value = games.value.filter((g) => g.id !== id)
    delete details[id]
    if (expanded.value === id) expanded.value = null
    success.value = `Partie #${id} supprimée.`
    setTimeout(() => (success.value = null), 3000)
    showConfirm.value = false
    confirmTarget.value = null
  } catch (e) {
    error.value = e.response?.data?.message || 'La suppression a échoué.'
  } finally {
    deleting.value = false
  }
}

// ─── HELPERS D'AFFICHAGE ──────────────────────────────────────────────────────

function fmtDate(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleString('fr-FR', {
    day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit',
  })
}

const ANSWER_LABEL = { yes: 'Oui', no: 'Non', dont_know: 'Je ne sais pas' }

function actionLine(a) {
  const who = a.player?.pseudo ?? '?'
  const target = a.target_player?.pseudo ?? '?'
  if (a.type === 'question') {
    if (!a.is_valid) return `${who} → ${target} : question refusée (${a.refused_reason || 'mot interdit'})`
    const ans = a.answer ? ` — ${ANSWER_LABEL[a.answer] ?? a.answer}` : ''
    return `${who} → ${target} : « ${a.content} »${ans}`
  }
  const verdict = a.accusation_correct === true ? '✓ correcte' : a.accusation_correct === false ? '✗ fausse' : 'en attente'
  return `${who} accuse ${target} d'être « ${a.character_name ?? a.content} » — ${verdict}`
}

onMounted(loadGames)
</script>

<template>
  <BContainer class="admin-games py-4">
    <h1 class="page-title">Parties</h1>
    <p class="page-subtitle">
      Consulter et supprimer les parties enregistrées. Supprimer une partie libère
      les personnages qu'elle utilisait.
    </p>

    <div class="filters">
      <button
        v-for="(label, key) in STATUS_LABEL"
        :key="key"
        type="button"
        class="filter-chip"
        :class="{ active: statusFilter[key] }"
        @click="statusFilter[key] = !statusFilter[key]"
      >
        {{ label }}
      </button>
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm ms-auto" @click="loadGames">
        <i class="fa-solid fa-rotate-right"></i> Rafraîchir
      </button>
    </div>

    <p v-if="error" class="alert-line alert-line--error">{{ error }}</p>
    <p v-if="success" class="alert-line alert-line--success">{{ success }}</p>

    <p v-if="loading" class="muted">Chargement…</p>
    <p v-else-if="filteredGames.length === 0" class="muted">Aucune partie.</p>

    <ul v-else class="game-list">
      <li v-for="game in filteredGames" :key="game.id" class="game-card">
        <div class="game-row">
          <div class="game-head">
            <span class="badge" :class="`badge--${game.status}`">{{ STATUS_LABEL[game.status] || game.status }}</span>
            <span class="game-code">#{{ game.id }} · salon {{ game.room_code || '—' }}</span>
          </div>
          <div class="game-meta">
            <span><i class="fa-solid fa-users"></i> {{ game.players_count }}</span>
            <span><i class="fa-solid fa-user-group"></i> {{ game.binomes_discovered_count }}/{{ game.binomes_count }} découverts</span>
            <span><i class="fa-solid fa-hourglass-half"></i> {{ game.rounds_count }} rounds</span>
            <span><i class="fa-solid fa-clock"></i> {{ fmtDate(game.created_at) }}</span>
          </div>
          <div class="game-actions">
            <button type="button" class="cabinet-btn cabinet-btn--sm" @click="toggleDetail(game)">
              <i class="fa-solid" :class="expanded === game.id ? 'fa-chevron-up' : 'fa-eye'"></i>
              {{ expanded === game.id ? 'Masquer' : 'Voir' }}
            </button>
            <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" @click="askDelete(game)">
              <i class="fa-solid fa-trash"></i> Supprimer
            </button>
          </div>
        </div>

        <div v-if="expanded === game.id" class="game-detail">
          <p v-if="detailLoading[game.id]" class="muted">Chargement du détail…</p>
          <template v-else-if="details[game.id]">
            <section
              v-for="binome in details[game.id].binomes"
              :key="binome.id"
              class="binome"
            >
              <h3 class="binome-title">
                {{ binome.universe }}
                <span v-if="binome.cosmos" class="cosmos">· {{ binome.cosmos }}</span>
                <span v-if="binome.is_discovered" class="tag tag--discovered">découvert</span>
              </h3>
              <div class="players">
                <div v-for="p in binome.players" :key="p.id" class="player">
                  <div class="player-name">
                    {{ p.pseudo }}
                    <span v-if="p.is_eliminated" class="tag tag--eliminated">éliminé</span>
                  </div>
                  <div v-if="p.character" class="character">
                    <strong>{{ p.character.name }}</strong>
                    <span class="forbidden">
                      mots interdits :
                      <em v-for="(w, i) in p.character.forbidden_words" :key="i">{{ w }}<span v-if="i < p.character.forbidden_words.length - 1">, </span></em>
                    </span>
                  </div>
                  <div v-else class="character muted">personnage inconnu</div>
                </div>
              </div>
            </section>

            <section v-if="details[game.id].rounds.length" class="rounds">
              <h3 class="binome-title">Journal</h3>
              <div v-for="round in details[game.id].rounds" :key="round.id" class="round">
                <div class="round-label">
                  Round {{ round.number }}
                  <span v-if="round.is_finished" class="muted">(terminé)</span>
                </div>
                <ul class="actions">
                  <li v-for="a in round.actions" :key="a.id">{{ actionLine(a) }}</li>
                  <li v-if="round.actions.length === 0" class="muted">aucune action</li>
                </ul>
              </div>
            </section>

            <section v-if="details[game.id].game_stats.length" class="stats">
              <h3 class="binome-title">Statistiques</h3>
              <ul class="actions">
                <li v-for="(s, i) in details[game.id].game_stats" :key="i">
                  {{ s.player_pseudo }} ({{ s.character_name }}) — score {{ s.score }}, {{ s.eliminations }} élim.
                  <span v-if="s.is_winner" class="tag tag--discovered">gagnant</span>
                </li>
              </ul>
            </section>
          </template>
        </div>
      </li>
    </ul>

    <BModal v-model="showConfirm" title="Supprimer la partie" no-footer class="arcade-modal">
      <p v-if="confirmTarget" class="text-center">
        Supprimer la partie <strong>#{{ confirmTarget.id }}</strong> (salon
        {{ confirmTarget.room_code || '—' }}) ? Les binomes, rounds, actions et
        statistiques associés seront supprimés. Cette action est irréversible.
      </p>
      <div class="text-center mt-3 btn-modal">
        <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" :disabled="deleting" @click="confirmDelete">
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
.admin-games {
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

.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
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

.game-list {
  list-style: none;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.game-card {
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.9rem 1rem;
}

.game-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.6rem 1rem;
}

.game-head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  min-width: 200px;
}

.game-code {
  font-weight: 700;
}

.badge {
  font-size: 0.7rem;
  font-weight: 700;
  text-transform: uppercase;
  padding: 0.2rem 0.5rem;
  border-radius: 6px;
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
}

.badge--in_progress { background: var(--arcade-gold); color: var(--arcade-dark); }
.badge--finished { background: var(--arcade-success); }
.badge--waiting { background: var(--arcade-taupe); color: var(--arcade-dark); }

.game-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem 1rem;
  color: var(--arcade-taupe);
  font-size: 0.85rem;
  flex: 1;
}

.game-actions {
  display: flex;
  gap: 0.5rem;
}

.game-detail {
  margin-top: 0.9rem;
  padding-top: 0.9rem;
  border-top: 1px dashed var(--arcade-blue-grey-dark);
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.binome-title {
  font-size: 0.95rem;
  font-weight: 700;
  margin-bottom: 0.4rem;
}

.cosmos { color: var(--arcade-taupe); font-weight: 400; }

.players {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 0.5rem;
}

.player {
  background: rgba(0, 0, 0, 0.25);
  border-radius: 8px;
  padding: 0.5rem 0.7rem;
}

.player-name { font-weight: 700; }

.character { font-size: 0.85rem; display: flex; flex-direction: column; }

.forbidden { color: var(--arcade-taupe); }

.tag {
  font-size: 0.65rem;
  text-transform: uppercase;
  padding: 0.1rem 0.4rem;
  border-radius: 5px;
  margin-left: 0.4rem;
}

.tag--discovered { background: var(--arcade-success); color: #fff; }
.tag--eliminated { background: var(--arcade-danger); color: #fff; }

.round { margin-bottom: 0.6rem; }
.round-label { font-weight: 700; font-size: 0.85rem; }

.actions {
  margin: 0.2rem 0 0;
  padding-left: 1.2rem;
  font-size: 0.85rem;
}

.actions li { margin-bottom: 0.15rem; }
</style>
