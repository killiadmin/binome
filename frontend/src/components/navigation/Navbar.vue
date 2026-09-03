<script setup>
import {ref, watch, onMounted} from 'vue';
import {useRoute, useRouter} from 'vue-router';
import {BNav, BNavItem, BModal, BFormInput} from 'bootstrap-vue-next';
import {charactersAccess, isCharactersUnlocked} from '../../services/charactersAccess';
import {copyToClipboard} from '../../services/copyToClipboard';

const isOpen = ref(false);
const route = useRoute();
const router = useRouter();

watch(() => route.fullPath, () => {
  isOpen.value = false;
});

// ─── Accès « gestion des personnages » ───────────────────────────────────────
const showUnlockModal = ref(false);
const passwordInput = ref('');
const unlockError = ref(null);
const unlocking = ref(false);

async function submitUnlock() {
  if (unlocking.value) return;
  unlocking.value = true;
  unlockError.value = null;
  try {
    await charactersAccess.unlock(passwordInput.value);
    showUnlockModal.value = false;
    passwordInput.value = '';
  } catch (e) {
    unlockError.value = e.response?.data?.message || 'Mot de passe incorrect.';
  } finally {
    unlocking.value = false;
  }
}

function onUnlockModalHidden() {
  unlockError.value = null;
  passwordInput.value = '';
}

function lockAccess() {
  charactersAccess.lock();
  if (route.meta.requiresCharactersAccess) {
    router.push('/');
  }
}

// ─── Copier l'URL de connexion (celle du réseau local que les autres joueurs utilisent) ──
const copyState = ref('idle'); // 'idle' | 'done' | 'error'
// URL que les autres joueurs doivent ouvrir. Par défaut l'origine courante ;
// si l'hôte est sur localhost, on récupère l'IP LAN réelle écrite par
// scripts/update-lan-ip.sh dans public/lan-url.json (adresse dynamique).
const joinUrl = ref(window.location.origin + '/');
let copyResetTimer = null;

onMounted(async () => {
  const host = window.location.hostname;
  const onLocalhost = host === 'localhost' || host === '127.0.0.1' || host === '[::1]';
  if (!onLocalhost) return; // déjà ouvert via l'IP LAN : l'origine courante convient
  try {
    const res = await fetch('/lan-url.json?t=' + Date.now(), {cache: 'no-store'});
    if (!res.ok) return;
    const data = await res.json();
    if (data?.url) joinUrl.value = data.url;
  } catch (e) {
    /* pas de fichier lan-url.json : on garde l'origine courante */
  }
});

async function copyJoinUrl() {
  copyState.value = (await copyToClipboard(joinUrl.value)) ? 'done' : 'error';
  clearTimeout(copyResetTimer);
  copyResetTimer = setTimeout(() => {
    copyState.value = 'idle';
  }, 2000);
}
</script>

<template>
  <nav class="arcade-navbar fixed-top w-100">
    <div class="arcade-navbar-bar">
      <router-link to="/" class="arcade-brand" @click="isOpen = false">
        <i class="fa-solid fa-dice"></i> Binome
      </router-link>
      <button
        type="button"
        class="arcade-burger"
        :class="{ 'is-open': isOpen }"
        :aria-expanded="isOpen"
        aria-label="Ouvrir le menu"
        @click="isOpen = !isOpen"
      >
        <i class="fa-solid" :class="isOpen ? 'fa-xmark' : 'fa-bars'"></i>
      </button>
    </div>
    <BNav class="arcade-nav-links" :class="{ 'is-open': isOpen }">
      <BNavItem to="/rooms" class="arcade-nav-link">
        <i class="fa-solid fa-door-open"></i> Salons
      </BNavItem>
      <BNavItem to="/rules" class="arcade-nav-link">
        <i class="fa-solid fa-book"></i> Règles
      </BNavItem>
      <BNavItem v-if="isCharactersUnlocked" to="/characters/list" class="arcade-nav-link">
        <i class="fa-solid fa-user-astronaut"></i> Personnages
      </BNavItem>
      <BNavItem v-if="isCharactersUnlocked" to="/characters" class="arcade-nav-link">
        <i class="fa-solid fa-user-plus"></i> Créer
      </BNavItem>
      <li class="arcade-nav-link nav-item">
        <button
          type="button"
          class="nav-link arcade-lock-btn"
          :class="{ 'is-done': copyState === 'done', 'is-error': copyState === 'error' }"
          :title="'Copier l\'URL pour rejoindre : ' + joinUrl"
          @click="copyJoinUrl"
        >
          <i
            class="fa-solid"
            :class="{
              'fa-link': copyState === 'idle',
              'fa-check': copyState === 'done',
              'fa-triangle-exclamation': copyState === 'error',
            }"
          ></i>
          <span class="arcade-copy-label">{{
            copyState === 'done' ? 'Copié !' : copyState === 'error' ? 'Échec' : 'Copier le lien'
          }}</span>
        </button>
      </li>
      <li class="arcade-nav-link nav-item">
        <button
          v-if="!isCharactersUnlocked"
          type="button"
          class="nav-link arcade-lock-btn"
          title="Accéder à la gestion des personnages"
          @click="showUnlockModal = true"
        >
          <i class="fa-solid fa-lock"></i>
        </button>
        <button
          v-else
          type="button"
          class="nav-link arcade-lock-btn"
          title="Verrouiller la gestion des personnages"
          @click="lockAccess"
        >
          <i class="fa-solid fa-lock-open"></i>
        </button>
      </li>
    </BNav>
  </nav>

  <BModal
    v-model="showUnlockModal"
    title="Gestion des personnages"
    no-footer
    class="arcade-modal"
    @hidden="onUnlockModalHidden"
  >
    <p class="mb-2">Saisis le mot de passe pour afficher les onglets de gestion des personnages.</p>
    <form @submit.prevent="submitUnlock">
      <BFormInput
        v-model="passwordInput"
        type="password"
        placeholder="Mot de passe"
        autofocus
      />
      <p v-if="unlockError" class="text-danger mt-2 mb-0">{{ unlockError }}</p>
      <div class="text-center mt-3">
        <button type="submit" class="cabinet-btn cabinet-btn--sm" :disabled="unlocking || !passwordInput">
          <i class="fa-solid fa-unlock"></i> Déverrouiller
        </button>
        <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="showUnlockModal = false">
          Annuler
        </button>
      </div>
    </form>
  </BModal>
</template>

<style scoped>
.arcade-navbar {
  font-family: 'Baloo 2', sans-serif;
  background: var(--arcade-blue-grey);
  border-bottom: 3px solid var(--arcade-blue-grey-dark);
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.2);
  padding: 0.5rem 1rem;
}

.arcade-navbar-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 40px;
}

.arcade-brand {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.75rem;
  color: var(--arcade-beige) !important;
  letter-spacing: 1px;
  text-decoration: none;
  padding: 0.5rem 0.25rem;
}

.arcade-burger {
  background: transparent;
  border: 2px solid var(--arcade-beige);
  border-radius: 8px;
  color: var(--arcade-beige);
  font-size: 1.1rem;
  line-height: 1;
  padding: 0.4rem 0.6rem;
}

.arcade-burger:hover {
  background: rgba(245, 245, 220, 0.15);
}

/* Mobile-first: links collapsed into a dropdown by default */
.arcade-nav-links {
  flex-direction: column;
  align-items: stretch;
  gap: 0.25rem;
  max-height: 0;
  overflow: hidden;
  transition: max-height 0.2s ease;
}

.arcade-nav-links.is-open {
  max-height: 20rem;
  margin-top: 0.5rem;
}

.arcade-nav-link :deep(.nav-link) {
  font-weight: 700;
  color: var(--arcade-beige) !important;
  padding: 0.6rem 0.9rem;
  border-radius: 10px;
  transition: background 0.15s ease;
}

.arcade-nav-link :deep(.nav-link:hover) {
  background: rgba(245, 245, 220, 0.15);
  color: var(--arcade-beige) !important;
}

.arcade-nav-link :deep(.router-link-active) {
  background: var(--arcade-taupe);
}

.arcade-lock-btn {
  background: transparent;
  border: none;
  font-weight: 700;
  color: var(--arcade-beige) !important;
  padding: 0.6rem 0.9rem;
  border-radius: 10px;
  cursor: pointer;
  transition: background 0.15s ease;
}

.arcade-lock-btn:hover {
  background: rgba(245, 245, 220, 0.15);
}

.arcade-copy-label {
  margin-left: 0.4rem;
}

.arcade-lock-btn.is-done {
  color: var(--arcade-green, #7bd88f) !important;
}

.arcade-lock-btn.is-error {
  color: var(--arcade-red, #f2777a) !important;
}

/* From tablet width up: show links inline, hide the burger */
@media (min-width: 768px) {
  .arcade-burger {
    display: none;
  }

  .arcade-nav-links {
    flex-direction: row;
    align-items: center;
    gap: 0.25rem;
    max-height: none;
    overflow: visible;
    margin-top: 0;
  }
}
</style>
