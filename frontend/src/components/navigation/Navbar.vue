<script setup>
import {BNav, BNavItem} from 'bootstrap-vue-next';
</script>

<template>
  <BNav class="navbar navbar-expand-lg navbar-dar bg-color-blue-grey fixed-top w-100 bb-white">
    <BNavItem to="/" class="navbar-brand">
      <div class="text-white">Binome</div>
    </BNavItem>
    <BNavItem to="/rooms" class="nav-link">
      <div class="text-white">Salons</div>
    </BNavItem>
    <BNavItem to="/rules" class="nav-link">
      <div class="text-white">Règles</div>
    </BNavItem>
  </BNav>
</template>

<style scoped>
.navbar {
  margin-bottom: 20px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
}
</style>
