<script setup>
import {computed, ref, watch} from 'vue'
import {BModal, BSpinner} from 'bootstrap-vue-next'
import ImageCropperModal from '../cropper/ImageCropperModal.vue'
import PlayerAvatar from './PlayerAvatar.vue'
import {avatarService} from '../../services/avatarService'
import {canTakePhoto, readAsDataUrl, usePhotoCapture} from '../../composables/usePhotoCapture'

// Choix de la photo de profil : prise de vue ou import, recadrage carré,
// puis envoi (JPEG 256 px, ~20 ko). La photo est rattachée au pseudo côté
// serveur : le joueur la retrouve à chaque partie.

const OUTPUT_SIZE = 256
const QUALITY = 0.85

const show = defineModel({type: Boolean, default: false})

const props = defineProps({
  player: {type: Object, default: null}, // { id, pseudo, avatar_url }
})

const emit = defineEmits(['updated'])

const {
  cameraInput, fileInput, video, webcamOn, cropSrc, showCropper, error,
  takePhoto, importPhoto, onFile, captureWebcam, stopWebcam,
} = usePhotoCapture()

const saving = ref(false)

const hasAvatar = computed(() => !!props.player?.avatar_url)

async function onCropped(file) {
  saving.value = true
  error.value = null
  try {
    const url = await avatarService.upload(props.player.id, await readAsDataUrl(file))
    emit('updated', url)
    show.value = false
  } catch (e) {
    error.value = e.response?.status === 429
        ? 'Trop de changements : réessaie dans une minute.'
        : e.response?.data?.message || "La photo n'a pas pu être enregistrée."
  } finally {
    saving.value = false
  }
}

async function removePhoto() {
  saving.value = true
  error.value = null
  try {
    await avatarService.remove(props.player.id)
    emit('updated', null)
    show.value = false
  } catch (e) {
    error.value = e.response?.data?.message || "La photo n'a pas pu être supprimée."
  } finally {
    saving.value = false
  }
}

watch(show, (open) => {
  if (!open) stopWebcam()
  else error.value = null
})
</script>

<template>
  <BModal v-model="show" title="📸 Ma photo" no-footer class="arcade-modal" centered>
    <div class="avatar-editor">
      <!-- Aperçu webcam (ordinateur) ou photo actuelle -->
      <div v-if="webcamOn" class="avatar-editor__webcam">
        <video ref="video" autoplay playsinline muted class="avatar-editor__video"></video>
        <div class="avatar-editor__actions">
          <button type="button" class="cabinet-btn cabinet-btn--sm" @click="captureWebcam">
            <i class="fa-solid fa-camera"></i> Capturer
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="stopWebcam">
            Annuler
          </button>
        </div>
      </div>

      <template v-else>
        <div class="avatar-editor__preview">
          <PlayerAvatar :pseudo="player?.pseudo" :url="player?.avatar_url" :letters="1" />
        </div>
        <p class="avatar-editor__hint">
          Ta photo remplace ton initiale partout dans le jeu.
          Elle est liée à ton pseudo : tu la retrouveras à ta prochaine partie.
        </p>

        <div v-if="error" class="avatar-editor__error" role="alert">
          <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
        </div>

        <div class="avatar-editor__actions">
          <button v-if="canTakePhoto" type="button" class="cabinet-btn cabinet-btn--sm"
                  :disabled="saving" @click="takePhoto">
            <i class="fa-solid fa-camera"></i> Prendre une photo
          </button>
          <button type="button" class="cabinet-btn cabinet-btn--sm" :disabled="saving" @click="importPhoto">
            <i class="fa-solid fa-image"></i> Importer une photo
          </button>
          <button v-if="hasAvatar" type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm"
                  :disabled="saving" @click="removePhoto">
            <i class="fa-solid fa-trash"></i> Retirer
          </button>
        </div>
        <p v-if="saving" class="avatar-editor__saving"><BSpinner small /> Enregistrement…</p>
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
.avatar-editor {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.9rem;
  text-align: center;
}

.avatar-editor__preview {
  width: 120px;
  height: 120px;
  border-radius: 50%;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--arcade-taupe);
  color: #fff;
  font-family: 'Press Start 2P', cursive;
  font-size: 2.2rem;
  border: 4px solid var(--arcade-blue-grey-dark);
  box-shadow: inset 0 -5px 0 rgba(0, 0, 0, 0.2);
}

.avatar-editor__hint {
  margin: 0;
  font-size: 0.85rem;
  color: #6c6259;
}

.avatar-editor__error {
  width: 100%;
  padding: 0.5rem 0.75rem;
  font-size: 0.85rem;
  color: #7a2f2b;
  background: rgba(179, 69, 63, 0.12);
  border: 2px solid var(--arcade-danger);
  border-radius: 8px;
}

.avatar-editor__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.5rem;
}

.avatar-editor__saving {
  margin: 0;
  font-size: 0.85rem;
}

.avatar-editor__webcam {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  width: 100%;
}

.avatar-editor__video {
  width: 100%;
  max-width: 320px;
  border-radius: 12px;
  /* Aperçu miroir, comme une caméra frontale */
  transform: scaleX(-1);
  background: #000;
}
</style>
