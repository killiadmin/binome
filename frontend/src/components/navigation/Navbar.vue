<script setup>
import {BNav, BNavItem} from 'bootstrap-vue-next';
</script>

<template>
  <BNav class="navbar navbar-expand-lg arcade-navbar fixed-top w-100">
    <BNavItem to="/" class="navbar-brand arcade-brand">
      <i class="fa-solid fa-dice"></i> Binome
    </BNavItem>
    <BNavItem to="/rooms" class="arcade-nav-link">
      <i class="fa-solid fa-door-open"></i> Salons
    </BNavItem>
    <BNavItem to="/rules" class="arcade-nav-link">
      <i class="fa-solid fa-book"></i> Règles
    </BNavItem>
  </BNav>
</template>

<style scoped>
.arcade-navbar {
  font-family: 'Baloo 2', sans-serif;
  background: var(--arcade-blue-grey);
  border-bottom: 3px solid var(--arcade-blue-grey-dark);
  box-shadow: 0 4px 0 rgba(0, 0, 0, 0.2);
  padding: 0.5rem 1rem;
}

.arcade-brand :deep(.nav-link) {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.75rem;
  color: var(--arcade-beige) !important;
  letter-spacing: 1px;
}

.arcade-nav-link :deep(.nav-link) {
  font-weight: 700;
  color: var(--arcade-beige) !important;
  padding: 0.5rem 0.9rem;
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
</style>
