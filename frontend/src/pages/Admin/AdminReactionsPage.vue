<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { BContainer, BModal } from 'bootstrap-vue-next'
import { adminService } from '../../services/adminService'
import ReactionEditor from '../../components/game/ReactionEditor.vue'

// Toutes les réactions de la barre en partie : emojis Unicode et images
// (créées par les joueurs via QuickReactions → ReactionEditor, actives dès
// leur création, ou ajoutées ici). On les ajoute, renomme, masque (retirées
// de la barre, image conservée) ou supprime.

// ─── STATE ────────────────────────────────────────────────────────────────────

const reactions = ref([])
const loading = ref(true)
const error = ref(null)
const success = ref(null)

const filter = ref('all')
const FILTERS = { all: 'Tous', enabled: 'Actifs', disabled: 'Masqués' }

const busy = reactive({})

// Ajout
const newEmoji = ref('')
const newEmojiName = ref('')
const adding = ref(false)
const showImageEditor = ref(false)

// Renommage en ligne
const editingId = ref(null)
const editName = ref('')

// Suppression
const showConfirm = ref(false)
const confirmTarget = ref(null)
const deleting = ref(false)

const filteredReactions = computed(() => {
  if (filter.value === 'enabled') return reactions.value.filter((r) => r.is_enabled)
  if (filter.value === 'disabled') return reactions.value.filter((r) => !r.is_enabled)
  return reactions.value
})

// ─── CHARGEMENT ───────────────────────────────────────────────────────────────

async function loadReactions() {
  loading.value = true
  error.value = null
  try {
    const { data } = await adminService.listReactions()
    reactions.value = data.reactions
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les emojis.'
  } finally {
    loading.value = false
  }
}

function flash(message) {
  success.value = message
  setTimeout(() => (success.value = null), 3000)
}

// ─── AJOUT ────────────────────────────────────────────────────────────────────

async function addEmoji() {
  if (adding.value || !newEmoji.value.trim() || !newEmojiName.value.trim()) return
  adding.value = true
  error.value = null
  try {
    const { data } = await adminService.createReaction({ name: newEmojiName.value.trim(), emoji: newEmoji.value.trim() })
    onAdded(data.reaction)
    newEmoji.value = ''
    newEmojiName.value = ''
  } catch (e) {
    error.value = e.response?.data?.message || "L'ajout a échoué."
  } finally {
    adding.value = false
  }
}

function onAdded(reaction) {
  reactions.value = [...reactions.value, reaction]
  flash(`« ${reaction.name} » ajouté.`)
}

// ─── MODIFICATION ─────────────────────────────────────────────────────────────

async function update(reaction, payload) {
  if (busy[reaction.id]) return
  busy[reaction.id] = true
  error.value = null
  try {
    const { data } = await adminService.updateReaction(reaction.id, payload)
    reactions.value = reactions.value.map((r) => (r.id === reaction.id ? data.reaction : r))
    return true
  } catch (e) {
    error.value = e.response?.data?.message || 'La mise à jour a échoué.'
    return false
  } finally {
    busy[reaction.id] = false
  }
}

async function toggle(reaction) {
  if (await update(reaction, { is_enabled: !reaction.is_enabled })) {
    flash(`« ${reaction.name} » ${reaction.is_enabled ? 'masqué' : 'réactivé'}.`)
  }
}

function startRename(reaction) {
  editingId.value = reaction.id
  editName.value = reaction.name
}

async function saveRename(reaction) {
  const name = editName.value.trim()
  if (!name || name === reaction.name) {
    editingId.value = null
    return
  }
  if (await update(reaction, { name })) editingId.value = null
}

// ─── SUPPRESSION ──────────────────────────────────────────────────────────────

function askDelete(reaction) {
  confirmTarget.value = reaction
  showConfirm.value = true
}

async function confirmDelete() {
  if (!confirmTarget.value || deleting.value) return
  deleting.value = true
  error.value = null
  const { id, name } = confirmTarget.value
  try {
    await adminService.deleteReaction(id)
    reactions.value = reactions.value.filter((r) => r.id !== id)
    flash(`« ${name} » supprimé.`)
    showConfirm.value = false
    confirmTarget.value = null
  } catch (e) {
    error.value = e.response?.data?.message || 'La suppression a échoué.'
  } finally {
    deleting.value = false
  }
}

function fmtDate(iso) {
  if (!iso) return '—'
  return new Date(iso).toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', year: 'numeric' })
}

onMounted(loadReactions)
</script>

<template>
  <BContainer class="admin-reactions py-4">
    <h1 class="page-title">Emojis</h1>
    <p class="page-subtitle">
      Réactions de la barre en partie. Les joueurs peuvent aussi créer les leurs
      (photo ou image, bouton <i class="fa-solid fa-plus"></i> de la barre) : elles
      sont actives tout de suite. Un emoji masqué disparaît de la barre mais reste
      lisible dans les réactions déjà envoyées.
    </p>

    <section class="add">
      <h2 class="section-title">Ajouter</h2>
      <form class="add-form" @submit.prevent="addEmoji">
        <input v-model="newEmoji" type="text" maxlength="16" class="input input--emoji"
               placeholder="🎉" aria-label="Emoji" />
        <input v-model="newEmojiName" type="text" maxlength="20" class="input"
               placeholder="Nom (ex : Fête)" aria-label="Nom de l'emoji" />
        <button type="submit" class="cabinet-btn cabinet-btn--sm"
                :disabled="adding || !newEmoji.trim() || !newEmojiName.trim()">
          <i class="fa-solid fa-plus"></i> Ajouter
        </button>
      </form>
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showImageEditor = true">
        <i class="fa-solid fa-camera"></i> Ajouter une photo / image
      </button>
    </section>

    <div class="filters">
      <button
        v-for="(label, key) in FILTERS"
        :key="key"
        type="button"
        class="filter-chip"
        :class="{ active: filter === key }"
        @click="filter = key"
      >
        {{ label }}
      </button>
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm ms-auto" @click="loadReactions">
        <i class="fa-solid fa-rotate-right"></i> Rafraîchir
      </button>
    </div>

    <p v-if="error" class="alert-line alert-line--error">{{ error }}</p>
    <p v-if="success" class="alert-line alert-line--success">{{ success }}</p>

    <p v-if="loading" class="muted">Chargement…</p>
    <p v-else-if="filteredReactions.length === 0" class="muted">
      Aucun emoji{{ filter === 'all' ? ' créé pour le moment' : '' }}.
    </p>

    <ul v-else class="reaction-list">
      <li
        v-for="r in filteredReactions"
        :key="r.id"
        class="reaction-card"
        :class="{ 'is-disabled': !r.is_enabled }"
      >
        <img v-if="r.url" :src="r.url" :alt="r.name" class="reaction-img" loading="lazy" />
        <span v-else class="reaction-img reaction-img--emoji">{{ r.emoji }}</span>

        <form v-if="editingId === r.id" class="rename" @submit.prevent="saveRename(r)">
          <input v-model="editName" type="text" maxlength="20" class="input" autofocus />
          <button type="submit" class="icon-btn" :disabled="busy[r.id]" aria-label="Enregistrer le nom">
            <i class="fa-solid fa-check"></i>
          </button>
          <button type="button" class="icon-btn" aria-label="Annuler" @click="editingId = null">
            <i class="fa-solid fa-xmark"></i>
          </button>
        </form>
        <div v-else class="reaction-name">
          {{ r.name }}
          <span v-if="!r.is_enabled" class="tag">masqué</span>
        </div>

        <div class="reaction-meta">
          {{ r.url ? `par ${r.created_by ?? '?'}` : 'emoji' }} · {{ fmtDate(r.created_at) }}
        </div>

        <div class="reaction-actions">
          <button type="button" class="icon-btn" :disabled="busy[r.id]" aria-label="Renommer" title="Renommer"
                  @click="startRename(r)">
            <i class="fa-solid fa-pen"></i>
          </button>
          <button type="button" class="icon-btn" :disabled="busy[r.id]"
                  :aria-label="r.is_enabled ? 'Masquer' : 'Réactiver'"
                  :title="r.is_enabled ? 'Masquer' : 'Réactiver'"
                  @click="toggle(r)">
            <i class="fa-solid" :class="r.is_enabled ? 'fa-eye-slash' : 'fa-eye'"></i>
          </button>
          <button type="button" class="icon-btn icon-btn--danger" :disabled="busy[r.id]"
                  aria-label="Supprimer" title="Supprimer" @click="askDelete(r)">
            <i class="fa-solid fa-trash"></i>
          </button>
        </div>
      </li>
    </ul>

    <BModal v-model="showConfirm" title="Supprimer l'emoji" no-footer class="arcade-modal">
      <p v-if="confirmTarget" class="text-center">
        Supprimer l'emoji <strong>« {{ confirmTarget.name }} »</strong>
        <template v-if="confirmTarget.created_by">créé par {{ confirmTarget.created_by }}</template> ?
        Cette action est irréversible — pour le retirer temporairement, masque-le plutôt.
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

    <ReactionEditor v-model="showImageEditor" admin @created="onAdded" />
  </BContainer>
</template>

<style scoped>
.admin-reactions {
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

.section-title {
  font-size: 0.95rem;
  font-weight: 700;
  margin-bottom: 0.5rem;
}

.add {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.5rem;
  margin-bottom: 1.25rem;
  padding: 0.75rem;
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
}

.add-form {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  width: 100%;
}

.add-form .input {
  flex: 1 1 140px;
  padding: 0.4rem 0.6rem;
}

.add-form .input--emoji {
  flex: 0 0 56px;
  text-align: center;
  font-size: 1.2rem;
}

.add-form .cabinet-btn {
  margin: 0;
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

.reaction-list {
  list-style: none;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(150px, 100%), 1fr));
  gap: 0.6rem;
}

.reaction-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.35rem;
  min-width: 0;
  text-align: center;
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.75rem 0.6rem 0.5rem;
}

.reaction-card.is-disabled .reaction-img {
  opacity: 0.4;
  filter: grayscale(1);
}

.reaction-img--emoji {
  display: grid;
  place-items: center;
  font-size: 2.2rem;
  line-height: 1;
  background: rgba(0, 0, 0, 0.25);
}

.reaction-img {
  width: 64px;
  height: 64px;
  border-radius: 50%;
  object-fit: cover;
  border: 3px solid var(--arcade-blue-grey);
}

.reaction-name {
  font-weight: 700;
  overflow-wrap: anywhere;
}

.reaction-meta {
  color: var(--arcade-taupe);
  font-size: 0.75rem;
  overflow-wrap: anywhere;
}

.reaction-actions,
.rename {
  display: flex;
  justify-content: center;
  gap: 0.25rem;
  width: 100%;
}

.input {
  flex: 1;
  min-width: 0;
  background: rgba(0, 0, 0, 0.3);
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 8px;
  color: var(--arcade-beige);
  padding: 0.2rem 0.4rem;
}

.icon-btn {
  flex: none;
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
  opacity: 0.4;
}

.icon-btn--danger {
  color: var(--arcade-danger);
}

.tag {
  font-size: 0.65rem;
  text-transform: uppercase;
  padding: 0.1rem 0.4rem;
  border-radius: 5px;
  margin-left: 0.3rem;
  background: var(--arcade-blue-grey);
  color: #fff;
}
</style>
