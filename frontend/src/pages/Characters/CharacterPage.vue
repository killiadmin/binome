<script setup>
import { ref, onMounted, computed } from 'vue'
import { characterService } from '../../services/characterService'
import { BContainer, BFormInput, BFormSelect, BModal } from 'bootstrap-vue-next'
import ImageCropperModal from '../../components/cropper/ImageCropperModal.vue'

// ─── STATE ────────────────────────────────────────────────────────────────────

const universes = ref([])
const loadingUniverses = ref(true)
const error = ref(null)
const success = ref(null)
const submitting = ref(false)

const isNewUniverse = ref(false)
const selectedUniverseId = ref(null)
const newUniverseName = ref('')

const characterName = ref('')
const characterImageFile = ref(null)
const characterImagePreview = ref(null)
const forbiddenWords = ref(['', '', ''])
const levelAffectation = ref(null)

const characters = ref([])
const loadingCharacters = ref(true)

const showEditModal = ref(false)
const editingCharacterId = ref(null)
const editName = ref('')
const editForbiddenWords = ref(['', '', ''])
const editLevelAffectation = ref(null)
const editImageFile = ref(null)
const editImagePreview = ref(null)
const editCurrentImage = ref(null)
const editError = ref(null)
const editSubmitting = ref(false)

const showDeleteModal = ref(false)
const deletingCharacter = ref(null)
const deleteError = ref(null)
const deleteSubmitting = ref(false)

const characterFileInput = ref(null)
const editFileInput = ref(null)

const showCropperModal = ref(false)
const cropperSourceSrc = ref(null)
const cropperTarget = ref(null) // 'create' | 'edit'

const universeOptions = computed(() => [
  { value: null, text: 'Choisir un univers existant…' },
  ...universes.value.map(u => ({
    value: u.id,
    text: `${u.name} (${u.characters_count} personnage${u.characters_count > 1 ? 's' : ''})`,
  })),
])

// ─── DATA LOADING ─────────────────────────────────────────────────────────────

async function loadUniverses() {
  loadingUniverses.value = true
  try {
    const res = await characterService.listUniverses()
    universes.value = res.data.universes
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les univers.'
  } finally {
    loadingUniverses.value = false
  }
}

async function loadCharacters() {
  loadingCharacters.value = true
  try {
    const res = await characterService.listCharacters()
    characters.value = res.data.characters
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les personnages.'
  } finally {
    loadingCharacters.value = false
  }
}

onMounted(() => {
  loadUniverses()
  loadCharacters()
})

// ─── IMAGE UPLOAD & RECADRAGE ─────────────────────────────────────────────────

function onCharacterImageChange(event) {
  const file = event.target.files[0] ?? null
  if (!file) return
  cropperSourceSrc.value = URL.createObjectURL(file)
  cropperTarget.value = 'create'
  showCropperModal.value = true
}

function handleCropped(croppedFile) {
  if (cropperSourceSrc.value) {
    URL.revokeObjectURL(cropperSourceSrc.value)
    cropperSourceSrc.value = null
  }

  if (cropperTarget.value === 'create') {
    if (characterImagePreview.value) {
      URL.revokeObjectURL(characterImagePreview.value)
    }
    characterImageFile.value = croppedFile
    characterImagePreview.value = URL.createObjectURL(croppedFile)
    if (characterFileInput.value) characterFileInput.value.value = ''
  } else if (cropperTarget.value === 'edit') {
    if (editImagePreview.value) {
      URL.revokeObjectURL(editImagePreview.value)
    }
    editImageFile.value = croppedFile
    editImagePreview.value = URL.createObjectURL(croppedFile)
    if (editFileInput.value) editFileInput.value.value = ''
  }

  cropperTarget.value = null
}

function handleCropCancel() {
  if (cropperSourceSrc.value) {
    URL.revokeObjectURL(cropperSourceSrc.value)
    cropperSourceSrc.value = null
  }
  if (cropperTarget.value === 'create' && characterFileInput.value) {
    characterFileInput.value.value = ''
  }
  if (cropperTarget.value === 'edit' && editFileInput.value) {
    editFileInput.value.value = ''
  }
  cropperTarget.value = null
}

// ─── SUBMIT ───────────────────────────────────────────────────────────────────

function resetCharacterForm() {
  characterName.value = ''
  if (characterImagePreview.value) {
    URL.revokeObjectURL(characterImagePreview.value)
  }
  characterImageFile.value = null
  characterImagePreview.value = null
  forbiddenWords.value = ['', '', '']
  levelAffectation.value = null
}

const handleSubmit = async () => {
  error.value = null
  success.value = null

  const words = forbiddenWords.value.map(w => w.trim()).filter(Boolean)

  if (!characterName.value.trim()) {
    error.value = 'Le nom du personnage est requis.'
    return
  }

  if (words.length !== 3) {
    error.value = 'Il faut exactement 3 mots interdits.'
    return
  }

  if (!levelAffectation.value || levelAffectation.value < 1) {
    error.value = "Le niveau d'affectation est requis."
    return
  }

  if (isNewUniverse.value && !newUniverseName.value.trim()) {
    error.value = "Le nom du nouvel univers est requis."
    return
  }

  if (!isNewUniverse.value && !selectedUniverseId.value) {
    error.value = 'Choisis un univers existant ou crées-en un nouveau.'
    return
  }

  submitting.value = true

  try {
    let universeId = selectedUniverseId.value

    if (isNewUniverse.value) {
      const res = await characterService.createUniverse(newUniverseName.value.trim())
      universeId = res.data.universe.id
    }

    await characterService.createCharacter(universeId, {
      name: characterName.value.trim(),
      imageFile: characterImageFile.value,
      forbiddenWords: words,
      levelAffectation: levelAffectation.value,
    })

    success.value = 'Personnage créé avec succès !'
    resetCharacterForm()

    if (isNewUniverse.value) {
      isNewUniverse.value = false
      newUniverseName.value = ''
    }

    await loadUniverses()
    await loadCharacters()
    selectedUniverseId.value = universeId
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors de la création du personnage.'
  } finally {
    submitting.value = false
  }
}

// ─── ÉDITION ──────────────────────────────────────────────────────────────────

function openEditModal(character) {
  editingCharacterId.value = character.id
  editName.value = character.name
  editForbiddenWords.value = [...character.forbidden_words]
  editLevelAffectation.value = character.level_affectation
  editImageFile.value = null
  editImagePreview.value = null
  editCurrentImage.value = character.image
  editError.value = null
  showEditModal.value = true
}

function onEditImageChange(event) {
  const file = event.target.files[0] ?? null
  if (!file) return
  cropperSourceSrc.value = URL.createObjectURL(file)
  cropperTarget.value = 'edit'
  showCropperModal.value = true
}

const handleUpdateCharacter = async () => {
  editError.value = null

  const words = editForbiddenWords.value.map(w => w.trim()).filter(Boolean)

  if (!editName.value.trim()) {
    editError.value = 'Le nom du personnage est requis.'
    return
  }

  if (words.length !== 3) {
    editError.value = 'Il faut exactement 3 mots interdits.'
    return
  }

  if (!editLevelAffectation.value || editLevelAffectation.value < 1) {
    editError.value = "Le niveau d'affectation est requis."
    return
  }

  editSubmitting.value = true

  try {
    await characterService.updateCharacter(editingCharacterId.value, {
      name: editName.value.trim(),
      imageFile: editImageFile.value,
      forbiddenWords: words,
      levelAffectation: editLevelAffectation.value,
    })

    success.value = 'Personnage modifié avec succès !'
    showEditModal.value = false
    if (editImagePreview.value) {
      URL.revokeObjectURL(editImagePreview.value)
    }

    await loadCharacters()
  } catch (e) {
    editError.value = e.response?.data?.message || 'Erreur lors de la modification du personnage.'
  } finally {
    editSubmitting.value = false
  }
}

// ─── SUPPRESSION ──────────────────────────────────────────────────────────────

function openDeleteModal(character) {
  deletingCharacter.value = character
  deleteError.value = null
  showDeleteModal.value = true
}

const handleDeleteCharacter = async () => {
  deleteError.value = null
  deleteSubmitting.value = true

  try {
    await characterService.deleteCharacter(deletingCharacter.value.id)
    showDeleteModal.value = false

    await loadUniverses()
    await loadCharacters()
  } catch (e) {
    deleteError.value = e.response?.data?.message || 'Erreur lors de la suppression du personnage.'
  } finally {
    deleteSubmitting.value = false
  }
}
</script>

<template>
  <div class="arcade-page arcade-bg">
    <BContainer>
      <div class="arcade-title-wrap">
        <h1 class="arcade-title">Créer un personnage</h1>
        <p class="arcade-subtitle">Ajoute un personnage à un univers existant, ou crées-en un nouveau.</p>
      </div>

      <div v-if="error" class="arcade-alert">
        <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
      </div>
      <div v-if="success" class="arcade-alert arcade-alert--success">
        <i class="fa-solid fa-circle-check"></i> {{ success }}
      </div>

      <div class="board-card">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>

        <h2 class="board-card__title">Univers</h2>

        <div class="mb-3 text-center">
          <button
              type="button"
              class="cabinet-btn cabinet-btn--sm"
              :class="{ 'cabinet-btn--ghost': isNewUniverse }"
              @click="isNewUniverse = false"
          >
            Univers existant
          </button>
          <button
              type="button"
              class="cabinet-btn cabinet-btn--sm"
              :class="{ 'cabinet-btn--ghost': !isNewUniverse }"
              @click="isNewUniverse = true"
          >
            Nouvel univers
          </button>
        </div>

        <div v-if="!isNewUniverse" class="mb-3">
          <label class="form-label">Univers :</label>
          <BFormSelect
              v-model="selectedUniverseId"
              :options="universeOptions"
              :disabled="loadingUniverses"
          />
        </div>

        <template v-else>
          <div class="mb-3">
            <label class="form-label">Nom de l'univers :</label>
            <BFormInput v-model="newUniverseName" placeholder="Ex : Harry Potter" />
          </div>
        </template>

        <hr />

        <h2 class="board-card__title">Personnage</h2>

        <div class="mb-3">
          <label class="form-label">Nom du personnage :</label>
          <BFormInput v-model="characterName" placeholder="Ex : Harry Potter" />
        </div>

        <div class="mb-3">
          <label class="form-label">Image (facultatif) :</label>
          <input ref="characterFileInput" type="file" accept="image/*" class="form-control" @change="onCharacterImageChange" />
          <img v-if="characterImagePreview" :src="characterImagePreview" alt="" class="image-preview" />
        </div>

        <div class="mb-3">
          <label class="form-label">Les 3 mots interdits :</label>
          <div
              v-for="(word, index) in forbiddenWords"
              :key="index"
              class="forbidden-word-row"
          >
            <BFormInput
                v-model="forbiddenWords[index]"
                :placeholder="`Mot interdit ${index + 1}`"
            />
          </div>
        </div>

        <div class="mb-3">
          <label class="form-label">Niveau d'affectation :</label>
          <BFormInput
              v-model.number="levelAffectation"
              type="number"
              min="1"
              placeholder="Ex : 1"
          />
          <small class="form-text text-muted">
            Relie ce personnage à un autre personnage du même univers partageant le même niveau (2 max par niveau) : ensemble, ils formeront un binome en jeu.
          </small>
        </div>

        <div class="text-center mt-4">
          <button type="button" class="play-btn" :disabled="submitting" @click="handleSubmit">
            <i class="fa-solid fa-floppy-disk"></i> Créer le personnage
          </button>
        </div>
      </div>

      <!-- Liste des personnages existants -->
      <div class="board-card characters-list-card">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>

        <h2 class="board-card__title">Personnages existants</h2>

        <p v-if="loadingCharacters" class="text-center text-muted">Chargement…</p>
        <p v-else-if="characters.length === 0" class="text-center text-muted">Aucun personnage pour le moment.</p>

        <template v-else>
        <div class="character-row" v-for="character in characters" :key="character.id">
          <img v-if="character.image" :src="character.image" alt="" class="character-row__avatar" />
          <div v-else class="character-row__avatar character-row__avatar--placeholder">
            <i class="fa-solid fa-user-astronaut"></i>
          </div>
          <div class="character-row__info">
            <div class="character-row__name">{{ character.name }}</div>
            <div class="character-row__universe">{{ character.universe_name }}</div>
            <div class="character-row__words">
              <span v-for="word in character.forbidden_words" :key="word" class="word-badge">{{ word }}</span>
              <span class="word-badge word-badge--level">Niveau {{ character.level_affectation }}</span>
            </div>
          </div>
          <div class="character-row__actions">
            <button type="button" class="cabinet-btn cabinet-btn--sm" @click="openEditModal(character)">
              <i class="fa-solid fa-pen"></i>
            </button>
            <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" @click="openDeleteModal(character)">
              <i class="fa-solid fa-trash"></i>
            </button>
          </div>
        </div>
        </template>
      </div>

      <!-- Modal édition -->
      <BModal v-model="showEditModal" title="Modifier le personnage" no-footer class="arcade-modal">
        <div v-if="editError" class="arcade-alert mb-3">
          <i class="fa-solid fa-triangle-exclamation"></i> {{ editError }}
        </div>

        <div class="mb-3">
          <label class="form-label">Nom du personnage :</label>
          <BFormInput v-model="editName" placeholder="Ex : Harry Potter" />
        </div>

        <div class="mb-3">
          <label class="form-label">Image (facultatif) :</label>
          <input ref="editFileInput" type="file" accept="image/*" class="form-control" @change="onEditImageChange" />
          <img
              v-if="editImagePreview || editCurrentImage"
              :src="editImagePreview || editCurrentImage"
              alt=""
              class="image-preview"
          />
        </div>

        <div class="mb-3">
          <label class="form-label">Les 3 mots interdits :</label>
          <div
              v-for="(word, index) in editForbiddenWords"
              :key="index"
              class="forbidden-word-row"
          >
            <BFormInput
                v-model="editForbiddenWords[index]"
                :placeholder="`Mot interdit ${index + 1}`"
            />
          </div>
        </div>

        <div class="mb-3">
          <label class="form-label">Niveau d'affectation :</label>
          <BFormInput
              v-model.number="editLevelAffectation"
              type="number"
              min="1"
              placeholder="Ex : 1"
          />
        </div>

        <div class="text-center">
          <button type="button" class="cabinet-btn cabinet-btn--sm" :disabled="editSubmitting" @click="handleUpdateCharacter">
            Enregistrer
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showEditModal = false">
            Annuler
          </button>
        </div>
      </BModal>

      <!-- Modal suppression -->
      <BModal v-model="showDeleteModal" title="Supprimer le personnage" no-footer class="arcade-modal">
        <div v-if="deleteError" class="arcade-alert mb-3">
          <i class="fa-solid fa-triangle-exclamation"></i> {{ deleteError }}
        </div>
        <p class="text-center">
          Es-tu sûr de vouloir supprimer <strong>{{ deletingCharacter?.name }}</strong> ?
        </p>
        <div class="text-center mt-3 btn-modal">
          <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" :disabled="deleteSubmitting" @click="handleDeleteCharacter">
            <i class="fa-solid fa-trash"></i> Supprimer
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showDeleteModal = false">
            Annuler
          </button>
        </div>
      </BModal>

      <ImageCropperModal
          v-model="showCropperModal"
          :image-src="cropperSourceSrc"
          @cropped="handleCropped"
          @cancel="handleCropCancel"
      />
    </BContainer>
  </div>
</template>

<style scoped>
.arcade-page {
  font-family: 'Baloo 2', system-ui, sans-serif;
  min-height: 100vh;
  padding-top: 1.5rem;
  padding-bottom: 2rem;
}

.arcade-title-wrap {
  text-align: center;
  margin: 2rem 0 2.5rem;
}

.arcade-title {
  font-size: 1.9rem;
  margin-top: 0.75rem;
}

.board-card {
  max-width: 560px;
  margin: 0 auto 1.5rem;
}

.forbidden-word-row {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}

.forbidden-word-row :deep(.form-control) {
  flex: 1;
}

.arcade-alert--success {
  background: #1f4a2a;
  border-color: #3a3;
  color: #d7ffd7;
}

.image-preview {
  display: block;
  max-width: 120px;
  max-height: 120px;
  object-fit: cover;
  border-radius: 10px;
  margin-top: 0.5rem;
  border: 2px solid var(--arcade-blue-grey-dark);
}

.characters-list-card {
  max-width: 700px;
}

.character-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.75rem 0;
  border-bottom: 1px solid rgba(0, 0, 0, 0.1);
}

.character-row:last-child {
  border-bottom: none;
}

.character-row__avatar {
  width: 56px;
  height: 56px;
  min-width: 56px;
  border-radius: 10px;
  object-fit: cover;
  border: 2px solid var(--arcade-blue-grey-dark);
}

.character-row__avatar--placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
  font-size: 1.4rem;
}

.character-row__info {
  flex: 1;
  min-width: 0;
}

.character-row__name {
  font-weight: 700;
}

.character-row__universe {
  font-size: 0.8rem;
  color: var(--arcade-taupe);
  margin-bottom: 0.35rem;
}

.character-row__words {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.word-badge {
  font-size: 0.7rem;
  font-weight: 700;
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
  padding: 0.15rem 0.5rem;
  border-radius: 6px;
}

.word-badge--level {
  background: var(--arcade-taupe);
}

.character-row__actions {
  display: flex;
  gap: 0.4rem;
}
</style>
