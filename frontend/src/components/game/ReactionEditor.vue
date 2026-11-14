<script setup>
import {ref, watch} from 'vue'
import {BModal, BSpinner} from 'bootstrap-vue-next'
import ImageCropperModal from '../cropper/ImageCropperModal.vue'
import {reactionService} from '../../services/reactionService'
import {adminService} from '../../services/adminService'
import {canTakePhoto, readAsDataUrl, usePhotoCapture} from '../../composables/usePhotoCapture'

// Création d'un emoji de réaction : selfie ou image de la bibliothèque,
// recadrage carré (JPEG 128 px, ~8 ko), puis un nom. L'emoji est aussitôt
// disponible pour tous les joueurs ; un administrateur peut le masquer.
// Réutilisé par l'onglet Emojis de l'admin (prop `admin`).

const OUTPUT_SIZE = 128
const QUALITY = 0.85

const show = defineModel({type: Boolean, default: false})

const props = defineProps({
  playerId: {type: [Number, String], default: null},
  // Création depuis l'admin : POST /admin/reactions, sans joueur
  admin: {type: Boolean, default: false},
})

const emit = defineEmits(['created'])

const {
  cameraInput, fileInput, video, webcamOn, cropSrc, showCropper, error,
  takePhoto, importPhoto, onFile, captureWebcam, stopWebcam,
} = usePhotoCapture()

const preview = ref(null) // data URI recadrée, envoyée telle quelle
const name = ref('')
const saving = ref(false)

async function onCropped(file) {
  preview.value = await readAsDataUrl(file)
}

async function save() {
  if (!preview.value || !name.value.trim() || saving.value) return
  saving.value = true
  error.value = null
  try {
    const reaction = props.admin
        ? (await adminService.createReaction({name: name.value.trim(), image: preview.value})).data.reaction
        : await reactionService.create(props.playerId, name.value.trim(), preview.value)
    emit('created', reaction)
    show.value = false
  } catch (e) {
    error.value = e.response?.status === 429
        ? 'Trop de créations : réessaie dans une minute.'
        : e.response?.data?.message || "L'emoji n'a pas pu être créé."
  } finally {
    saving.value = false
  }
}

watch(show, (open) => {
  if (!open) {
    stopWebcam()
    return
  }
  error.value = null
  preview.value = null
  name.value = ''
})
</script>

<template>
  <BModal v-model="show" title="✨ Nouvel emoji" no-footer class="arcade-modal" centered>
    <div class="reaction-editor">
      <div v-if="webcamOn" class="reaction-editor__webcam">
        <video ref="video" autoplay playsinline muted class="reaction-editor__video"></video>
        <div class="reaction-editor__actions">
          <button type="button" class="cabinet-btn cabinet-btn--sm" @click="captureWebcam">
            <i class="fa-solid fa-camera"></i> Capturer
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="stopWebcam">
            Annuler
          </button>
        </div>
      </div>

      <template v-else>
        <div class="reaction-editor__preview">
          <img v-if="preview" :src="preview" alt="Aperçu de l'emoji" />
          <i v-else class="fa-regular fa-face-grin-stars"></i>
        </div>
        <p class="reaction-editor__hint">
          {{ admin ? 'Prends une photo' : 'Prends-toi en photo' }} ou choisis une image :
          elle rejoindra la barre de réactions de tous les joueurs. Ne prends personne en photo
          sans son accord.
          <router-link :to="{ name: 'Legal' }" target="_blank">Tes données</router-link>
        </p>

        <div v-if="error" class="reaction-editor__error" role="alert">
          <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
        </div>

        <div class="reaction-editor__actions">
          <button v-if="canTakePhoto" type="button" class="cabinet-btn cabinet-btn--sm"
                  :class="{ 'cabinet-btn--ghost': preview }" :disabled="saving" @click="takePhoto">
            <i class="fa-solid fa-camera"></i> {{ preview ? 'Reprendre' : 'Prendre une photo' }}
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--sm"
                  :class="{ 'cabinet-btn--ghost': preview }" :disabled="saving" @click="importPhoto">
            <i class="fa-solid fa-image"></i> {{ preview ? 'Changer' : 'Importer une image' }}
          </button>
        </div>

        <form v-if="preview" class="reaction-editor__form" @submit.prevent="save">
          <label class="reaction-editor__label" for="reaction-name">Nom de l'emoji</label>
          <input id="reaction-name" v-model="name" type="text" maxlength="20"
                 class="form-control" placeholder="ex : Killian choqué" autocomplete="off" />
          <button type="submit" class="cabinet-btn cabinet-btn--sm" :disabled="saving || !name.trim()">
            <BSpinner v-if="saving" small /><i v-else class="fa-solid fa-check"></i> Créer l'emoji
          </button>
        </form>
      </template>

      <input ref="cameraInput" type="file" accept="image/*" capture="user" class="d-none" @change="onFile" />
      <input ref="fileInput" type="file" accept="image/*" class="d-none" @change="onFile" />
    </div>
  </BModal>

  <ImageCropperModal
      v-model="showCropper"
      :image-src="cropSrc"
      :output-size="OUTPUT_SIZE"
      :quality="QUALITY"
      @cropped="onCropped"
  />
</template>

<style scoped>
.reaction-editor {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.9rem;
  text-align: center;
}

.reaction-editor__preview {
  width: 96px;
  height: 96px;
  border-radius: 50%;
  overflow: hidden;
  display: grid;
  place-items: center;
  background: var(--arcade-taupe);
  color: #fff;
  font-size: 2.6rem;
  border: 4px solid var(--arcade-blue-grey-dark);
}

.reaction-editor__preview img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.reaction-editor__hint {
  margin: 0;
  font-size: 0.85rem;
  color: #6c6259;
}

.reaction-editor__hint a {
  color: inherit;
  font-weight: 700;
  text-decoration: underline;
}

.reaction-editor__error {
  width: 100%;
  padding: 0.5rem 0.75rem;
  font-size: 0.85rem;
  color: #7a2f2b;
  background: rgba(179, 69, 63, 0.12);
  border: 2px solid var(--arcade-danger);
  border-radius: 8px;
}

.reaction-editor__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.5rem;
}

.reaction-editor__form {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
}

.reaction-editor__label {
  font-weight: 700;
  font-size: 0.85rem;
}

.reaction-editor__webcam {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  width: 100%;
}

.reaction-editor__video {
  width: 100%;
  max-width: 320px;
  border-radius: 12px;
  transform: scaleX(-1);
  background: #000;
}
</style>
