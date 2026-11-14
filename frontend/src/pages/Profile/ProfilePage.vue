<script setup>
import {ref, computed, onMounted} from 'vue'
import {useRouter} from 'vue-router'
import {BFormInput} from 'bootstrap-vue-next'
import {profileService} from '../../services/profileService'
import {logout, setProfile, updateProfilePlayer, profilePlayer} from '../../services/profile'
import {resumeFromProfile} from '../../services/gameSession'
import {resetEcho} from '../../sockets/useReverb'
import PlayerAvatar from '../../components/player/PlayerAvatar.vue'
import AvatarEditor from '../../components/player/AvatarEditor.vue'

// Page du profil connecté : photo, pseudo, mot de passe, partie en cours et
// parties jouées. Accessible seulement connecté (garde du routeur).

const router = useRouter()

const loading = ref(true)
const loadError = ref(null)
const current = ref(null)
const totals = ref(null)
const games = ref([])

const player = computed(() => profilePlayer.value)

async function load() {
  loading.value = true
  loadError.value = null
  try {
    const {data} = await profileService.show()
    updateProfilePlayer(data.player)
    current.value = data.current
    totals.value = data.totals
    games.value = data.games ?? []
  } catch (e) {
    if (e.response?.status === 401) {
      // Jeton révoqué (mot de passe posé depuis un autre appareil) ou profil supprimé
      logout()
      router.replace({name: 'Login', query: {pseudo: player.value?.pseudo}})
      return
    }
    loadError.value = 'Impossible de charger ton profil.'
  } finally {
    loading.value = false
  }
}

onMounted(load)

// ─── Partie / salon en cours ──────────────────────────────────────────────────
function resume() {
  const target = resumeFromProfile(player.value.id, current.value)
  if (!target) return
  resetEcho()
  router.push(target)
}

// ─── Photo ────────────────────────────────────────────────────────────────────
const showAvatarEditor = ref(false)

// ─── Pseudo ───────────────────────────────────────────────────────────────────
const editingPseudo = ref(false)
const newPseudo = ref('')
const pseudoError = ref(null)
const pseudoSaving = ref(false)

function startRename() {
  newPseudo.value = player.value.pseudo
  pseudoError.value = null
  editingPseudo.value = true
}

async function saveRename() {
  pseudoError.value = null
  pseudoSaving.value = true
  try {
    const {data} = await profileService.rename(newPseudo.value.trim())
    setProfile(data.token, data.player)
    editingPseudo.value = false
  } catch (e) {
    pseudoError.value = e.response?.data?.errors?.pseudo?.[0] || e.response?.data?.message || "Le pseudo n'a pas pu être changé."
  } finally {
    pseudoSaving.value = false
  }
}

// ─── Mot de passe ─────────────────────────────────────────────────────────────
const pwd = ref({current: '', next: '', confirm: ''})
const pwdMode = ref(null) // null | 'set' | 'remove'
const pwdError = ref(null)
const pwdMessage = ref(null)
const pwdSaving = ref(false)

function openPwd(mode) {
  pwd.value = {current: '', next: '', confirm: ''}
  pwdError.value = null
  pwdMessage.value = null
  pwdMode.value = mode
}

function firstError(e) {
  const errors = e.response?.data?.errors
  if (errors) return Object.values(errors)[0]?.[0]
  if (e.response?.status === 429) return 'Trop de tentatives : réessaie dans une minute.'
  return e.response?.data?.message || 'Opération impossible.'
}

async function savePassword() {
  pwdError.value = null
  if (pwdMode.value === 'set') {
    if (pwd.value.next.length < 6) return (pwdError.value = 'Le mot de passe doit faire au moins 6 caractères.')
    if (pwd.value.next !== pwd.value.confirm) return (pwdError.value = 'Les deux mots de passe ne correspondent pas.')
  }
  pwdSaving.value = true
  try {
    const {data} = pwdMode.value === 'set'
        ? await profileService.setPassword(pwd.value.next, pwd.value.current)
        : await profileService.removePassword(pwd.value.current)
    setProfile(data.token, data.player)
    pwdMessage.value = data.message
    pwdMode.value = null
  } catch (e) {
    pwdError.value = firstError(e)
  } finally {
    pwdSaving.value = false
  }
}

// ─── Déconnexion ──────────────────────────────────────────────────────────────
function handleLogout() {
  logout()
  router.push({name: 'Home'})
}

// ─── Affichage ────────────────────────────────────────────────────────────────
function fmtDay(iso) {
  if (!iso) return ''
  return new Date(iso).toLocaleDateString('fr-FR', {day: 'numeric', month: 'short', year: 'numeric'})
}

function gameStatus(g) {
  if (g.status === 'finished') return g.is_winner ? {label: 'Victoire', cls: 'pixel-chip--gold'} : {label: 'Terminée', cls: ''}
  return {label: 'En cours', cls: 'pixel-chip--danger'}
}
</script>

<template>
  <div class="arcade-page arcade-bg">
    <div class="profile-wrap">
      <div class="arcade-title-wrap">
        <h1 class="arcade-title">Profil</h1>
      </div>

      <div v-if="loadError" class="arcade-alert mb-3" role="alert">
        <i class="fa-solid fa-triangle-exclamation"></i> {{ loadError }}
      </div>

      <!-- Identité -->
      <section v-if="player" class="board-card profile-card">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>

        <div class="identity">
          <button type="button" class="identity__avatar" aria-label="Changer ma photo" @click="showAvatarEditor = true">
            <PlayerAvatar :pseudo="player.pseudo" :url="player.avatar_url" :letters="2" />
            <span class="identity__camera"><i class="fa-solid fa-camera"></i></span>
          </button>

          <div class="identity__body">
            <template v-if="!editingPseudo">
              <div class="identity__pseudo">
                <span class="identity__name">{{ player.pseudo }}</span>
                <button
                    v-if="!player.is_admin"
                    type="button"
                    class="icon-btn"
                    aria-label="Changer de pseudo"
                    title="Changer de pseudo"
                    @click="startRename"
                >
                  <i class="fa-solid fa-pen"></i>
                </button>
              </div>
              <div class="identity__chips">
                <span v-if="player.is_admin" class="pixel-chip pixel-chip--gold">Admin</span>
                <template v-else>
                  <span v-if="player.admin_role === 'admin'" class="pixel-chip pixel-chip--gold" title="Administrateur désigné">Admin</span>
                  <span v-if="player.has_password" class="pixel-chip">🔒 Protégé</span>
                  <span v-else class="pixel-chip pixel-chip--danger">🔓 Public</span>
                </template>
                <template v-if="!player.admin_role">
                  <span v-if="player.is_validated" class="pixel-chip" title="Historique et emojis personnalisés ouverts">✔ Validé</span>
                  <span v-else class="pixel-chip" title="L'administrateur doit valider ton profil pour ouvrir l'historique et les emojis personnalisés">⏳ En attente de validation</span>
                </template>
              </div>
            </template>
            <form v-else class="rename" @submit.prevent="saveRename">
              <BFormInput v-model="newPseudo" maxlength="20" autocomplete="username" aria-label="Nouveau pseudo" />
              <div class="rename__actions">
                <button type="submit" class="cabinet-btn cabinet-btn--sm" :disabled="pseudoSaving || newPseudo.trim().length < 2">
                  Enregistrer
                </button>
                <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="editingPseudo = false">
                  Annuler
                </button>
              </div>
              <p v-if="pseudoError" class="text-danger small mt-2 mb-0">{{ pseudoError }}</p>
            </form>
          </div>
        </div>

        <div class="identity__links">
          <router-link v-if="player.admin_role" to="/admin" class="cabinet-btn cabinet-btn--sm">
            <i class="fa-solid fa-lock-open"></i> Administration
          </router-link>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="handleLogout">
            <i class="fa-solid fa-right-from-bracket"></i> Se déconnecter
          </button>
        </div>
      </section>

      <!-- Partie / salon en cours -->
      <section v-if="current" class="board-card profile-card">
        <h2 class="board-card__title">
          {{ current.game_id ? 'Partie en cours' : 'Salon en cours' }}
        </h2>
        <p class="text-center mb-3">
          Salon <strong class="room-code">{{ current.room_code }}</strong>
          <template v-if="!current.game_id && current.room_game_id"> — une partie s'y joue, tu peux la regarder.</template>
        </p>
        <div class="text-center">
          <button type="button" class="play-btn play-btn--sm" @click="resume">
            <i class="fa-solid fa-play"></i> {{ current.game_id ? 'Rejoindre la partie' : 'Retourner au salon' }}
          </button>
        </div>
      </section>

      <!-- Mot de passe -->
      <section v-if="player" class="board-card profile-card">
        <h2 class="board-card__title">Mot de passe</h2>

        <p v-if="player.is_admin" class="mb-0 text-center">
          Ton profil s'ouvre avec le mot de passe d'administration, défini dans la configuration du serveur.
        </p>
        <template v-else>
          <p class="mb-3 text-center">
            <template v-if="player.has_password">Ton profil est protégé : ton mot de passe est demandé à chaque connexion.</template>
            <template v-else>Ton profil est public : n'importe qui peut s'y connecter avec ton pseudo.</template>
          </p>
          <p v-if="pwdMessage" class="text-success text-center small">{{ pwdMessage }}</p>

          <div v-if="!pwdMode" class="pwd-actions">
            <button type="button" class="cabinet-btn cabinet-btn--sm" @click="openPwd('set')">
              <i class="fa-solid fa-lock"></i> {{ player.has_password ? 'Changer' : 'Protéger mon profil' }}
            </button>
            <!-- Un administrateur désigné garde son mot de passe (refusé par le serveur) -->
            <button
                v-if="player.has_password && player.admin_role !== 'admin'"
                type="button"
                class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm"
                @click="openPwd('remove')"
            >
              <i class="fa-solid fa-lock-open"></i> Retirer
            </button>
          </div>

          <form v-else class="pwd-form" @submit.prevent="savePassword">
            <BFormInput
                v-if="player.has_password"
                v-model="pwd.current"
                type="password"
                autocomplete="current-password"
                placeholder="Mot de passe actuel"
            />
            <template v-if="pwdMode === 'set'">
              <BFormInput v-model="pwd.next" type="password" autocomplete="new-password" placeholder="Nouveau mot de passe (6 min.)" />
              <BFormInput v-model="pwd.confirm" type="password" autocomplete="new-password" placeholder="Confirme le mot de passe" />
              <p class="text-muted small mb-0">Tes autres appareils connectés à ce profil seront déconnectés.</p>
            </template>
            <p v-if="pwdError" class="text-danger small mb-0">{{ pwdError }}</p>
            <div class="pwd-actions">
              <button
                  type="submit"
                  class="cabinet-btn cabinet-btn--sm"
                  :class="{ 'cabinet-btn--danger': pwdMode === 'remove' }"
                  :disabled="pwdSaving"
              >
                {{ pwdMode === 'set' ? 'Enregistrer' : 'Retirer le mot de passe' }}
              </button>
              <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="pwdMode = null">
                Annuler
              </button>
            </div>
          </form>
        </template>
      </section>

      <!-- Parties -->
      <section v-if="player" class="board-card profile-card">
        <h2 class="board-card__title">Mes parties</h2>

        <div v-if="totals" class="totals">
          <div class="totals__item"><span class="totals__value">{{ totals.games_played }}</span> jouée{{ totals.games_played > 1 ? 's' : '' }}</div>
          <div class="totals__item"><span class="totals__value">{{ totals.wins }}</span> victoire{{ totals.wins > 1 ? 's' : '' }}</div>
          <div class="totals__item"><span class="totals__value">{{ totals.total_score }}</span> point{{ totals.total_score > 1 ? 's' : '' }}</div>
        </div>

        <p v-if="loading" class="text-center mb-0"><i class="fa-solid fa-spinner fa-spin"></i></p>
        <p v-else-if="!games.length" class="text-center mb-0">Aucune partie pour l'instant.</p>
        <ul v-else class="games">
          <li v-for="g in games" :key="g.game_id" class="games__row">
            <div class="games__main">
              <span class="games__date">{{ fmtDay(g.played_at) }}</span>
              <span v-if="g.room_code" class="games__room">· {{ g.room_code }}</span>
              <div v-if="g.character_name" class="games__character">{{ g.character_name }}</div>
            </div>
            <div class="games__side">
              <span class="pixel-chip" :class="gameStatus(g).cls">{{ gameStatus(g).label }}</span>
              <span class="games__score">{{ g.score }} pt{{ g.score > 1 ? 's' : '' }}</span>
              <router-link
                  v-if="g.status === 'finished'"
                  :to="{ name: 'History', query: { partie: g.game_id } }"
                  class="icon-btn"
                  aria-label="Voir la partie dans l'historique"
                  title="Voir dans l'historique"
              >
                <i class="fa-solid fa-chevron-right"></i>
              </router-link>
            </div>
          </li>
        </ul>
      </section>
    </div>

    <AvatarEditor
        v-if="player"
        v-model="showAvatarEditor"
        :player="player"
        @updated="url => updateProfilePlayer({ avatar_url: url })"
    />
  </div>
</template>

<style scoped>
.arcade-page {
  font-family: 'Baloo 2', system-ui, sans-serif;
  min-height: 100vh;
  padding: 1.5rem 1rem 2rem;
}

.profile-wrap {
  max-width: 560px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1.75rem;
}

.arcade-title-wrap {
  text-align: center;
  margin: 1.5rem 0 0.25rem;
}

.arcade-title {
  font-size: clamp(1.25rem, 6.5vw, 1.9rem);
}

.profile-card {
  padding: 1.5rem 1.1rem;
}

/* ── Identité ─────────────────────────────────────────────── */
.identity {
  display: flex;
  align-items: center;
  gap: 1rem;
  min-width: 0;
}

.identity__avatar {
  position: relative;
  flex-shrink: 0;
  width: 80px;
  height: 80px;
  border-radius: 50%;
  border: 3px solid var(--arcade-blue-grey-dark);
  background: var(--arcade-taupe);
  color: var(--arcade-beige);
  font-family: 'Press Start 2P', cursive;
  font-size: 1rem;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  cursor: pointer;
}

.identity__camera {
  position: absolute;
  right: -4px;
  bottom: -4px;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--arcade-blue-grey);
  border: 2px solid var(--arcade-beige);
  font-size: 0.7rem;
  display: flex;
  align-items: center;
  justify-content: center;
}

.identity__body {
  flex: 1;
  min-width: 0;
}

.identity__pseudo {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  min-width: 0;
}

.identity__name {
  font-weight: 800;
  font-size: 1.35rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.identity__chips {
  margin-top: 0.35rem;
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.identity__links {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.5rem;
  margin-top: 1.25rem;
}

.identity__links a {
  text-decoration: none;
}

.icon-btn {
  background: transparent;
  border: none;
  color: var(--arcade-blue-grey-dark);
  min-width: 44px;
  min-height: 44px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 10px;
  flex-shrink: 0;
}

.icon-btn:hover {
  background: rgba(0, 0, 0, 0.06);
}

.rename__actions,
.pwd-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.5rem;
  margin-top: 0.6rem;
}

.pwd-form {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.room-code {
  margin-left: 0.35rem;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.8rem;
  letter-spacing: 2px;
}

/* ── Parties ──────────────────────────────────────────────── */
.totals {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.5rem;
  text-align: center;
  margin-bottom: 1rem;
}

.totals__item {
  font-size: 0.85rem;
  line-height: 1.2;
}

.totals__value {
  display: block;
  font-family: 'Press Start 2P', cursive;
  font-size: 1rem;
  margin-bottom: 0.3rem;
}

.games {
  list-style: none;
  margin: 0;
  padding: 0;
}

.games__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.55rem 0;
  border-top: 2px dashed rgba(0, 0, 0, 0.12);
}

.games__main {
  min-width: 0;
}

.games__date {
  font-weight: 700;
}

.games__room {
  opacity: 0.7;
  font-size: 0.85rem;
}

.games__character {
  font-size: 0.85rem;
  opacity: 0.85;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.games__side {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-shrink: 0;
}

.games__score {
  font-weight: 700;
  font-size: 0.85rem;
  white-space: nowrap;
}
</style>
