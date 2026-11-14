<script setup>
import logo96 from '../../assets/logo/binome-logo-96.webp';
</script>

<template>
  <div class="footer arcade-footer">
    <b-container fluid class="py-3">
      <b-row>
        <b-col cols="12" class="text-center align-self-center p-2">
          <p class="mb-0 footer-line"><img :src="logo96" alt="" class="footer-logo" width="20" height="20" loading="lazy" decoding="async" /> &copy; {{ new Date().getFullYear() }} Binome. Tous droits réservés.
            <span class="footer-sep" aria-hidden="true">·</span>
            <router-link :to="{ name: 'Legal' }" class="footer-link">Mentions légales &amp; confidentialité</router-link></p>
        </b-col>
      </b-row>
    </b-container>
  </div>
</template>

<style scoped>
/* Téléphone : le footer suit le contenu (voir App.vue, --footer-height) */
.footer {
  width: 100%;
}

@media (min-width: 768px) and (min-height: 501px) {
  .footer {
    position: fixed;
    bottom: 0;
    left: 0;
  }
}

.footer-line {
  display: inline-flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.4rem;
}

/* 20px = hauteur de ligne du texte : le footer ne grandit pas. */
.footer-logo {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

.footer-link {
  color: inherit;
  text-decoration: underline;
}

/* Téléphone : le lien passe à la ligne ; le « · » resterait seul en bout de ligne. */
.footer-sep {
  display: none;
}

@media (min-width: 576px) {
  .footer-sep {
    display: inline;
  }
}

.arcade-footer {
  font-family: 'Baloo 2', sans-serif;
  background: var(--arcade-blue-grey);
  border-top: 3px solid var(--arcade-blue-grey-dark);
  color: var(--arcade-beige);
  font-weight: 600;
  font-size: 0.85rem;
}
</style>
