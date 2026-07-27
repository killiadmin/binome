<script setup>
import { ref } from 'vue'
import Cropper from 'cropperjs'
import 'cropperjs/dist/cropper.css'
import { BModal } from 'bootstrap-vue-next'

const props = defineProps({
  imageSrc: { type: String, default: null },
})

const emit = defineEmits(['cropped', 'cancel'])

const visible = defineModel({ type: Boolean, default: false })

const imageRef = ref(null)
let cropper = null
let modalIsShown = false
let imageIsLoaded = false

function tryInitCropper() {
  if (modalIsShown && imageIsLoaded && imageRef.value && !cropper) {
    cropper = new Cropper(imageRef.value, {
      aspectRatio: 1,
      viewMode: 1,
      dragMode: 'move',
      autoCropArea: 1,
      background: false,
      responsive: true,
    })
  }
}

function onShown() {
  modalIsShown = true
  tryInitCropper()
}

function onHidden() {
  modalIsShown = false
  imageIsLoaded = false
  if (cropper) {
    cropper.destroy()
    cropper = null
  }
}

function onImageLoad() {
  imageIsLoaded = true
  tryInitCropper()
}

function handleCancel() {
  emit('cancel')
  visible.value = false
}

function handleValidate() {
  if (!cropper) return

  cropper.getCroppedCanvas({ width: 512, height: 512 }).toBlob((blob) => {
    if (!blob) return
    emit('cropped', new File([blob], 'avatar.jpg', { type: 'image/jpeg' }))
    visible.value = false
  }, 'image/jpeg', 0.9)
}
</script>

<template>
  <BModal
      v-model="visible"
      title="Recadrer l'image"
      no-footer
      no-close-on-backdrop
      class="arcade-modal"
      @shown="onShown"
      @hidden="onHidden"
  >
    <div class="cropper-container">
      <img v-if="imageSrc" ref="imageRef" :src="imageSrc" alt="" class="cropper-image" @load="onImageLoad" />
    </div>
    <div class="text-center mt-3">
      <button type="button" class="cabinet-btn cabinet-btn--sm" @click="handleValidate">
        <i class="fa-solid fa-crop"></i> Valider le cadrage
      </button>
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="handleCancel">
        Annuler
      </button>
    </div>
  </BModal>
</template>

<style scoped>
.cropper-container {
  max-height: 60vh;
}

.cropper-image {
  display: block;
  max-width: 100%;
}
</style>
