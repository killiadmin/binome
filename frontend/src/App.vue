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
/* Hauteur réelle de la navbar fixed-top, utilisée pour décaler le contenu
   placé en dessous : 8px + 8px de padding vertical + 44px (la cible tactile
   du burger, qui fixe la hauteur de .arcade-navbar-bar) + 3px de bordure.
   Mesurée, pas arrondie : trop grande elle laisse une bande du fond du body
   sous la navbar, trop petite elle fait passer le contenu dessous. */
:root {
  --navbar-height: 63px;
}

.app-main {
  padding-top: var(--navbar-height);
  padding-bottom: var(--navbar-height);
}
</style>
