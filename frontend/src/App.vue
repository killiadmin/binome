<script setup>
import Navbar from './components/navigation/Navbar.vue';
import Footer from './components/footer/Footer.vue';
</script>

<template>
  <Navbar />
  <main class="app-main">
    <router-view/>
  </main>
  <Footer />
</template>

<style>
body {
  background: #060410;
}

/* Hauteur de la navbar fixed-top, pour décaler le contenu en dessous */
:root {
  --navbar-height: 64px;
}

.app-main {
  padding-top: var(--navbar-height);
}
</style>
