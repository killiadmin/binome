<script setup>
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import { BContainer, BModal } from 'bootstrap-vue-next'
import { adminService } from '../../services/adminService'
import { characterService } from '../../services/characterService'
import UniverseSheet from '../../components/universe/UniverseSheet.vue'
import ImageDropzone from '../../components/cropper/ImageDropzone.vue'
import ImageCropperModal from '../../components/cropper/ImageCropperModal.vue'

// Fiches de présentation des univers (un cosmos : Disney, Films saga…) : nom,
// image 16/9, courte présentation. Montrées aux joueurs dans le salon (choix du
// cosmos) et dans les règles (« Univers jouables »), pour les cosmos activés.

const DESCRIPTION_MIN = 10
const DESCRIPTION_MAX = 600

const cosmos = ref([])
const loading = ref(true)
const error = ref(null)
const success = ref(null)

// Fiche en cours d'édition (une seule à la fois)
const editingId = ref(null)
const form = reactive({ description: '', imageFile: null, imagePreview: null, removeImage: false })
const saving = ref(false)

// Recadrage de l'image choisie
const cropSrc = ref(null)
const showCropper = ref(false)

// Suppression
const confirmTarget = ref(null)
const deleting = ref(false)
const showConfirm = computed({
  get: () => confirmTarget.value !== null,
  set: (v) => { if (!v) confirmTarget.value = null },
})

// Cosmos activés d'abord (ceux que voient les joueurs), puis par nom.
const sortedCosmos = computed(() =>
  [...cosmos.value].sort((a, b) => (b.is_enabled - a.is_enabled) || a.name.localeCompare(b.name, 'fr')),
)

const editing = computed(() => cosmos.value.find((c) => c.id === editingId.value) ?? null)

const descriptionLength = computed(() => form.description.trim().length)
const descriptionValid = computed(() => descriptionLength.value >= DESCRIPTION_MIN && descriptionLength.value <= DESCRIPTION_MAX)

// Aperçu en direct de la fiche telle que la verront les joueurs.
const preview = computed(() => {
  if (!editing.value) return null
  return {
    name: editing.value.name,
    description: form.description.trim() || null,
    image: form.removeImage ? null : (form.imagePreview || editing.value.image),
  }
})

function hasSheet(c) {
  return !!c.description || !!c.image
}

async function load() {
  loading.value = true
  error.value = null
  try {
    const { data } = await characterService.listCosmos()
    cosmos.value = data.cosmos ?? []
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les univers.'
  } finally {
    loading.value = false
  }
}

function flash(message) {
  success.value = message
  setTimeout(() => (success.value = null), 3000)
}

function revokePreview() {
  if (form.imagePreview) URL.revokeObjectURL(form.imagePreview)
  form.imagePreview = null
}

function startEdit(c) {
  revokePreview()
  editingId.value = c.id
  form.description = c.description ?? ''
  form.imageFile = null
  form.removeImage = false
  error.value = null
}

function cancelEdit() {
  revokePreview()
  editingId.value = null
}

function onFile(file) {
  if (cropSrc.value) URL.revokeObjectURL(cropSrc.value)
  cropSrc.value = URL.createObjectURL(file)
  showCropper.value = true
}

function onCropped(file) {
  revokePreview()
  form.imageFile = file
  form.imagePreview = URL.createObjectURL(file)
  form.removeImage = false
}

function removeImage() {
  revokePreview()
  form.imageFile = null
  form.removeImage = true
}

function applySheet(sheet) {
  const c = cosmos.value.find((x) => x.id === sheet.id)
  if (c) Object.assign(c, { description: sheet.description, image: sheet.image })
}

async function save() {
  if (!editing.value || saving.value || !descriptionValid.value) return
  saving.value = true
  error.value = null
  try {
    const { data } = await adminService.saveCosmosSheet(editing.value.id, {
      description: form.description.trim(),
      imageFile: form.imageFile,
      removeImage: form.removeImage,
    })
    applySheet(data.cosmos)
    flash(data.message)
    cancelEdit()
  } catch (e) {
    const errors = e.response?.data?.errors
    error.value = (errors && Object.values(errors)[0]?.[0]) || e.response?.data?.message || "L'enregistrement a échoué."
  } finally {
    saving.value = false
  }
}

async function confirmDelete() {
  const c = confirmTarget.value
  if (!c || deleting.value) return
  deleting.value = true
  error.value = null
  try {
    const { data } = await adminService.deleteCosmosSheet(c.id)
    applySheet(data.cosmos)
    flash(data.message)
    if (editingId.value === c.id) cancelEdit()
    confirmTarget.value = null
  } catch (e) {
    error.value = e.response?.data?.message || 'La suppression a échoué.'
  } finally {
    deleting.value = false
  }
}

onMounted(load)
onUnmounted(() => {
  revokePreview()
  if (cropSrc.value) URL.revokeObjectURL(cropSrc.value)
})
</script>

<template>
  <BContainer class="admin-universes py-4">
    <h1 class="page-title">Fiches univers</h1>
    <p class="page-subtitle">
      Courte présentation de chaque univers (cosmos) : nom, image et quelques
      lignes pour que les joueurs sachent jusqu'où chercher. Elle s'affiche dans
      le salon quand l'hôte choisit un cosmos et dans les règles, rubrique
      « Univers jouables » — seulement pour les cosmos activés (onglet Parties).
    </p>

    <div class="toolbar">
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" :disabled="loading" @click="load">
        <i class="fa-solid fa-rotate-right"></i> Rafraîchir
      </button>
    </div>

    <p v-if="error" class="alert-line alert-line--error">{{ error }}</p>
    <p v-if="success" class="alert-line alert-line--success">{{ success }}</p>
    <p v-if="loading" class="muted">Chargement…</p>
    <p v-else-if="cosmos.length === 0" class="muted">Aucun cosmos pour le moment (création : onglet Binômes).</p>

    <ul v-else class="universe-list">
      <li v-for="c in sortedCosmos" :key="c.id" class="universe-item" :class="{ 'is-editing': editingId === c.id }">
        <div class="universe-row">
          <span class="universe-thumb">
            <img v-if="c.image" :src="c.image" alt="" loading="lazy" />
            <i v-else class="fa-solid fa-earth-europe" aria-hidden="true"></i>
          </span>
          <div class="universe-info">
            <span class="universe-name">{{ c.name }}</span>
            <span class="universe-tags">
              <span class="tag" :class="c.is_enabled ? 'tag--on' : 'tag--off'">{{ c.is_enabled ? 'Activé' : 'Désactivé' }}</span>
              <span class="tag" :class="hasSheet(c) ? 'tag--on' : 'tag--off'">{{ hasSheet(c) ? 'Fiche' : 'Sans fiche' }}</span>
              <span v-if="hasSheet(c) && !c.image" class="tag tag--off">Sans image</span>
            </span>
          </div>
          <div v-if="editingId !== c.id" class="universe-actions">
            <button type="button" class="cabinet-btn cabinet-btn--sm" @click="startEdit(c)">
              <i class="fa-solid" :class="hasSheet(c) ? 'fa-pen' : 'fa-plus'"></i>
              {{ hasSheet(c) ? 'Modifier' : 'Créer la fiche' }}
            </button>
            <button
                v-if="hasSheet(c)"
                type="button"
                class="icon-btn icon-btn--danger"
                :aria-label="`Supprimer la fiche ${c.name}`"
                @click="confirmTarget = c"
            >
              <i class="fa-solid fa-trash"></i>
            </button>
          </div>
        </div>

        <!-- Édition -->
        <form v-if="editingId === c.id" class="sheet-form" @submit.prevent="save">
          <label class="field-label" :for="`sheet-desc-${c.id}`">Présentation</label>
          <textarea
              :id="`sheet-desc-${c.id}`"
              v-model="form.description"
              class="input textarea"
              rows="5"
              :maxlength="DESCRIPTION_MAX"
              placeholder="Ex : les grands classiques que tout le monde connaît, rien de confidentiel…"
          ></textarea>
          <span class="counter" :class="{ 'is-bad': form.description.trim() && !descriptionValid }">
            {{ descriptionLength }} / {{ DESCRIPTION_MAX }} caractères (au moins {{ DESCRIPTION_MIN }})
          </span>

          <span class="field-label">Image <span class="muted">(facultative, recadrée en 16/9)</span></span>
          <ImageDropzone hint="PNG, JPG, GIF ou WEBP — recadrage 16/9 à l’étape suivante" :disabled="saving" @file="onFile" />
          <button
              v-if="!form.removeImage && (form.imagePreview || c.image)"
              type="button"
              class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm remove-image"
              @click="removeImage"
          >
            <i class="fa-solid fa-image"></i> Retirer l'image
          </button>

          <span class="field-label">Aperçu</span>
          <div class="preview">
            <UniverseSheet v-if="preview" :cosmos="preview" heading-tag="h3" />
          </div>

          <div class="form-actions">
            <button type="submit" class="cabinet-btn cabinet-btn--sm" :disabled="saving || !descriptionValid">
              <i class="fa-solid fa-floppy-disk"></i> {{ saving ? 'Enregistrement…' : 'Enregistrer' }}
            </button>
            <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" :disabled="saving" @click="cancelEdit">
              Annuler
            </button>
          </div>
        </form>
      </li>
    </ul>

    <ImageCropperModal
        v-model="showCropper"
        :image-src="cropSrc"
        :aspect-ratio="16 / 9"
        :output-size="960"
        :output-height="540"
        :quality="0.85"
        @cropped="onCropped"
    />

    <BModal v-model="showConfirm" title="Supprimer la fiche" no-footer class="arcade-modal">
      <p class="text-center">
        Supprimer la fiche de <strong>{{ confirmTarget?.name }}</strong> (présentation et image) ?
        Le cosmos lui-même n'est pas touché.
      </p>
      <div class="text-center mt-3 confirm-actions">
        <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" :disabled="deleting" @click="confirmDelete">
          <i class="fa-solid fa-trash"></i> Supprimer
        </button>
        <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="confirmTarget = null">
          Annuler
        </button>
      </div>
    </BModal>
  </BContainer>
</template>

<style scoped>
.admin-universes {
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
  margin-bottom: 1rem;
}

.toolbar {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 0.75rem;
}

.muted {
  color: var(--arcade-taupe);
  font-weight: 400;
}

.alert-line {
  border-radius: 8px;
  padding: 0.5rem 0.75rem;
  font-weight: 700;
}
.alert-line--error { background: var(--arcade-danger-dark); color: #fff; }
.alert-line--success { background: var(--arcade-success-dark); color: #fff; }

.universe-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.universe-item {
  text-align: left;
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.6rem;
}

.universe-item.is-editing {
  border-color: var(--arcade-blue-grey);
}

.universe-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.6rem;
}

.universe-thumb {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 80px;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  border-radius: 6px;
  background: linear-gradient(135deg, var(--arcade-blue-grey), var(--arcade-blue-grey-dark));
  color: rgba(245, 245, 220, 0.55);
}

.universe-thumb img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.universe-info {
  flex: 1 1 120px;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.universe-name {
  font-weight: 700;
  overflow-wrap: anywhere;
}

.universe-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}

.tag {
  padding: 0 0.45rem;
  border-radius: 999px;
  border: 1px solid currentColor;
  font-size: 0.72rem;
  font-weight: 700;
}
.tag--on { color: var(--arcade-success, #66bb6a); }
.tag--off { color: var(--arcade-taupe); }

.universe-actions {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-left: auto;
}

.universe-actions .cabinet-btn {
  margin: 0;
}

.icon-btn {
  width: 44px;
  height: 44px;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 8px;
  background: transparent;
  color: var(--arcade-beige);
  cursor: pointer;
}

.icon-btn--danger {
  color: #ef9a9a;
}

.sheet-form {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  margin-top: 0.75rem;
  padding-top: 0.75rem;
  border-top: 2px dashed var(--arcade-blue-grey-dark);
}

.field-label {
  font-weight: 700;
  font-size: 0.9rem;
  margin-top: 0.4rem;
}

.input {
  width: 100%;
  min-width: 0;
  background: rgba(0, 0, 0, 0.3);
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 8px;
  color: var(--arcade-beige);
  padding: 0.4rem 0.6rem;
  /* 16px : évite le zoom automatique d'iOS sur le champ. */
  font-size: 1rem;
}

.textarea {
  resize: vertical;
  line-height: 1.4;
}

.counter {
  font-size: 0.75rem;
  color: var(--arcade-taupe);
  text-align: right;
}
.counter.is-bad { color: #ef9a9a; }

.remove-image {
  align-self: flex-start;
  margin: 0;
}

.preview {
  max-width: 420px;
}

.form-actions,
.confirm-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  margin-top: 0.6rem;
}

.confirm-actions {
  justify-content: center;
}

.form-actions .cabinet-btn,
.confirm-actions .cabinet-btn {
  margin: 0;
}
</style>
