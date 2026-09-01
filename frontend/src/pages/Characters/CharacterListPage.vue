<script setup>
import { ref, reactive, onMounted, computed, watch } from 'vue'
import { characterService } from '../../services/characterService'
import { BContainer, BFormInput, BFormSelect, BModal } from 'bootstrap-vue-next'
import ImageCropperModal from '../../components/cropper/ImageCropperModal.vue'
import ImageDropzone from '../../components/cropper/ImageDropzone.vue'

// ─── STATE ────────────────────────────────────────────────────────────────────

const characters = ref([])
const loadingCharacters = ref(true)
const universes = ref([])
const loadingUniverses = ref(true)
const cosmosList = ref([])
const loadingCosmos = ref(true)
const error = ref(null)
const success = ref(null)

// ─── FILTRES ──────────────────────────────────────────────────────────────────

const searchQuery = ref('')
const statusFilter = reactive({ validated: true, pending: true })
const selectedCosmosFilters = ref(null) // null tant que non initialisé (= tout coché)

const cosmosFilterOptions = computed(() => {
  const seen = new Map()
  for (const c of characters.value) {
    const key = c.cosmos_name || '__none__'
    if (!seen.has(key)) seen.set(key, c.cosmos_name || 'Sans cosmos')
  }
  return [...seen.entries()]
    .map(([value, text]) => ({ value, text }))
    .sort((a, b) => {
      if (a.value === '__none__') return 1
      if (b.value === '__none__') return -1
      return a.text.localeCompare(b.text)
    })
})

// Initialise le filtre cosmos (tout coché) une fois les personnages chargés.
watch(cosmosFilterOptions, (opts) => {
  if (selectedCosmosFilters.value === null && opts.length > 0) {
    selectedCosmosFilters.value = opts.map(o => o.value)
  }
})

const filteredCharacters = computed(() => {
  const search = searchQuery.value.trim().toLowerCase()

  return characters.value.filter((c) => {
    if (c.verif_manual && !statusFilter.validated) return false
    if (!c.verif_manual && !statusFilter.pending) return false

    if (selectedCosmosFilters.value) {
      const cosmosKey = c.cosmos_name || '__none__'
      if (!selectedCosmosFilters.value.includes(cosmosKey)) return false
    }

    if (search) {
      const haystack = `${c.name} ${c.universe_name} ${c.cosmos_name ?? ''}`.toLowerCase()
      if (!haystack.includes(search)) return false
    }

    return true
  })
})

// Regroupe par cosmos → univers → niveau d'affectation (= binôme potentiel), trié.
const groupedCosmos = computed(() => {
  const byCosmos = new Map()

  for (const c of filteredCharacters.value) {
    const cosmosName = c.cosmos_name || 'Sans cosmos'
    if (!byCosmos.has(cosmosName)) byCosmos.set(cosmosName, new Map())
    const universeMap = byCosmos.get(cosmosName)

    if (!universeMap.has(c.universe_id)) {
      universeMap.set(c.universe_id, { id: c.universe_id, name: c.universe_name, characters: [] })
    }
    universeMap.get(c.universe_id).characters.push(c)
  }

  return [...byCosmos.entries()]
    .sort(([a], [b]) => {
      if (a === 'Sans cosmos') return 1
      if (b === 'Sans cosmos') return -1
      return a.localeCompare(b)
    })
    .map(([name, universeMap]) => ({
      name,
      universes: [...universeMap.values()]
        .sort((a, b) => a.name.localeCompare(b.name))
        .map(u => ({ ...u, levelGroups: groupByLevel(u.characters) })),
    }))
})

function groupByLevel(chars) {
  const byLevel = new Map()
  for (const c of chars) {
    if (!byLevel.has(c.level_affectation)) byLevel.set(c.level_affectation, [])
    byLevel.get(c.level_affectation).push(c)
  }

  return [...byLevel.entries()]
    .sort(([a], [b]) => a - b)
    .map(([level, levelCharacters]) => ({
      level,
      isBinome: levelCharacters.length >= 2,
      characters: [...levelCharacters].sort((a, b) => a.name.localeCompare(b.name)),
    }))
}

// ─── ÉDITION ──────────────────────────────────────────────────────────────────

const showEditModal = ref(false)
const editingCharacterId = ref(null)
const editUniverseId = ref(null)
const editCosmosId = ref(null)
const editOriginalCosmosId = ref(null)
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

const showCropperModal = ref(false)
const cropperSourceSrc = ref(null)
const cropperTarget = ref(null) // 'edit' | 'validation'

// Cosmos éditables uniquement (sans l'option vide) pour le sélecteur de la modale d'édition
const editCosmosOptions = computed(() => [
  { value: null, text: 'Aucun cosmos' },
  ...cosmosList.value.map(c => ({ value: c.id, text: c.name })),
])

const editUniverseOptions = computed(() =>
  universes.value.map(u => ({
    value: u.id,
    text: u.cosmos_name ? `${u.name} · ${u.cosmos_name}` : u.name,
  })),
)

const editSelectedUniverse = computed(
  () => universes.value.find(u => u.id === editUniverseId.value) ?? null,
)

// Quand on change l'univers du personnage dans la modale, le cosmos affiché suit
// celui du nouvel univers (et n'est donc pas considéré comme "modifié" tant qu'on n'y touche pas).
watch(editUniverseId, (id) => {
  const u = universes.value.find(x => x.id === id)
  editCosmosId.value = u?.cosmos_id ?? null
  editOriginalCosmosId.value = u?.cosmos_id ?? null
})

function openEditModal(character) {
  editingCharacterId.value = character.id
  editUniverseId.value = character.universe_id
  editCosmosId.value = character.cosmos_id ?? null
  editOriginalCosmosId.value = character.cosmos_id ?? null
  editName.value = character.name
  editForbiddenWords.value = [...character.forbidden_words]
  editLevelAffectation.value = character.level_affectation
  editImageFile.value = null
  editImagePreview.value = null
  editCurrentImage.value = character.image
  editError.value = null
  showEditModal.value = true
}

function onEditFile(file) {
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

  if (!editUniverseId.value) {
    editError.value = "L'univers du personnage est requis."
    return
  }

  editSubmitting.value = true

  try {
    await characterService.updateCharacter(editingCharacterId.value, {
      name: editName.value.trim(),
      universeId: editUniverseId.value,
      imageFile: editImageFile.value,
      forbiddenWords: words,
      levelAffectation: editLevelAffectation.value,
    })

    // Le cosmos appartient à l'univers : on ne pousse la mise à jour que si
    // l'hôte l'a effectivement changé (impacte tous les personnages de l'univers).
    // Si l'univers a changé, editOriginalCosmosId a déjà été resynchronisé sur le nouvel univers.
    if (editCosmosId.value !== editOriginalCosmosId.value) {
      await characterService.updateUniverseCosmos(editUniverseId.value, editCosmosId.value)
    }

    success.value = 'Personnage modifié avec succès !'
    showEditModal.value = false
    if (editImagePreview.value) {
      URL.revokeObjectURL(editImagePreview.value)
    }

    await loadUniverses()
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

// ─── VALIDATION DES BINÔMES PROPOSÉS ──────────────────────────────────────────

const pendingBinomes = ref([])
const showValidationModal = ref(false)
const validationIndex = ref(0)
const validationStep = ref('review') // 'review' | 'images'
const validationImageIndex = ref(0)
const validationError = ref(null)
const validationBusy = ref(false)

const currentBinome = computed(() => pendingBinomes.value[validationIndex.value] ?? null)
const currentValidationCharacter = computed(
  () => currentBinome.value?.characters[validationImageIndex.value] ?? null,
)

async function loadPendingBinomes({ openIfAny = false } = {}) {
  try {
    const res = await characterService.listPendingBinomes()
    // copie locale éditable des mots interdits + snapshot pour détecter les changements
    pendingBinomes.value = res.data.binomes.map(b => ({
      ...b,
      characters: b.characters.map(c => ({
        ...c,
        forbidden_words: [...c.forbidden_words],
        _words0: [...c.forbidden_words],
      })),
    }))
  } catch {
    pendingBinomes.value = []
  }

  if (openIfAny && pendingBinomes.value.length > 0) {
    validationIndex.value = 0
    validationStep.value = 'review'
    validationImageIndex.value = 0
    validationError.value = null
    showValidationModal.value = true
  }
}

function openValidationModal() {
  if (pendingBinomes.value.length === 0) return
  validationIndex.value = 0
  validationStep.value = 'review'
  validationImageIndex.value = 0
  validationError.value = null
  showValidationModal.value = true
}

function advanceValidation() {
  validationImageIndex.value = 0
  validationStep.value = 'review'
  validationError.value = null

  if (validationIndex.value + 1 >= pendingBinomes.value.length) {
    showValidationModal.value = false
    pendingBinomes.value = []
    validationIndex.value = 0
    loadCharacters()
    loadUniverses()
  } else {
    validationIndex.value += 1
  }
}

async function rejectCurrentBinome() {
  if (!currentBinome.value) return
  validationBusy.value = true
  validationError.value = null
  try {
    await characterService.rejectBinome(currentBinome.value.characters.map(c => c.id))
    advanceValidation()
  } catch (e) {
    validationError.value = e.response?.data?.message || 'Erreur lors du refus du binôme.'
  } finally {
    validationBusy.value = false
  }
}

async function acceptCurrentBinome() {
  validationError.value = null

  const chars = currentBinome.value?.characters ?? []

  // Chaque personnage doit conserver exactement 3 mots interdits non vides.
  for (const c of chars) {
    if (c.forbidden_words.map(w => (w ?? '').trim()).filter(Boolean).length !== 3) {
      validationError.value = `« ${c.name} » doit avoir 3 mots interdits non vides.`
      return
    }
  }

  validationBusy.value = true
  try {
    for (const c of chars) {
      const words = c.forbidden_words.map(w => w.trim())
      if (JSON.stringify(words) !== JSON.stringify(c._words0)) {
        await characterService.updateForbiddenWords(c.id, words)
      }
      c.forbidden_words = words
      c._words0 = [...words]
    }
    validationImageIndex.value = 0
    validationStep.value = 'images'
  } catch (e) {
    validationError.value = e.response?.data?.message || 'Erreur lors de la mise à jour des mots interdits.'
  } finally {
    validationBusy.value = false
  }
}

function onValidationFile(file) {
  if (!file) return
  cropperSourceSrc.value = URL.createObjectURL(file)
  cropperTarget.value = 'validation'
  showCropperModal.value = true
}

async function uploadValidationImage(croppedFile) {
  const character = currentValidationCharacter.value
  if (!character) return
  validationBusy.value = true
  validationError.value = null
  try {
    const res = await characterService.uploadCharacterImage(character.id, croppedFile)
    character.image = res.data.character.image
  } catch (e) {
    validationError.value = e.response?.data?.message || "Erreur lors de l'import de l'image."
  } finally {
    validationBusy.value = false
  }
}

async function nextValidationImage() {
  if (!currentValidationCharacter.value?.image) {
    validationError.value = 'Importe une image pour ce personnage.'
    return
  }

  if (validationImageIndex.value + 1 < currentBinome.value.characters.length) {
    validationImageIndex.value += 1
    validationError.value = null
    return
  }

  // Toutes les images sont là : on valide le binôme.
  validationBusy.value = true
  validationError.value = null
  try {
    await characterService.acceptBinome(currentBinome.value.characters.map(c => c.id))
    advanceValidation()
  } catch (e) {
    validationError.value = e.response?.data?.message || 'Erreur lors de la validation du binôme.'
  } finally {
    validationBusy.value = false
  }
}

// ─── RECADRAGE D'IMAGE (partagé édition + validation) ─────────────────────────

function handleCropped(croppedFile) {
  if (cropperSourceSrc.value) {
    URL.revokeObjectURL(cropperSourceSrc.value)
    cropperSourceSrc.value = null
  }

  if (cropperTarget.value === 'edit') {
    if (editImagePreview.value) {
      URL.revokeObjectURL(editImagePreview.value)
    }
    editImageFile.value = croppedFile
    editImagePreview.value = URL.createObjectURL(croppedFile)
  } else if (cropperTarget.value === 'validation') {
    cropperTarget.value = null
    uploadValidationImage(croppedFile)
    return
  }

  cropperTarget.value = null
}

function handleCropCancel() {
  if (cropperSourceSrc.value) {
    URL.revokeObjectURL(cropperSourceSrc.value)
    cropperSourceSrc.value = null
  }
  cropperTarget.value = null
}

// ─── DATA LOADING ─────────────────────────────────────────────────────────────

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
  loadCharacters()
  loadUniverses()
  loadCosmos()
  loadPendingBinomes({ openIfAny: true })
})
</script>

<template>
  <div class="arcade-page arcade-bg">
    <BContainer>
      <div class="arcade-title-wrap">
        <h1 class="arcade-title">Personnages &amp; binômes</h1>
        <p class="arcade-subtitle">Tous les personnages, triés par cosmos puis univers.</p>
      </div>

      <div class="text-center mb-4 list-actions">
        <router-link to="/characters" class="play-btn">
          <i class="fa-solid fa-plus"></i> Créer un personnage
        </router-link>
        <button
            v-if="pendingBinomes.length > 0"
            type="button"
            class="cabinet-btn cabinet-btn--sm"
            @click="openValidationModal"
        >
          <i class="fa-solid fa-hourglass-half"></i>
          {{ pendingBinomes.length }} binôme{{ pendingBinomes.length > 1 ? 's' : '' }} à valider
        </button>
      </div>

      <div v-if="error" class="arcade-alert">
        <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
      </div>
      <div v-if="success" class="arcade-alert arcade-alert--success">
        <i class="fa-solid fa-circle-check"></i> {{ success }}
      </div>

      <!-- Filtres -->
      <div class="board-card filters-card">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>

        <div class="mb-3">
          <label class="form-label">Recherche :</label>
          <BFormInput v-model="searchQuery" placeholder="Nom, univers ou cosmos…" />
        </div>

        <div class="mb-3">
          <label class="form-label">Statut :</label>
          <div class="filter-checks">
            <label class="filter-check">
              <input type="checkbox" v-model="statusFilter.validated" /> Validés
            </label>
            <label class="filter-check">
              <input type="checkbox" v-model="statusFilter.pending" /> À valider
            </label>
          </div>
        </div>

        <div v-if="cosmosFilterOptions.length" class="mb-1">
          <label class="form-label">Cosmos :</label>
          <div class="filter-checks">
            <label v-for="opt in cosmosFilterOptions" :key="opt.value" class="filter-check">
              <input type="checkbox" :value="opt.value" v-model="selectedCosmosFilters" /> {{ opt.text }}
            </label>
          </div>
        </div>
      </div>

      <!-- Résultats -->
      <p v-if="loadingCharacters" class="text-center text-muted">Chargement…</p>
      <p v-else-if="groupedCosmos.length === 0" class="text-center text-muted">
        Aucun personnage ne correspond à ces filtres.
      </p>

      <div v-else class="cosmos-groups">
        <div v-for="cosmos in groupedCosmos" :key="cosmos.name" class="board-card cosmos-group">
          <div class="board-card__rivet board-card__rivet--tl"></div>
          <div class="board-card__rivet board-card__rivet--tr"></div>
          <div class="board-card__rivet board-card__rivet--bl"></div>
          <div class="board-card__rivet board-card__rivet--br"></div>

          <h2 class="board-card__title cosmos-group__title">
            <i class="fa-solid fa-globe"></i> {{ cosmos.name }}
          </h2>

          <div v-for="universe in cosmos.universes" :key="universe.id" class="universe-block">
            <h3 class="universe-block__title">{{ universe.name }}</h3>

            <div
                v-for="group in universe.levelGroups"
                :key="group.level"
                class="binome-group"
                :class="{ 'binome-group--complete': group.isBinome }"
            >
              <div class="binome-group__label">
                <span class="word-badge word-badge--level">Niveau {{ group.level }}</span>
                <span v-if="group.isBinome" class="word-badge word-badge--binome">
                  <i class="fa-solid fa-link"></i> Binôme
                </span>
                <span v-else class="word-badge word-badge--lonely">
                  <i class="fa-solid fa-user"></i> Sans binôme
                </span>
              </div>

              <div class="character-row" v-for="character in group.characters" :key="character.id">
                <img v-if="character.image" :src="character.image" alt="" class="character-row__avatar" />
                <div v-else class="character-row__avatar character-row__avatar--placeholder">
                  <i class="fa-solid fa-user-astronaut"></i>
                </div>
                <div class="character-row__info">
                  <div class="character-row__name">
                    {{ character.name }}
                    <span v-if="character.verif_manual === false" class="word-badge word-badge--pending">
                      <i class="fa-solid fa-hourglass-half"></i> à valider
                    </span>
                  </div>
                  <div class="character-row__words">
                    <span v-for="word in character.forbidden_words" :key="word" class="word-badge">{{ word }}</span>
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
            </div>
          </div>
        </div>
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
          <ImageDropzone @file="onEditFile" />
          <img
              v-if="editImagePreview || editCurrentImage"
              :src="editImagePreview || editCurrentImage"
              alt=""
              class="image-preview"
          />
        </div>

        <div class="mb-3">
          <label class="form-label">Univers :</label>
          <BFormSelect v-model="editUniverseId" :options="editUniverseOptions" :disabled="loadingUniverses" />
          <small class="form-text text-muted">
            Change l'univers auquel ce personnage est rattaché. Le niveau d'affectation devra rester valide dans le nouvel univers (2 personnages max par niveau).
          </small>
        </div>

        <div class="mb-3">
          <label class="form-label">Cosmos de l'univers :</label>
          <BFormSelect v-model="editCosmosId" :options="editCosmosOptions" :disabled="loadingCosmos" />
          <small class="form-text text-muted">
            Le modifier ici impacte <strong>tous</strong> les personnages de
            « {{ editSelectedUniverse?.name ?? 'cet univers' }} ».
            Pour créer un nouveau cosmos, utilise le formulaire de création.
          </small>
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

      <!-- Modal validation des binômes proposés -->
      <BModal
          v-model="showValidationModal"
          title="Binômes à valider"
          no-footer
          no-close-on-backdrop
          no-close-on-esc
          hide-header-close
          class="arcade-modal"
      >
        <template v-if="currentBinome">
          <p class="text-center validation-progress">
            Binôme {{ validationIndex + 1 }} / {{ pendingBinomes.length }}
          </p>

          <div v-if="validationError" class="arcade-alert mb-3">
            <i class="fa-solid fa-triangle-exclamation"></i> {{ validationError }}
          </div>

          <!-- Étape 1 : relecture -->
          <template v-if="validationStep === 'review'">
            <div class="validation-universe">
              <strong>{{ currentBinome.universe_name }}</strong>
              <span v-if="currentBinome.cosmos_name"> · {{ currentBinome.cosmos_name }}</span>
              <span class="word-badge word-badge--level">Niveau {{ currentBinome.level_affectation }}</span>
            </div>

            <div v-for="c in currentBinome.characters" :key="c.id" class="validation-char">
              <div class="validation-char__name">{{ c.name }}</div>
              <div class="validation-words">
                <div v-for="(w, i) in c.forbidden_words" :key="i" class="validation-word-row">
                  <BFormInput
                      v-model="c.forbidden_words[i]"
                      :placeholder="`Mot interdit ${i + 1}`"
                      size="sm"
                  />
                  <button
                      type="button"
                      class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm validation-word-clear"
                      title="Vider ce mot"
                      @click="c.forbidden_words[i] = ''"
                  >
                    <i class="fa-solid fa-xmark"></i>
                  </button>
                </div>
              </div>
            </div>

            <p class="text-center text-muted validation-hint">
              Tu peux corriger les mots interdits ici avant d'accepter. En acceptant, tu devras
              importer une image pour chacun des 2 personnages. Un binôme refusé ne te sera plus jamais reproposé.
            </p>

            <div class="text-center mt-3 btn-modal">
              <button type="button" class="cabinet-btn cabinet-btn--danger cabinet-btn--sm" :disabled="validationBusy" @click="rejectCurrentBinome">
                <i class="fa-solid fa-xmark"></i> Refuser
              </button>
              <button type="button" class="cabinet-btn cabinet-btn--sm" :disabled="validationBusy" @click="acceptCurrentBinome">
                <i class="fa-solid fa-check"></i> Accepter
              </button>
            </div>
          </template>

          <!-- Étape 2 : images -->
          <template v-else>
            <p class="text-center">
              Image {{ validationImageIndex + 1 }} / {{ currentBinome.characters.length }} —
              <strong>{{ currentValidationCharacter?.name }}</strong>
            </p>

            <div class="text-center my-3">
              <img
                  v-if="currentValidationCharacter?.image"
                  :src="currentValidationCharacter.image"
                  alt=""
                  class="image-preview validation-preview"
              />
              <div v-else class="text-muted">Aucune image importée</div>
            </div>

            <ImageDropzone :disabled="validationBusy" @file="onValidationFile" />

            <div class="text-center mt-3 btn-modal">
              <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" :disabled="validationBusy" @click="validationStep = 'review'">
                Retour
              </button>
              <button
                  type="button"
                  class="cabinet-btn cabinet-btn--sm"
                  :disabled="validationBusy || !currentValidationCharacter?.image"
                  @click="nextValidationImage"
              >
                {{ validationImageIndex + 1 < currentBinome.characters.length ? 'Personnage suivant' : 'Valider le binôme' }}
              </button>
            </div>
          </template>
        </template>
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
  margin: 2rem 0 1.5rem;
}

.arcade-title {
  font-size: 1.9rem;
  margin-top: 0.75rem;
}

.list-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.6rem;
}

.arcade-alert--success {
  background: #1f4a2a;
  border-color: #3a3;
  color: #d7ffd7;
}

.filters-card,
.cosmos-group {
  max-width: 760px;
  margin: 0 auto 1.5rem;
}

.filter-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem 1rem;
}

.filter-check {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
}

.filter-check input[type='checkbox'] {
  width: 1rem;
  height: 1rem;
  accent-color: var(--arcade-gold);
}

.cosmos-group__title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.universe-block {
  margin-bottom: 1.25rem;
}

.universe-block:last-child {
  margin-bottom: 0;
}

.universe-block__title {
  font-size: 1rem;
  font-weight: 700;
  border-bottom: 2px solid var(--arcade-blue-grey-dark);
  padding-bottom: 0.3rem;
  margin-bottom: 0.6rem;
}

.binome-group {
  border: 1px dashed rgba(0, 0, 0, 0.15);
  border-radius: 10px;
  padding: 0.6rem 0.75rem;
  margin-bottom: 0.6rem;
}

.binome-group--complete {
  border-style: solid;
  border-color: var(--arcade-gold);
}

.binome-group__label {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 0.5rem;
}

.word-badge--binome {
  background: var(--arcade-gold);
  color: #4a2f00;
}

.word-badge--lonely {
  background: transparent;
  color: var(--arcade-taupe);
  border: 1px solid var(--arcade-taupe);
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

.forbidden-word-row {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}

.forbidden-word-row :deep(.form-control) {
  flex: 1;
}

.character-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.5rem 0;
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

.word-badge--pending {
  background: #8a5a00;
  color: #ffe9c7;
  margin-left: 0.4rem;
  font-size: 0.65rem;
}

.character-row__actions {
  display: flex;
  gap: 0.4rem;
}

.validation-progress {
  font-weight: 700;
  opacity: 0.8;
  margin-bottom: 0.75rem;
}

.validation-universe {
  text-align: center;
  margin-bottom: 1rem;
}

.validation-universe .word-badge--level {
  margin-left: 0.5rem;
}

.validation-char {
  border: 1px solid rgba(0, 0, 0, 0.12);
  border-radius: 10px;
  padding: 0.6rem 0.75rem;
  margin-bottom: 0.6rem;
}

.validation-char__name {
  font-weight: 700;
  margin-bottom: 0.35rem;
}

.validation-words {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.validation-word-row {
  display: flex;
  gap: 0.4rem;
  align-items: center;
}

.validation-word-row :deep(.form-control) {
  flex: 1;
}

.validation-word-clear {
  flex-shrink: 0;
}

.validation-hint {
  font-size: 0.8rem;
  margin-top: 1rem;
}

.validation-preview {
  margin: 0 auto;
}
</style>
