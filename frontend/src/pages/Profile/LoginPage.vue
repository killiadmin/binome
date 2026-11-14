<script setup>
import {ref, onMounted} from 'vue'
import {useRoute, useRouter} from 'vue-router'
import {BFormInput} from 'bootstrap-vue-next'
import {profileService} from '../../services/profileService'
import {setProfile, isLoggedIn} from '../../services/profile'
import {charactersAccess} from '../../services/charactersAccess'
import {dropForeignSession, resumeFromProfile} from '../../services/gameSession'
import {resetEcho} from '../../sockets/useReverb'

// Connexion au profil : pseudo + mot de passe s'il est protégé. Un pseudo
// inconnu crée le profil. Si le joueur a un salon ou une partie en cours, on
// l'y ramène directement (reprise depuis un autre navigateur).

const route = useRoute()
const router = useRouter()

const pseudo = ref(String(route.query.pseudo ?? ''))
// Renvoyé ici par api.js après un 401 (jeton absent ou révoqué)
const expired = route.query.expired === '1'
const password = ref('')
const error = ref(null)
const needsPassword = ref(false)
const loading = ref(false)

onMounted(() => {
  if (isLoggedIn.value) router.replace({name: 'Profile'})
})

async function submit() {
  if (loading.value) return
  error.value = null
  if (pseudo.value.trim().length < 2) {
    error.value = 'Entre ton pseudo (2 caractères minimum).'
    return
  }
  loading.value = true
  try {
    const {data} = await profileService.login(pseudo.value.trim(), password.value)
    dropForeignSession(data.player.id)
    setProfile(data.token, data.player)
    if (data.admin_token) charactersAccess.setToken(data.admin_token)
    resetEcho()
    await router.push(resumeFromProfile(data.player.id, data.current) ?? {name: 'Profile'})
  } catch (e) {
    const status = e.response?.status
    const fieldError = e.response?.data?.errors?.password?.[0]
    if (status === 429) {
      error.value = 'Trop de tentatives : réessaie dans une minute.'
    } else if (fieldError) {
      needsPassword.value = true
      error.value = fieldError
    } else {
      error.value = e.response?.data?.message || 'Connexion impossible.'
    }
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="arcade-page arcade-bg">
    <div class="login-wrap">
      <div class="arcade-title-wrap">
        <h1 class="arcade-title">Connexion</h1>
        <p class="arcade-subtitle">Retrouve ton profil, ta photo et tes parties.</p>
      </div>

      <div class="board-card login-card">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>

        <div v-if="expired" class="arcade-alert mb-3" role="status">
          <i class="fa-solid fa-clock-rotate-left"></i>
          Ta session a expiré : reconnecte-toi pour reprendre ton salon ou ta partie.
        </div>

        <form @submit.prevent="submit">
          <label class="form-label" for="login-pseudo">Pseudo</label>
          <BFormInput
              id="login-pseudo"
              v-model="pseudo"
              autocomplete="username"
              maxlength="20"
              placeholder="Ton pseudo"
              class="mb-3"
              autofocus
          />

          <label class="form-label" for="login-password">
            Mot de passe <span v-if="!needsPassword" class="login-optional">(si ton profil est protégé)</span>
          </label>
          <BFormInput
              id="login-password"
              v-model="password"
              type="password"
              autocomplete="current-password"
              placeholder="Mot de passe"
          />

          <div v-if="error" class="arcade-alert mt-3 mb-0" role="alert">
            <i class="fa-solid fa-triangle-exclamation"></i> {{ error }}
          </div>

          <div class="text-center mt-4">
            <button type="submit" class="play-btn play-btn--sm" :disabled="loading">
              <i class="fa-solid" :class="loading ? 'fa-spinner fa-spin' : 'fa-right-to-bracket'"></i> Se connecter
            </button>
          </div>
        </form>

        <p class="login-hint">
          Nouveau pseudo ? Ton profil est créé à la connexion. Il est public tant que tu ne
          le protèges pas par un mot de passe.
        </p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.arcade-page {
  font-family: 'Baloo 2', system-ui, sans-serif;
  min-height: 100vh;
  padding: 1.5rem 1rem 2rem;
}

.login-wrap {
  max-width: 420px;
  margin: 0 auto;
}

.arcade-title-wrap {
  text-align: center;
  margin: 1.5rem 0 2rem;
}

.arcade-title {
  font-size: clamp(1.25rem, 6.5vw, 1.9rem);
}

.login-card {
  padding: 1.5rem 1.25rem;
  text-align: left;
}

.login-card .form-label {
  display: block;
}

.login-optional {
  font-weight: 400;
  opacity: 0.75;
  font-size: 0.85em;
}

.login-hint {
  margin: 1.25rem 0 0;
  font-size: 0.85rem;
  opacity: 0.8;
  text-align: center;
}
</style>
