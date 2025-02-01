<script setup></script>

<template>
  <div class="footer bg-color-blue-grey bt-white">
    <b-container fluid class="text-light py-3">
      <b-row>
        <b-col cols="12" md="6" class="text-center text-md-start align-self-center p-2">
          <p class="mb-0">&copy; {{ new Date().getFullYear() }} Mon Site. Tous droits réservés.</p>
        </b-col>
        <b-col cols="12" md="6" class="text-center text-md-end p-2">
          <b-button class="m-1" variant="outline-light" href="#">Mentions Légales</b-button>
          <b-button class="m-1" variant="outline-light" href="#">Contact</b-button>
        </b-col>
      </b-row>
    </b-container>
  </div>
</template>

<style scoped>
.footer {
  position: fixed;
  bottom: 0;
  left: 0;
  width: 100%;
}
</style>
