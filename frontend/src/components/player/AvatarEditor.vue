<script setup>
import {computed, onBeforeUnmount, ref, watch, nextTick} from 'vue'
import {BModal, BSpinner} from 'bootstrap-vue-next'
import ImageCropperModal from '../cropper/ImageCropperModal.vue'
import PlayerAvatar from './PlayerAvatar.vue'
import {avatarService} from '../../services/avatarService'

// Choix de la photo de profil : prise de vue ou import, recadrage carré,
// puis envoi (JPEG 256 px, ~20 ko). La photo est rattachée au pseudo côté
// serveur : le joueur la retrouve à chaque partie.
//
// Prise de vue :
//  - téléphone : <input capture="user"> ouvre directement la caméra frontale,
//    et fonctionne aussi en http:// sur l'IP LAN ;
//  - ordinateur : getUserMedia n'existe qu'en contexte sécurisé (https ou
//    localhost) — donc pour l'hôte sur localhost, pas pour un PC en LAN.

const OUTPUT_SIZE = 256
const QUALITY = 0.85

const show = defineModel({type: Boolean, default: false})

const props = defineProps({
  player: {type: Object, default: null}, // { id, pseudo, avatar_url }
})

const emit = defineEmits(['updated'])

const isTouch = typeof window !== 'undefined' && window.matchMedia('(pointer: coarse)').matches
const canUseWebcam = typeof window !== 'undefined' && window.isSecureContext && !!navigator.mediaDevices?.getUserMedia
const canTakePhoto = isTouch || canUseWebcam

const cameraInput = ref(null)
const fileInput = ref(null)
const cropSrc = ref(null)
const showCropper = ref(false)
const saving = ref(false)
const error = ref(null)

// Webcam (ordinateur)
const webcamOn = ref(false)
const video = ref(null)
let stream = null

const hasAvatar = computed(() => !!props.player?.avatar_url)

function takePhoto() {
  error.value = null
  if (isTouch) cameraInput.value?.click()
  else startWebcam()
}

function importPhoto() {
  error.value = null
  fileInput.value?.click()
}

function onFile(event) {
  const file = event.target.files?.[0]
  event.target.value = ''
  if (!file) return
  if (!file.type.startsWith('image/')) {
    error.value = "Ce fichier n'est pas une image."
    return
  }
  openCropper(URL.createObjectURL(file))
}

function openCropper(src) {
  if (cropSrc.value) URL.revokeObjectURL(cropSrc.value)
  cropSrc.value = src
  showCropper.value = true
}

async function startWebcam() {
  try {
    stream = await navigator.mediaDevices.getUserMedia({video: {facingMode: 'user'}, audio: false})
    webcamOn.value = true
    await nextTick()
    video.value.srcObject = stream
  } catch {
    error.value = "Impossible d'accéder à la caméra (autorisation refusée ou aucune caméra)."
    stopWebcam()
  }
}

function captureWebcam() {
  const v = video.value
  if (!v?.videoWidth) return
  const canvas = document.createElement('canvas')
  canvas.width = v.videoWidth
  canvas.height = v.videoHeight
  const ctx = canvas.getContext('2d')
  // Image miroir, comme l'aperçu : c'est ce que le joueur s'attend à voir.
  ctx.translate(canvas.width, 0)
  ctx.scale(-1, 1)
  ctx.drawImage(v, 0, 0)
  stopWebcam()
  canvas.toBlob(blob => blob && openCropper(URL.createObjectURL(blob)), 'image/jpeg', 0.92)
}

function stopWebcam() {
  stream?.getTracks().forEach(t => t.stop())
  stream = null
  webcamOn.value = false
}

function readAsDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result)
    reader.onerror = reject
    reader.readAsDataURL(file)
  })
}

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

onBeforeUnmount(() => {
  stopWebcam()
  if (cropSrc.value) URL.revokeObjectURL(cropSrc.value)
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
