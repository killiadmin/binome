<script setup>
import {ref, watch} from 'vue';
import {useRoute} from 'vue-router';
import {BNav, BNavItem} from 'bootstrap-vue-next';

const isOpen = ref(false);
const route = useRoute();

watch(() => route.fullPath, () => {
  isOpen.value = false;
});
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
      <BNavItem to="/characters/list" class="arcade-nav-link">
        <i class="fa-solid fa-user-astronaut"></i> Personnages
      </BNavItem>
      <BNavItem to="/characters" class="arcade-nav-link">
        <i class="fa-solid fa-user-plus"></i> Créer
      </BNavItem>
    </BNav>
  </nav>
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
