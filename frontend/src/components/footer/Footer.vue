<script setup></script>

<template>
  <div class="footer arcade-footer">
    <b-container fluid class="py-3">
      <b-row>
        <b-col cols="12" class="text-center align-self-center p-2">
          <p class="mb-0"><i class="fa-solid fa-dice-d20"></i> &copy; {{ new Date().getFullYear() }} Binome. Tous droits réservés.</p>
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

.arcade-footer {
  font-family: 'Baloo 2', sans-serif;
  background: var(--arcade-blue-grey);
  border-top: 3px solid var(--arcade-blue-grey-dark);
  color: var(--arcade-beige);
  font-weight: 600;
  font-size: 0.85rem;
}
</style>
