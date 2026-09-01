<script setup>
import { ref } from 'vue'

const props = defineProps({
  disabled: { type: Boolean, default: false },
  hint: { type: String, default: 'PNG, JPG, GIF ou WEBP — recadrage carré à l’étape suivante' },
})

const emit = defineEmits(['file'])

const inputRef = ref(null)
const isDragOver = ref(false)

function pick() {
  if (props.disabled) return
  inputRef.value?.click()
}

function emitFile(file) {
  if (!file || props.disabled) return
  if (!file.type.startsWith('image/')) return
  emit('file', file)
}

function onChange(event) {
  emitFile(event.target.files?.[0] ?? null)
  event.target.value = ''
}

function onDrop(event) {
  isDragOver.value = false
  emitFile(event.dataTransfer?.files?.[0] ?? null)
}
</script>

<template>
  <div
      class="dropzone"
      :class="{ 'dropzone--over': isDragOver, 'dropzone--disabled': disabled }"
      role="button"
      tabindex="0"
      @click="pick"
      @keydown.enter.prevent="pick"
      @keydown.space.prevent="pick"
      @dragover.prevent="isDragOver = !disabled"
      @dragleave="isDragOver = false"
      @drop.prevent="onDrop"
  >
    <i class="fa-solid fa-cloud-arrow-up dropzone__icon"></i>
    <div class="dropzone__text">
      <strong>Glisse une image ici</strong> ou clique pour parcourir
    </div>
    <div class="dropzone__hint">{{ hint }}</div>
    <input ref="inputRef" type="file" accept="image/*" class="dropzone__input" @change="onChange" />
  </div>
</template>

<style scoped>
.dropzone {
  border: 2px dashed var(--arcade-blue-grey-dark, #4a5568);
  border-radius: 12px;
  padding: 1.25rem 1rem;
  text-align: center;
  cursor: pointer;
  background: rgba(0, 0, 0, 0.03);
  transition: border-color 0.15s ease, background 0.15s ease;
}

.dropzone:hover:not(.dropzone--disabled),
.dropzone--over {
  border-color: var(--arcade-gold, #e0a422);
  background: rgba(224, 164, 34, 0.1);
}

.dropzone--over {
  transform: scale(1.01);
}

.dropzone--disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.dropzone__icon {
  font-size: 1.6rem;
  opacity: 0.55;
  margin-bottom: 0.4rem;
}

.dropzone__text {
  font-size: 0.9rem;
}

.dropzone__hint {
  font-size: 0.75rem;
  opacity: 0.6;
  margin-top: 0.25rem;
}

.dropzone__input {
  display: none;
}
</style>
