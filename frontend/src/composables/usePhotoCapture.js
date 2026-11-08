import {nextTick, onBeforeUnmount, ref} from 'vue'

// Prise de vue ou import d'une image, puis ouverture du recadrage carré
// (ImageCropperModal). Partagé par la photo de profil et les emojis de réaction.
//
// Prise de vue :
//  - téléphone : <input capture="user"> ouvre directement la caméra frontale,
//    et fonctionne aussi en http:// sur l'IP LAN ;
//  - ordinateur : getUserMedia n'existe qu'en contexte sécurisé (https ou
//    localhost) — donc pour l'hôte sur localhost, pas pour un PC en LAN.
//
// Le composant branche `cameraInput` / `fileInput` sur deux <input type="file">
// cachés (onFile en @change), `video` sur l'aperçu webcam, et passe
// `cropSrc` / `showCropper` à ImageCropperModal.

const isTouch = typeof window !== 'undefined' && window.matchMedia('(pointer: coarse)').matches
const canUseWebcam = typeof window !== 'undefined' && window.isSecureContext && !!navigator.mediaDevices?.getUserMedia

export const canTakePhoto = isTouch || canUseWebcam

export function readAsDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result)
    reader.onerror = reject
    reader.readAsDataURL(file)
  })
}

export function usePhotoCapture() {
  const cameraInput = ref(null)
  const fileInput = ref(null)
  const cropSrc = ref(null)
  const showCropper = ref(false)
  const error = ref(null)

  // Webcam (ordinateur)
  const webcamOn = ref(false)
  const video = ref(null)
  let stream = null

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

  onBeforeUnmount(() => {
    stopWebcam()
    if (cropSrc.value) URL.revokeObjectURL(cropSrc.value)
  })

  return {
    cameraInput, fileInput, video, webcamOn, cropSrc, showCropper, error,
    takePhoto, importPhoto, onFile, captureWebcam, stopWebcam,
  }
}
