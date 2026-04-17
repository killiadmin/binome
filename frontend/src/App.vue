<script setup>
import Navbar from './components/navigation/Navbar.vue';
import Footer from './components/footer/Footer.vue';
</script>

<template>
  <Navbar />
  <router-view/>
  <Footer />
</template>

<style scoped>
body {
  background: #060410;
}
</style>
