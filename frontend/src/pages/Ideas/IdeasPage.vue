<script setup>
import { ref, computed, onMounted } from 'vue'
import { BContainer, BModal } from 'bootstrap-vue-next'
import { ideaService, ideaError } from '../../services/ideaService'
import { isCharactersUnlocked } from '../../services/charactersAccess'
import { isLoggedIn, isAdminProfile, isDelegateAdmin, profilePlayer } from '../../services/profile'
import PlayerAvatar from '../../components/player/PlayerAvatar.vue'

const BODY_MIN = 10
const BODY_MAX = 500

const ideas = ref([])
const dailyLimit = ref(5)
const remaining = ref(null)
const loading = ref(true)
const listError = ref(null)

// Même condition que la garde de l'espace admin (router.js) : le propriétaire
// doit avoir déverrouillé le cadenas, un admin désigné passe par son jeton.
const canModerate = computed(() => (isAdminProfile.value && isCharactersUnlocked.value) || isDelegateAdmin.value)

async function load() {
  loading.value = true
  listError.value = null
  try {
    const data = await ideaService.list()
    ideas.value = data.ideas ?? []
    dailyLimit.value = data.daily_limit ?? 5
    remaining.value = data.remaining_today
  } catch (e) {
    listError.value = ideaError(e, 'Impossible de charger les idées.')
  } finally {
    loading.value = false
  }
}

// Tri du serveur, réappliqué après un vote ou un favori : favoris d'abord
// (le plus récemment épinglé en tête), puis les plus votées, puis les plus récentes.
function sortIdeas() {
  ideas.value.sort((a, b) =>
    (b.is_pinned - a.is_pinned)
    || (a.is_pinned && (b.pinned_at ?? '').localeCompare(a.pinned_at ?? ''))
    || (b.votes - a.votes)
    || (b.id - a.id))
}

function replace(idea) {
  const i = ideas.value.findIndex((x) => x.id === idea.id)
  if (i !== -1) ideas.value[i] = idea
  sortIdeas()
}

function drop(id) {
  ideas.value = ideas.value.filter((x) => x.id !== id)
}

// ─── POSTER ───────────────────────────────────────────────────────────────────

const draft = ref('')
const posting = ref(false)
const postError = ref(null)
const postDone = ref(false)
const draftLength = computed(() => draft.value.trim().length)
const canPost = computed(() => draftLength.value >= BODY_MIN && draftLength.value <= BODY_MAX && !posting.value && remaining.value !== 0)

async function submit() {
  if (!canPost.value) return
  posting.value = true
  postError.value = null
  postDone.value = false
  try {
    const idea = await ideaService.create(profilePlayer.value.id, draft.value.trim())
    ideas.value.push(idea)
    sortIdeas()
    draft.value = ''
    postDone.value = true
    if (remaining.value !== null) remaining.value = Math.max(0, remaining.value - 1)
  } catch (e) {
    postError.value = ideaError(e, "Impossible de publier l'idée.")
  } finally {
    posting.value = false
  }
}

// ─── VOTER / FAVORI ───────────────────────────────────────────────────────────

const busy = ref({})
const rowError = ref({})

async function act(idea, fn) {
  if (busy.value[idea.id]) return
  busy.value[idea.id] = true
  rowError.value[idea.id] = null
  try {
    await fn()
  } catch (e) {
    if (e?.response?.status === 404) drop(idea.id)
    else rowError.value[idea.id] = ideaError(e, 'Action impossible.')
  } finally {
    busy.value[idea.id] = false
  }
}

function toggleVote(idea) {
  if (!isLoggedIn.value || idea.is_mine) return
  act(idea, async () => replace(await ideaService.vote(profilePlayer.value.id, idea.id, !idea.has_voted)))
}

function togglePin(idea) {
  act(idea, async () => replace(await ideaService.setPinned(idea.id, !idea.is_pinned)))
}

// ─── SUPPRIMER ────────────────────────────────────────────────────────────────

const confirmTarget = ref(null)
const showConfirm = computed({
  get: () => !!confirmTarget.value,
  set: (v) => { if (!v) confirmTarget.value = null },
})
const deleting = ref(false)
const deleteError = ref(null)

function askDelete(idea) {
  deleteError.value = null
  confirmTarget.value = idea
}

async function confirmDelete() {
  const idea = confirmTarget.value
  if (!idea || deleting.value) return
  deleting.value = true
  deleteError.value = null
  try {
    if (idea.is_mine) await ideaService.remove(profilePlayer.value.id, idea.id)
    else await ideaService.adminRemove(idea.id)
    drop(idea.id)
    confirmTarget.value = null
  } catch (e) {
    if (e?.response?.status === 404) {
      drop(idea.id)
      confirmTarget.value = null
    } else {
      deleteError.value = ideaError(e, "Impossible de supprimer l'idée.")
    }
  } finally {
    deleting.value = false
  }
}

function fmtDate(iso) {
  if (!iso) return ''
  return new Date(iso).toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', year: '2-digit' })
}

onMounted(load)
</script>

<template>
  <BContainer class="ideas py-4">
    <h1 class="page-title">Boîte à idées</h1>
    <p class="page-subtitle">
      Propose une amélioration du jeu et soutiens celles des autres d'un +1.
      Les idées en favori ont été retenues par l'administration.
    </p>

    <!-- ══════════ POSTER ══════════ -->
    <form v-if="isLoggedIn" class="composer" @submit.prevent="submit">
      <label for="idea-body" class="visually-hidden">Ton idée</label>
      <textarea
          id="idea-body"
          v-model="draft"
          class="composer__input"
          rows="3"
          :maxlength="BODY_MAX"
          placeholder="Ton idée pour améliorer Binome…"
          :disabled="remaining === 0"
      ></textarea>
      <div class="composer__bar">
        <span class="muted">
          {{ draftLength }}/{{ BODY_MAX }}
          <template v-if="remaining !== null"> · {{ remaining }}/{{ dailyLimit }} idée{{ dailyLimit > 1 ? 's' : '' }} restante{{ remaining > 1 ? 's' : '' }} sur 24 h</template>
        </span>
        <button type="submit" class="cabinet-btn cabinet-btn--sm" :disabled="!canPost">
          <i class="fa-solid fa-paper-plane"></i> {{ posting ? 'Envoi…' : 'Publier' }}
        </button>
      </div>
      <p class="composer__privacy muted">
        Ton idée est publique et signée de ton pseudo. Tu peux la supprimer à tout moment.
        <router-link :to="{ name: 'Legal' }">Tes données</router-link>
      </p>
      <p v-if="postError" class="alert-line" role="alert">{{ postError }}</p>
      <p v-else-if="postDone" class="ok-line" role="status">Idée publiée, merci !</p>
    </form>
    <div v-else class="locked">
      <i class="fa-solid fa-lightbulb locked__icon"></i>
      <p><router-link to="/connexion">Connecte-toi</router-link> pour proposer une idée ou voter.</p>
    </div>

    <!-- ══════════ LISTE ══════════ -->
    <div class="toolbar">
      <span class="muted">{{ ideas.length }} idée{{ ideas.length > 1 ? 's' : '' }}</span>
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" aria-label="Rafraîchir" @click="load">
        <i class="fa-solid fa-rotate-right"></i>
      </button>
    </div>

    <p v-if="listError" class="alert-line" role="alert">{{ listError }}</p>
    <p v-if="loading && ideas.length === 0" class="muted">Chargement…</p>
    <p v-else-if="!listError && ideas.length === 0" class="muted">Aucune idée pour le moment : lance-toi !</p>

    <ul v-else class="idea-list">
      <li v-for="idea in ideas" :key="idea.id" class="idea-card" :class="{ 'idea-card--pinned': idea.is_pinned }">
        <p v-if="idea.is_pinned" class="pinned-tag"><i class="fa-solid fa-star"></i> Favori</p>
        <p class="idea-card__body">{{ idea.body }}</p>
        <div class="idea-card__foot">
          <span class="author">
            <span class="mini-avatar" aria-hidden="true"><PlayerAvatar :pseudo="idea.author_name" :url="idea.author_avatar_url" :letters="1" /></span>
            <span class="author__name">{{ idea.author_name }}</span>
            <span class="muted author__date">· {{ fmtDate(idea.created_at) }}</span>
          </span>
          <div class="actions">
            <button
                v-if="canModerate"
                type="button"
                class="icon-btn"
                :class="{ 'is-on': idea.is_pinned }"
                :aria-pressed="idea.is_pinned"
                :title="idea.is_pinned ? 'Retirer des favoris' : 'Mettre en favori'"
                :aria-label="idea.is_pinned ? 'Retirer des favoris' : 'Mettre en favori'"
                :disabled="busy[idea.id]"
                @click="togglePin(idea)"
            >
              <i :class="idea.is_pinned ? 'fa-solid fa-star' : 'fa-regular fa-star'"></i>
            </button>
            <button
                v-if="idea.is_mine || canModerate"
                type="button"
                class="icon-btn icon-btn--danger"
                title="Supprimer"
                aria-label="Supprimer l'idée"
                :disabled="busy[idea.id]"
                @click="askDelete(idea)"
            >
              <i class="fa-solid fa-trash"></i>
            </button>
            <button
                type="button"
                class="vote-btn"
                :class="{ 'is-on': idea.has_voted }"
                :aria-pressed="idea.has_voted"
                :title="!isLoggedIn ? 'Connecte-toi pour voter' : idea.is_mine ? 'Tu ne peux pas voter pour ta propre idée' : idea.has_voted ? 'Retirer mon +1' : 'Soutenir cette idée'"
                :disabled="!isLoggedIn || idea.is_mine || busy[idea.id]"
                @click="toggleVote(idea)"
            >
              <i class="fa-solid fa-thumbs-up"></i> +{{ idea.votes }}
            </button>
          </div>
        </div>
        <p v-if="rowError[idea.id]" class="alert-line alert-line--sm" role="alert">{{ rowError[idea.id] }}</p>
      </li>
    </ul>

    <BModal v-model="showConfirm" title="Supprimer l'idée" no-footer class="arcade-modal">
      <p v-if="confirmTarget" class="text-center">
        <template v-if="confirmTarget.is_mine">Supprimer ton idée ?</template>
        <template v-else>Supprimer l'idée de <strong>{{ confirmTarget.author_name }}</strong> ?</template>
        Ses votes partent avec elle, c'est irréversible.
      </p>
      <p v-if="deleteError" class="alert-line" role="alert">{{ deleteError }}</p>
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
/* Mobile first : calé sur 320 px, rien ne doit dépasser la largeur du conteneur. */
.ideas {
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
.muted { color: var(--arcade-taupe); font-size: 0.85rem; }
.alert-line,
.ok-line {
  border-radius: 8px;
  padding: 0.5rem 0.75rem;
  margin: 0.5rem 0 0;
  font-weight: 700;
  color: #fff;
  overflow-wrap: anywhere;
}
.alert-line { background: var(--arcade-danger-dark); }
.ok-line { background: var(--arcade-success-dark); }
.alert-line--sm { font-size: 0.8rem; padding: 0.35rem 0.6rem; }

/* ── Saisie ───────────────────────────────────────────────────── */
.composer {
  margin-bottom: 1.2rem;
}
.composer__input {
  display: block;
  width: 100%;
  min-width: 0;
  padding: 0.55rem 0.7rem;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 10px;
  background: #fff;
  /* style.css déclare `color-scheme: light dark` : sans couleur explicite, le
     texte saisi devient blanc sur blanc quand l'OS est en thème sombre. */
  color: var(--arcade-dark);
  color-scheme: light;
  font-family: inherit;
  font-size: 16px; /* < 16px déclenche le zoom automatique de Safari iOS */
  resize: vertical;
}
.composer__input::placeholder { color: var(--arcade-taupe); opacity: 1; }
.composer__bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.3rem 0.5rem;
  margin-top: 0.3rem;
}
.composer__bar .muted { min-width: 0; font-size: 0.78rem; }
.composer__bar .cabinet-btn { margin: 0.35rem 0 0.35rem auto; }
.composer__privacy { margin: 0.1rem 0 0; font-size: 0.75rem; }
.composer__privacy a { color: var(--arcade-gold); font-weight: 700; }

.locked {
  text-align: center;
  padding: 1.2rem 1rem;
  margin-bottom: 1.2rem;
  border: 2px dashed var(--arcade-blue-grey);
  border-radius: 12px;
}
.locked__icon { font-size: 1.6rem; color: var(--arcade-gold); margin-bottom: 0.5rem; }
.locked p { margin: 0; }
.locked a { color: var(--arcade-gold); font-weight: 700; }

/* ── Liste ────────────────────────────────────────────────────── */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  margin-bottom: 0.4rem;
}
.toolbar .cabinet-btn { margin-right: 0; }
.idea-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}
.idea-card {
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.7rem 0.8rem;
  min-width: 0;
}
.idea-card--pinned { border-color: var(--arcade-gold); }
.pinned-tag {
  margin: 0 0 0.3rem;
  color: var(--arcade-gold);
  font-size: 0.75rem;
  font-weight: 800;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.idea-card__body {
  margin: 0 0 0.6rem;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.idea-card__foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.4rem 0.6rem;
}
.author {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  flex: 1 1 8rem;
  font-size: 0.85rem;
}
.author__name {
  font-weight: 700;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.author__date { flex: 0 0 auto; margin-left: 0.3rem; font-size: 0.75rem; }
.mini-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 1.5rem;
  height: 1.5rem;
  margin-right: 0.35rem;
  border-radius: 50%;
  overflow: hidden;
  background: var(--arcade-taupe);
  color: #fff;
  font-family: 'Press Start 2P', cursive;
  font-size: 0.5rem;
  line-height: 1;
}
.actions {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-left: auto;
}
/* Cibles tactiles de 44 px */
.icon-btn,
.vote-btn {
  min-height: 44px;
  border-radius: 10px;
  font-family: inherit;
  font-weight: 800;
  cursor: pointer;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
}
.icon-btn {
  min-width: 44px;
  padding: 0;
  border: 2px solid var(--arcade-blue-grey-dark);
  background: transparent;
  color: var(--arcade-taupe);
}
.icon-btn.is-on { color: var(--arcade-gold); border-color: var(--arcade-gold); }
.icon-btn--danger:hover:not(:disabled) { color: #fff; background: var(--arcade-danger); border-color: var(--arcade-danger); }
.vote-btn {
  min-width: 4.2rem;
  padding: 0 0.8rem;
  border: 2px solid var(--arcade-blue-grey);
  background: transparent;
  color: var(--arcade-beige);
}
.vote-btn.is-on {
  background: var(--arcade-success);
  border-color: var(--arcade-success);
  color: #fff;
}
.icon-btn:disabled,
.vote-btn:disabled { cursor: default; opacity: 0.55; }
</style>
