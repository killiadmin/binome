<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { BContainer } from 'bootstrap-vue-next'
import { historyService } from '../../services/historyService'
import PlayerAvatar from '../../components/player/PlayerAvatar.vue'

// ─── ONGLETS ──────────────────────────────────────────────────────────────────

const TABS = { games: 'Parties', leaderboard: 'Classement' }
const tab = ref('games')

// ─── PARTIES ──────────────────────────────────────────────────────────────────

const games = ref([])
const gamesLoading = ref(true)
const gamesError = ref(null)

const details = reactive({})
const detailLoading = reactive({})
const expandedGame = ref(null)

async function loadGames() {
  gamesLoading.value = true
  gamesError.value = null
  try {
    const { data } = await historyService.listGames()
    games.value = data.games ?? []
  } catch {
    gamesError.value = "Impossible de charger l'historique."
  } finally {
    gamesLoading.value = false
  }
}

async function toggleGame(game) {
  if (expandedGame.value === game.id) {
    expandedGame.value = null
    return
  }
  expandedGame.value = game.id

  if (details[game.id]) return

  detailLoading[game.id] = true
  try {
    const { data } = await historyService.getGame(game.id)
    details[game.id] = data
  } catch {
    gamesError.value = 'Impossible de charger le détail de cette partie.'
    expandedGame.value = null
  } finally {
    detailLoading[game.id] = false
  }
}

// ─── CLASSEMENT ───────────────────────────────────────────────────────────────

const leaderboard = ref([])
const boardLoading = ref(true)
const boardError = ref(null)
const search = ref('')

const breakdowns = reactive({})
const breakdownLoading = reactive({})
const expandedPlayer = ref(null)

const filteredBoard = computed(() => {
  const q = search.value.trim().toLowerCase()
  if (!q) return leaderboard.value
  return leaderboard.value.filter((row) => row.pseudo.toLowerCase().includes(q))
})

async function loadLeaderboard() {
  boardLoading.value = true
  boardError.value = null
  try {
    const { data } = await historyService.leaderboard()
    leaderboard.value = data.leaderboard ?? []
  } catch {
    boardError.value = 'Impossible de charger le classement.'
  } finally {
    boardLoading.value = false
  }
}

async function togglePlayer(row) {
  if (expandedPlayer.value === row.pseudo) {
    expandedPlayer.value = null
    return
  }
  expandedPlayer.value = row.pseudo

  if (breakdowns[row.pseudo]) return

  breakdownLoading[row.pseudo] = true
  try {
    const { data } = await historyService.player(row.pseudo)
    breakdowns[row.pseudo] = data
  } catch {
    boardError.value = 'Impossible de charger le détail de ce joueur.'
    expandedPlayer.value = null
  } finally {
    breakdownLoading[row.pseudo] = false
  }
}

// ─── HELPERS D'AFFICHAGE ──────────────────────────────────────────────────────

function fmtDate(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleString('fr-FR', {
    day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit',
  })
}

function fmtDay(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', year: '2-digit' })
}

const ANSWER_LABEL = { yes: 'Oui', no: 'Non', dont_know: 'Je ne sais pas' }

// Fin de ligne du journal, après « [photo] auteur → / accuse [photo] cible »
function actionTail(a) {
  if (a.type === 'question') {
    if (!a.is_valid) return `: question refusée (${a.refused_reason || 'mot interdit'})`
    const ans = a.answer ? ` — ${ANSWER_LABEL[a.answer] ?? a.answer}` : ''
    return `: « ${a.content} »${ans}`
  }
  const verdict = a.accusation_correct === true ? '✓ correcte'
      : a.accusation_correct === false ? '✗ fausse'
      : 'en attente'
  return `d'être « ${a.character_name ?? a.content} » — ${verdict}`
}

const MEDALS = { 1: '🥇', 2: '🥈', 3: '🥉' }

onMounted(() => {
  loadGames()
  loadLeaderboard()
})
</script>

<template>
  <BContainer class="history py-4">
    <h1 class="page-title">Historique</h1>
    <p class="page-subtitle">
      Toutes les parties terminées et le classement cumulé des joueurs.
      Les points d'un même pseudo s'additionnent d'une partie à l'autre.
    </p>

    <!-- Onglets -->
    <div class="tabs" role="tablist">
      <button
          v-for="(label, key) in TABS"
          :key="key"
          type="button"
          role="tab"
          class="tab"
          :class="{ 'is-active': tab === key }"
          :aria-selected="tab === key"
          @click="tab = key"
      >
        <i class="fa-solid" :class="key === 'games' ? 'fa-gamepad' : 'fa-ranking-star'"></i>
        {{ label }}
      </button>
    </div>

    <!-- ══════════ ONGLET PARTIES ══════════ -->
    <section v-if="tab === 'games'">
      <div class="toolbar">
        <span class="muted">{{ games.length }} partie{{ games.length > 1 ? 's' : '' }} terminée{{ games.length > 1 ? 's' : '' }}</span>
        <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="loadGames">
          <i class="fa-solid fa-rotate-right"></i> Rafraîchir
        </button>
      </div>

      <p v-if="gamesError" class="alert-line alert-line--error">{{ gamesError }}</p>
      <p v-if="gamesLoading" class="muted">Chargement…</p>
      <p v-else-if="games.length === 0" class="muted">Aucune partie terminée pour le moment.</p>

      <ul v-else class="hist-list">
        <li v-for="game in games" :key="game.id" class="hist-card">
          <button type="button" class="hist-card__head" :aria-expanded="expandedGame === game.id" @click="toggleGame(game)">
            <div class="hist-card__main">
              <span class="hist-card__title">Partie #{{ game.id }}</span>
              <span class="muted hist-card__sub">salon {{ game.room_code || '—' }} · {{ fmtDate(game.created_at) }}</span>
            </div>
            <i class="fa-solid chev" :class="expandedGame === game.id ? 'fa-chevron-up' : 'fa-chevron-down'"></i>
          </button>

          <div class="meta">
            <span><i class="fa-solid fa-users"></i> {{ game.players_count }} joueurs</span>
            <span><i class="fa-solid fa-user-group"></i> {{ game.binomes_discovered_count }}/{{ game.binomes_count }} découverts</span>
            <span><i class="fa-solid fa-hourglass-half"></i> {{ game.rounds_count }} rounds</span>
          </div>

          <p v-if="game.winners.length" class="winners">
            <i class="fa-solid fa-trophy"></i>
            {{ game.winners.join(', ') }}
          </p>

          <!-- Détail -->
          <div v-if="expandedGame === game.id" class="detail">
            <p v-if="detailLoading[game.id]" class="muted">Chargement du détail…</p>
            <template v-else-if="details[game.id]">
              <section v-for="binome in details[game.id].binomes" :key="binome.id" class="block">
                <h3 class="block-title">
                  {{ binome.universe }}
                  <span v-if="binome.cosmos" class="cosmos">· {{ binome.cosmos }}</span>
                  <span v-if="binome.is_discovered" class="tag tag--discovered">découvert</span>
                  <span v-if="binome.is_orphan" class="tag tag--orphan">orphelin</span>
                </h3>
                <div class="players">
                  <div v-for="p in binome.players" :key="p.id" class="player">
                    <div class="player-name">
                      <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="p.pseudo" :url="p.avatar_url" :letters="1" /></span>{{ p.pseudo }}
                      <span v-if="p.is_eliminated" class="tag tag--eliminated">éliminé</span>
                    </div>
                    <div v-if="p.character" class="character">
                      <strong>{{ p.character.name }}</strong>
                      <span v-if="p.character.forbidden_words?.length" class="forbidden">
                        mots interdits : {{ p.character.forbidden_words.join(', ') }}
                      </span>
                    </div>
                    <div v-else class="character muted">personnage inconnu</div>
                  </div>
                </div>
              </section>

              <section v-if="details[game.id].game_stats.length" class="block">
                <h3 class="block-title">Scores de la partie</h3>
                <ul class="plain-list">
                  <li v-for="(s, i) in details[game.id].game_stats" :key="i">
                    <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="s.player_pseudo" :url="s.avatar_url" :letters="1" /></span><strong>{{ s.player_pseudo }}</strong> ({{ s.character_name }}) —
                    {{ s.score }} pts, {{ s.eliminations }} élim., {{ s.rounds_survived }} rounds
                    <span v-if="s.is_winner" class="tag tag--discovered">gagnant</span>
                  </li>
                </ul>
              </section>

              <section v-if="details[game.id].rounds.length" class="block">
                <h3 class="block-title">Journal</h3>
                <div v-for="round in details[game.id].rounds" :key="round.id" class="round">
                  <div class="round-label">Round {{ round.number }}</div>
                  <ul class="plain-list">
                    <li v-for="a in round.actions" :key="a.id">
                      <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="a.player?.pseudo" :url="a.player?.avatar_url" :letters="1" /></span><strong>{{ a.player?.pseudo ?? '?' }}</strong>
                      {{ a.type === 'question' ? '→' : 'accuse' }}
                      <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="a.target_player?.pseudo" :url="a.target_player?.avatar_url" :letters="1" /></span><strong>{{ a.target_player?.pseudo ?? '?' }}</strong>
                      {{ actionTail(a) }}
                    </li>
                    <li v-if="round.actions.length === 0" class="muted">aucune action</li>
                  </ul>
                </div>
              </section>
            </template>
          </div>
        </li>
      </ul>
    </section>

    <!-- ══════════ ONGLET CLASSEMENT ══════════ -->
    <section v-else>
      <div class="toolbar">
        <input
            v-model="search"
            type="search"
            class="search"
            placeholder="Chercher un pseudo…"
            aria-label="Chercher un joueur"
        />
        <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="loadLeaderboard">
          <i class="fa-solid fa-rotate-right"></i>
        </button>
      </div>

      <p v-if="boardError" class="alert-line alert-line--error">{{ boardError }}</p>
      <p v-if="boardLoading" class="muted">Chargement…</p>
      <p v-else-if="leaderboard.length === 0" class="muted">Aucun score enregistré pour le moment.</p>
      <p v-else-if="filteredBoard.length === 0" class="muted">Aucun joueur ne correspond à « {{ search }} ».</p>

      <ul v-else class="hist-list">
        <li v-for="row in filteredBoard" :key="row.pseudo" class="hist-card" :class="{ 'hist-card--podium': row.rank <= 3 }">
          <button type="button" class="hist-card__head" :aria-expanded="expandedPlayer === row.pseudo" @click="togglePlayer(row)">
            <span class="rank">{{ MEDALS[row.rank] || `#${row.rank}` }}</span>
            <span class="mini-avatar mini-avatar--lg" aria-hidden="true"><PlayerAvatar :pseudo="row.pseudo" :url="row.avatar_url" :letters="1" /></span>
            <div class="hist-card__main">
              <span class="hist-card__title">{{ row.pseudo }}</span>
              <span class="muted hist-card__sub">
                {{ row.games_played }} partie{{ row.games_played > 1 ? 's' : '' }} ·
                {{ row.wins }} victoire{{ row.wins > 1 ? 's' : '' }} ·
                {{ row.average_score }} pts/partie
              </span>
            </div>
            <span class="total">{{ row.total_score }}<small>pts</small></span>
            <i class="fa-solid chev" :class="expandedPlayer === row.pseudo ? 'fa-chevron-up' : 'fa-chevron-down'"></i>
          </button>

          <!-- Détail : le cumul, redécoupé partie par partie -->
          <div v-if="expandedPlayer === row.pseudo" class="detail">
            <p v-if="breakdownLoading[row.pseudo]" class="muted">Chargement…</p>
            <template v-else-if="breakdowns[row.pseudo]">
              <h3 class="block-title">Détail par partie</h3>
              <div class="table-scroll">
                <table class="split">
                  <thead>
                    <tr>
                      <th>Partie</th><th>Personnage</th><th class="num">Pts</th>
                      <th class="num">Élim.</th><th class="num">Rounds</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="g in breakdowns[row.pseudo].games" :key="g.game_id">
                      <td>
                        <span class="nowrap">#{{ g.game_id }}</span>
                        <span v-if="g.is_winner" class="tag tag--discovered">gagné</span>
                        <div class="muted nowrap">{{ fmtDay(g.played_at) }}</div>
                      </td>
                      <td>{{ g.character_name }}</td>
                      <td class="num">{{ g.score }}</td>
                      <td class="num">{{ g.eliminations }}</td>
                      <td class="num">{{ g.rounds_survived }}</td>
                    </tr>
                  </tbody>
                  <tfoot>
                    <tr>
                      <td colspan="2"><strong>Total cumulé</strong></td>
                      <td class="num"><strong>{{ breakdowns[row.pseudo].totals.total_score }}</strong></td>
                      <td class="num"><strong>{{ breakdowns[row.pseudo].totals.eliminations }}</strong></td>
                      <td class="num"><strong>{{ breakdowns[row.pseudo].totals.rounds_survived }}</strong></td>
                    </tr>
                  </tfoot>
                </table>
              </div>
            </template>
          </div>
        </li>
      </ul>
    </section>
  </BContainer>
</template>

<style scoped>
/* Mobile first : tout est calé sur 320 px, les media queries n'élargissent
   qu'ensuite. Aucun élément ne doit pouvoir dépasser la largeur du conteneur. */
.history {
  max-width: 900px;
  color: var(--arcade-beige);
  text-align: left; /* #app impose text-align: center à toute la page */
}

.page-title {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.95rem;
  margin-bottom: 0.6rem;
}

.page-subtitle {
  color: var(--arcade-taupe);
  font-size: 0.85rem;
  margin-bottom: 1.1rem;
}

/* ── Onglets ──────────────────────────────────────────────────── */
.tabs {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1rem;
}

.tab {
  flex: 1 1 0;
  min-width: 0;
  padding: 0.5rem 0.6rem;
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 10px;
  background: transparent;
  color: var(--arcade-taupe);
  font-family: inherit;
  font-weight: 700;
  font-size: 0.85rem;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.tab.is-active {
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
  border-color: var(--arcade-blue-grey);
}

/* ── Barre d'outils ───────────────────────────────────────────── */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  margin-bottom: 0.8rem;
}

.search {
  flex: 1 1 auto;
  min-width: 0;
  padding: 0.45rem 0.7rem;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 10px;
  background: #fff;
  /* style.css déclare `color-scheme: light dark` : sans couleur explicite, le
     texte saisi devient blanc sur blanc quand l'OS est en thème sombre. */
  color: var(--arcade-dark);
  color-scheme: light;
  font-family: inherit;
  font-size: 16px; /* < 16px déclenche le zoom automatique de Safari iOS */
}

.search::placeholder { color: var(--arcade-taupe); opacity: 1; }

.muted { color: var(--arcade-taupe); font-size: 0.85rem; }

.alert-line {
  border-radius: 8px;
  padding: 0.5rem 0.75rem;
  font-weight: 700;
  background: var(--arcade-danger-dark);
  color: #fff;
}

/* ── Cartes ───────────────────────────────────────────────────── */
.hist-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.hist-card {
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.7rem 0.8rem;
  overflow: hidden;
}

.hist-card--podium { border-color: var(--arcade-gold); }

.hist-card__head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  width: 100%;
  padding: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.hist-card__head:hover { border-color: transparent; }

.hist-card__main {
  /* Sans min-width: 0, un enfant flex refuse de rétrécir sous son contenu
     et pousse la carte hors de l'écran. */
  min-width: 0;
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
}

.hist-card__title {
  font-weight: 800;
  overflow-wrap: anywhere;
}

.hist-card__sub { font-size: 0.75rem; }

.chev { flex: 0 0 auto; color: var(--arcade-taupe); }

.rank {
  flex: 0 0 auto;
  font-size: 1.1rem;
  font-weight: 800;
  min-width: 2rem;
}

.total {
  flex: 0 0 auto;
  font-weight: 800;
  font-size: 1.05rem;
  color: var(--arcade-gold);
}

.total small { font-size: 0.6rem; margin-left: 0.15rem; }

.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.2rem 0.8rem;
  margin-top: 0.4rem;
  color: var(--arcade-taupe);
  font-size: 0.78rem;
}

.winners {
  margin: 0.35rem 0 0;
  font-size: 0.8rem;
  color: var(--arcade-gold);
  overflow-wrap: anywhere;
}

/* ── Détail déplié ────────────────────────────────────────────── */
.detail {
  margin-top: 0.8rem;
  padding-top: 0.8rem;
  border-top: 1px dashed var(--arcade-blue-grey-dark);
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.block { min-width: 0; }

.block-title {
  font-size: 0.9rem;
  font-weight: 700;
  margin-bottom: 0.35rem;
  overflow-wrap: anywhere;
}

.cosmos { color: var(--arcade-taupe); font-weight: 400; }

.players {
  display: grid;
  grid-template-columns: 1fr;
  gap: 0.4rem;
}

.player {
  background: rgba(0, 0, 0, 0.25);
  border-radius: 8px;
  padding: 0.45rem 0.6rem;
  min-width: 0;
}

.player-name { font-weight: 700; overflow-wrap: anywhere; }

/* Photo (ou initiales) devant un pseudo — voir PlayerAvatar */
.mini-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 1.5rem;
  height: 1.5rem;
  margin-right: 0.3rem;
  vertical-align: middle;
  border-radius: 50%;
  overflow: hidden;
  background: var(--arcade-taupe);
  color: #fff;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.5rem;
  line-height: 1;
}

.mini-avatar--lg {
  width: 2.2rem;
  height: 2.2rem;
  margin-right: 0;
  font-size: 0.7rem;
}

.character {
  font-size: 0.82rem;
  display: flex;
  flex-direction: column;
  overflow-wrap: anywhere;
}

.forbidden { color: var(--arcade-taupe); }

.tag {
  display: inline-block;
  font-size: 0.62rem;
  text-transform: uppercase;
  padding: 0.1rem 0.4rem;
  border-radius: 5px;
  margin-left: 0.35rem;
  white-space: nowrap;
}

.tag--discovered { background: var(--arcade-success); color: #fff; }
.tag--eliminated { background: var(--arcade-danger); color: #fff; }
.tag--orphan { background: var(--arcade-blue-grey); color: #fff; }

.plain-list {
  margin: 0.2rem 0 0;
  padding-left: 1.1rem;
  font-size: 0.82rem;
}

.plain-list li { margin-bottom: 0.15rem; overflow-wrap: anywhere; }

.round { margin-bottom: 0.5rem; }
.round-label { font-weight: 700; font-size: 0.82rem; }

/* ── Tableau du split par partie ──────────────────────────────── */
.table-scroll {
  /* Le tableau défile horizontalement DANS la carte plutôt que d'élargir
     la page — c'est le seul endroit où le contenu peut être plus large.
     `min-width: 0` est indispensable : en tant qu'enfant flex de .detail, sa
     largeur min-content (celle du tableau) remonterait sinon jusqu'au
     conteneur et élargirait toute la page sur mobile. */
  min-width: 0;
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
}

.split {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.8rem;
}

.split th,
.split td {
  padding: 0.3rem 0.45rem;
  border-bottom: 1px solid var(--arcade-blue-grey-dark);
  text-align: left;
}

.split thead th {
  color: var(--arcade-taupe);
  font-size: 0.7rem;
  text-transform: uppercase;
  /* Pas de nowrap : les en-têtes qui se replient gardent la largeur
     min-content du tableau basse, donc sous celle de l'écran. */
}

.split .num { text-align: right; }
.split tfoot td { border-bottom: 0; border-top: 2px solid var(--arcade-blue-grey); }
.nowrap { white-space: nowrap; }

/* ── À partir du format tablette ──────────────────────────────── */
@media (min-width: 576px) {
  .page-title { font-size: 1rem; }
  .hist-card { padding: 0.9rem 1rem; }
  .players { grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr)); }
  .tab { flex: 0 0 auto; padding: 0.5rem 1.2rem; }
}
</style>
