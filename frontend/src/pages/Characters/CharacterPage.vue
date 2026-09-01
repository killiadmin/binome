<script setup>
import { ref, onMounted, computed } from 'vue'
import { characterService } from '../../services/characterService'
import { BContainer, BFormInput, BFormSelect } from 'bootstrap-vue-next'
import ImageCropperModal from '../../components/cropper/ImageCropperModal.vue'
import ImageDropzone from '../../components/cropper/ImageDropzone.vue'

// ─── STATE ────────────────────────────────────────────────────────────────────

const universes = ref([])
const loadingUniverses = ref(true)
const cosmosList = ref([])
const loadingCosmos = ref(true)
const error = ref(null)
const success = ref(null)
const submitting = ref(false)

const isNewUniverse = ref(false)
const selectedUniverseId = ref(null)
const newUniverseName = ref('')

// Cosmos rattaché au nouvel univers (sur-catégorie : ex. « Disney », « Animé Japonais »)
const isNewCosmos = ref(false)
const selectedCosmosId = ref(null)
const newCosmosName = ref('')

const characterName = ref('')
const characterImageFile = ref(null)
const characterImagePreview = ref(null)
const forbiddenWords = ref(['', '', ''])
const levelAffectation = ref(null)

const showCropperModal = ref(false)
const cropperSourceSrc = ref(null)

const universeOptions = computed(() => [
  { value: null, text: 'Choisir un univers existant…' },
  ...universes.value.map(u => ({
    value: u.id,
    text: `${u.name}${u.cosmos_name ? ` · ${u.cosmos_name}` : ''} (${u.characters_count} personnage${u.characters_count > 1 ? 's' : ''})`,
  })),
])

const cosmosOptions = computed(() => [
  { value: null, text: 'Choisir un cosmos existant…' },
  ...cosmosList.value.map(c => ({
    value: c.id,
    text: `${c.name} (${c.universes_count} univers)`,
  })),
])

const selectedUniverse = computed(
  () => universes.value.find(u => u.id === selectedUniverseId.value) ?? null,
)

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

async function loadCosmos() {
  loadingCosmos.value = true
  try {
    const res = await characterService.listCosmos()
    cosmosList.value = res.data.cosmos
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les cosmos.'
  } finally {
    loadingCosmos.value = false
  }
}

onMounted(() => {
  loadUniverses()
  loadCosmos()
})

// ─── IMAGE UPLOAD & RECADRAGE ─────────────────────────────────────────────────

function onCharacterFile(file) {
  if (!file) return
  cropperSourceSrc.value = URL.createObjectURL(file)
  showCropperModal.value = true
}

function handleCropped(croppedFile) {
  if (cropperSourceSrc.value) {
    URL.revokeObjectURL(cropperSourceSrc.value)
    cropperSourceSrc.value = null
  }
  if (characterImagePreview.value) {
    URL.revokeObjectURL(characterImagePreview.value)
  }
  characterImageFile.value = croppedFile
  characterImagePreview.value = URL.createObjectURL(croppedFile)
}

function handleCropCancel() {
  if (cropperSourceSrc.value) {
    URL.revokeObjectURL(cropperSourceSrc.value)
    cropperSourceSrc.value = null
  }
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

  if (isNewUniverse.value && isNewCosmos.value && !newCosmosName.value.trim()) {
    error.value = 'Le nom du nouveau cosmos est requis.'
    return
  }

  if (isNewUniverse.value && !isNewCosmos.value && !selectedCosmosId.value) {
    error.value = 'Choisis un cosmos existant ou crées-en un nouveau pour ce nouvel univers.'
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
      let cosmosId = selectedCosmosId.value

      if (isNewCosmos.value) {
        const cosmosRes = await characterService.createCosmos(newCosmosName.value.trim())
        cosmosId = cosmosRes.data.cosmos.id
      }

      const res = await characterService.createUniverse(newUniverseName.value.trim(), cosmosId)
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
      isNewCosmos.value = false
      newCosmosName.value = ''
      selectedCosmosId.value = null
    }

    await loadUniverses()
    await loadCosmos()
    selectedUniverseId.value = universeId
  } catch (e) {
    error.value = e.response?.data?.message || 'Erreur lors de la création du personnage.'
  } finally {
    submitting.value = false
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

      <div class="text-center mb-4">
        <router-link to="/characters/list" class="cabinet-btn cabinet-btn--sm">
          <i class="fa-solid fa-list"></i> Voir tous les personnages
        </router-link>
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
          <small v-if="selectedUniverse" class="form-text text-muted">
            Cosmos :
            <strong>{{ selectedUniverse.cosmos_name || 'aucun (modifiable via un personnage de cet univers)' }}</strong>
          </small>
        </div>

        <template v-else>
          <div class="mb-3">
            <label class="form-label">Nom de l'univers :</label>
            <BFormInput v-model="newUniverseName" placeholder="Ex : Raiponce" />
          </div>

          <div class="mb-3">
            <label class="form-label">Cosmos (sur-catégorie) :</label>

            <div class="mb-2 text-center">
              <button
                  type="button"
                  class="cabinet-btn cabinet-btn--sm"
                  :class="{ 'cabinet-btn--ghost': isNewCosmos }"
                  @click="isNewCosmos = false"
              >
                Cosmos existant
              </button>
              <button
                  type="button"
                  class="cabinet-btn cabinet-btn--sm"
                  :class="{ 'cabinet-btn--ghost': !isNewCosmos }"
                  @click="isNewCosmos = true"
              >
                Nouveau cosmos
              </button>
            </div>

            <BFormSelect
                v-if="!isNewCosmos"
                v-model="selectedCosmosId"
                :options="cosmosOptions"
                :disabled="loadingCosmos"
            />
            <BFormInput
                v-else
                v-model="newCosmosName"
                placeholder="Ex : Disney"
            />
            <small class="form-text text-muted">
              Le cosmos regroupe plusieurs univers (ex : « Disney » → Raiponce, La Reine des Neiges ; « Animé Japonais » → Pokémon, Naruto).
            </small>
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
          <ImageDropzone @file="onCharacterFile" />
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
  margin: 2rem 0 1.5rem;
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
</style>
