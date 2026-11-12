<script setup>
import {ref, watch, nextTick, onMounted, onBeforeUnmount} from 'vue';
import {useRoute, useRouter} from 'vue-router';
import {BNav, BNavItem, BModal, BFormInput} from 'bootstrap-vue-next';
import {charactersAccess, isCharactersUnlocked} from '../../services/charactersAccess';
import {copyToClipboard} from '../../services/copyToClipboard';
import {resolveJoinUrl} from '../../services/joinUrl';
import {isImmersive} from '../../composables/useImmersiveMode';
import logo96 from '../../assets/logo/binome-logo-96.webp';

const isOpen = ref(false);
const route = useRoute();
const router = useRouter();

watch(() => route.fullPath, () => {
  isOpen.value = false;
});

// ─── Accès à l'espace d'administration (pages/Admin/AdminLayout.vue) ─────────
// Cadenas fermé : mot de passe puis redirection vers /admin. Cadenas ouvert :
// lien direct vers /admin (le reverrouillage se fait depuis l'espace admin).
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
    router.push('/admin');
  } catch (e) {
    unlockError.value = e.response?.status === 429
      ? 'Trop de tentatives, réessayez dans une minute.'
      : e.response?.data?.message || 'Mot de passe incorrect.';
  } finally {
    unlocking.value = false;
  }
}

function onUnlockModalHidden() {
  unlockError.value = null;
  passwordInput.value = '';
}

// ─── Partager l'URL de connexion (celle du réseau local que les autres joueurs utilisent) ──
// L'URL est recalculée à chaque clic (voir services/joinUrl.js) : si l'hôte
// change de Wi-Fi, le lien partagé suit sans recharger la page.
const shareState = ref('idle'); // 'idle' | 'loading' | 'copied' | 'shared' | 'error'
const shareError = ref('');
const joinUrl = ref(null);
let shareResetTimer = null;

// Feuille de partage native seulement sur écran tactile (sur desktop, la copie
// est plus directe). navigator.share n'existe qu'en contexte sécurisé.
const canNativeShare = () =>
  typeof navigator.share === 'function' && window.matchMedia('(pointer: coarse)').matches;

async function refreshJoinUrl() {
  const {url, verified} = await resolveJoinUrl();
  joinUrl.value = url;
  return {url, verified};
}

onMounted(() => {
  refreshJoinUrl();
  window.addEventListener('online', refreshJoinUrl);
});
onBeforeUnmount(() => window.removeEventListener('online', refreshJoinUrl));

// ─── Hauteur réelle de la navbar → --navbar-height ───────────────────────────
// Elle varie : une ligne en mobile, deux à partir de 768px (liens sous le
// logo), plus encore si les liens passent à la ligne. Le contenu (.app-main) est décalé de cette
// valeur. En mobile, le menu burger ouvert recouvre la page au lieu de la
// pousser : sa hauteur est donc retirée de la mesure. En mode plein écran
// (navbar masquée par App.vue), elle vaut 0.
const navRef = ref(null);
let navResizeObserver = null;

function syncNavbarHeight() {
  const nav = navRef.value;
  if (!nav) return;
  if (isImmersive.value) {
    document.documentElement.style.setProperty('--navbar-height', '0px');
    return;
  }
  let height = nav.getBoundingClientRect().height;
  const burger = nav.querySelector('.arcade-burger');
  if (burger && getComputedStyle(burger).display !== 'none') {
    const links = nav.querySelector('.arcade-nav-links');
    height -= links.getBoundingClientRect().height + parseFloat(getComputedStyle(links).marginTop);
  }
  document.documentElement.style.setProperty('--navbar-height', Math.ceil(height) + 'px');
}

onMounted(() => {
  syncNavbarHeight();
  navResizeObserver = new ResizeObserver(syncNavbarHeight);
  navResizeObserver.observe(navRef.value);
});
onBeforeUnmount(() => navResizeObserver?.disconnect());
watch(isImmersive, () => nextTick(syncNavbarHeight));

function settle(state, error = '') {
  shareState.value = state;
  shareError.value = error;
  clearTimeout(shareResetTimer);
  shareResetTimer = setTimeout(() => {
    shareState.value = 'idle';
  }, state === 'error' ? 3500 : 2000);
}

async function shareJoinUrl() {
  if (shareState.value === 'loading') return;
  shareState.value = 'loading';
  const {url, verified} = await refreshJoinUrl();
  if (!url) return settle('error', 'Aucun réseau Wi-Fi détecté (relance ./start.sh)');
  if (!verified) return settle('error', 'Adresse ' + url + ' injoignable — réseau changé ?');

  if (canNativeShare()) {
    try {
      await navigator.share({title: 'Binome', text: 'Rejoins ma partie de Binome !', url});
      return settle('shared');
    } catch (e) {
      if (e?.name === 'AbortError') return settle('idle');
      /* partage refusé : repli sur la copie */
    }
  }
  if (await copyToClipboard(url)) return settle('copied');
  settle('error', 'Copie impossible — lien : ' + url);
}
</script>

<template>
  <!-- Masquée en mode plein écran (composables/useImmersiveMode.js). Le v-show
       est ici et non sur <Navbar> dans App.vue : le composant a deux racines
       (nav + BModal), Vue y ignorerait la directive. -->
  <nav v-show="!isImmersive" ref="navRef" class="arcade-navbar fixed-top w-100">
    <div class="arcade-navbar-bar">
      <router-link to="/" class="arcade-brand" @click="isOpen = false">
        <img :src="logo96" alt="" class="arcade-brand-logo" width="36" height="36" decoding="async" />
        Binome
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
      <BNavItem to="/historique" class="arcade-nav-link">
        <i class="fa-solid fa-ranking-star"></i> Historique
      </BNavItem>
      <li class="arcade-nav-link nav-item">
        <button
          type="button"
          class="nav-link arcade-lock-btn"
          :class="{
            'is-done': shareState === 'copied' || shareState === 'shared',
            'is-error': shareState === 'error',
          }"
          :title="
            shareState === 'error'
              ? shareError
              : joinUrl
                ? 'Partager l\'URL pour rejoindre : ' + joinUrl
                : 'Partager l\'URL pour rejoindre'
          "
          :disabled="shareState === 'loading'"
          @click="shareJoinUrl"
        >
          <i
            class="fa-solid"
            :class="{
              'fa-share-nodes': shareState === 'idle',
              'fa-spinner fa-spin': shareState === 'loading',
              'fa-check': shareState === 'copied' || shareState === 'shared',
              'fa-triangle-exclamation': shareState === 'error',
            }"
          ></i>
          <span class="arcade-copy-label">{{
            {
              idle: 'Partager le lien',
              loading: 'Analyse…',
              copied: 'Copié !',
              shared: 'Partagé !',
              error: 'Lien indisponible',
            }[shareState]
          }}</span>
        </button>
      </li>
      <li class="arcade-nav-link nav-item">
        <button
          v-if="!isCharactersUnlocked"
          type="button"
          class="nav-link arcade-lock-btn"
          title="Accéder à l'administration"
          aria-label="Accéder à l'administration"
          @click="showUnlockModal = true"
        >
          <i class="fa-solid fa-lock"></i>
        </button>
        <router-link
          v-else
          to="/admin"
          class="nav-link arcade-lock-btn"
          active-class="router-link-active"
          title="Administration"
          aria-label="Administration"
        >
          <i class="fa-solid fa-lock-open"></i>
        </router-link>
      </li>
    </BNav>
  </nav>

  <BModal
    v-model="showUnlockModal"
    title="Administration"
    no-footer
    class="arcade-modal"
    @hidden="onUnlockModalHidden"
  >
    <p class="mb-2">Saisis le mot de passe pour accéder à l'espace d'administration.</p>
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
  /* C'est aussi le lien de retour à l'accueil : cible tactile de 44px. */
  display: inline-flex;
  align-items: center;
  min-height: 44px;
  padding: 0.5rem 0.25rem;
  gap: 0.5rem;
}

/* 36px : tient dans la cible de 44px sans grandir la barre (--navbar-height). */
.arcade-brand-logo {
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  margin: -0.5rem 0;
}

.arcade-burger {
  background: transparent;
  border: 2px solid var(--arcade-beige);
  border-radius: 8px;
  color: var(--arcade-beige);
  font-size: 1.1rem;
  line-height: 1;
  padding: 0.4rem 0.6rem;
  /* 44px : cible tactile minimale, c'est le seul point d'entrée du menu. */
  min-width: 44px;
  min-height: 44px;
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
  /* Navbar fixe : en paysage (320px de haut), 8 liens dépassaient l'écran
     et les derniers étaient inatteignables → le menu défile. */
  max-height: min(20rem, calc(100dvh - 4rem));
  overflow-y: auto;
  margin-top: 0.5rem;
}

.arcade-nav-link :deep(.nav-link) {
  font-weight: 700;
  color: var(--arcade-beige) !important;
  display: flex;
  align-items: center;
  min-height: 44px;
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
  display: flex;
  align-items: center;
  min-width: 44px;
  min-height: 44px;
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
/* Téléphone en paysage : chaque pixel de hauteur compte (la hauteur réelle
   est remesurée par le ResizeObserver → --navbar-height suit). */
@media (max-height: 500px) {
  .arcade-navbar {
    padding-top: 0.15rem;
    padding-bottom: 0.15rem;
  }
}

/* Liens en ligne à partir d'une tablette — pas sur un téléphone en paysage,
   où la seconde ligne de liens portait la navbar à 96px de haut. */
@media (min-width: 768px) and (min-height: 501px) {
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
