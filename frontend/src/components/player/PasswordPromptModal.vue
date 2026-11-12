<script setup>
import {ref} from 'vue'
import {BModal, BFormInput} from 'bootstrap-vue-next'
import {profileService} from '../../services/profileService'
import {setProfile, updateProfilePlayer} from '../../services/profile'

// Proposée une seule fois par profil (players.password_prompted côté serveur),
// juste après la première création / arrivée dans un salon. Toute fermeture
// sans mot de passe compte comme un refus : on ne redemande plus.

const show = defineModel({type: Boolean, default: false})

const password = ref('')
const confirm = ref('')
const error = ref(null)
const saving = ref(false)
let saved = false

async function protect() {
  error.value = null
  if (password.value.length < 6) {
    error.value = 'Le mot de passe doit faire au moins 6 caractères.'
    return
  }
  if (password.value !== confirm.value) {
    error.value = 'Les deux mots de passe ne correspondent pas.'
    return
  }
  saving.value = true
  try {
    const {data} = await profileService.setPassword(password.value)
    setProfile(data.token, data.player)
    saved = true
    show.value = false
  } catch (e) {
    error.value = e.response?.data?.message || "Le mot de passe n'a pas pu être enregistré."
  } finally {
    saving.value = false
  }
}

async function onHidden() {
  password.value = ''
  confirm.value = ''
  error.value = null
  if (saved) {
    saved = false
    return
  }
  updateProfilePlayer({password_prompted: true})
  try {
    await profileService.dismissPasswordPrompt()
  } catch { /* hors ligne : on reposera la question une fois */ }
}
</script>

<template>
  <BModal v-model="show" title="🔒 Protéger ton profil ?" no-footer class="arcade-modal" centered @hidden="onHidden">
    <p class="mb-2">
      Ton profil est <strong>public</strong> : n'importe qui peut s'y connecter avec ton pseudo.
    </p>
    <p class="text-muted small mb-3">
      Avec un mot de passe, toi seul peux l'ouvrir — et reprendre ta partie depuis un autre appareil.
      Tu pourras le faire plus tard depuis l'onglet Profil.
    </p>
    <form @submit.prevent="protect">
      <BFormInput
          v-model="password"
          type="password"
          autocomplete="new-password"
          placeholder="Mot de passe (6 caractères min.)"
          class="mb-2"
      />
      <BFormInput
          v-model="confirm"
          type="password"
          autocomplete="new-password"
          placeholder="Confirme le mot de passe"
      />
      <p v-if="error" class="text-danger mt-2 mb-0">{{ error }}</p>
      <div class="prompt-actions">
        <button type="submit" class="cabinet-btn cabinet-btn--sm" :disabled="saving || !password">
          <i class="fa-solid fa-lock"></i> Protéger
        </button>
        <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" :disabled="saving" @click="show = false">
          Non merci
        </button>
      </div>
    </form>
  </BModal>
</template>

<style scoped>
.prompt-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.5rem;
  margin-top: 1rem;
}
</style>
