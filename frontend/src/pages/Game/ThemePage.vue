<script>
import { BButton, BCard, BContainer, BRow, BCol, BModal } from "bootstrap-vue-next";

export default {
  name: "ThemePage",
  components: {
    BButton,
    BCard,
    BContainer,
    BRow,
    BCol,
    BModal,
  },
  data() {
    return {
      themes: [
        {
          id: 1,
          name: "Animaux",
          description: "Testez vos connaissances sur le monde animal.",
          icon: "fa-solid fa-paw",
        },
        {
          id: 2,
          name: "Histoire",
          description: "Voyagez à travers le temps et les époques.",
          icon: "fa-solid fa-landmark",
        }
      ],
      showModal: false,
      selectedTheme: null,
    };
  }
};
</script>

<template>
  <BContainer>
    <h1 class="text-center m-5 color-beige">Choisir un thème</h1>

    <BRow class="justify-content-center mx-auto" style="max-width: 300px;">
      <BCol v-for="theme in themes" :key="theme.id" cols="12" class="mb-3 d-flex">
        <BCard class="w-100 bg-color-beige border shadow p-3">
          <div class="d-flex align-items-center">
            <i :class="[theme.icon, 'me-3', 'fa-2x', 'text-primary']"></i>
            <h2 class="h5 m-0">{{ theme.name }}</h2>
          </div>
          <p class="mt-2">{{ theme.description }}</p>
          <div class="d-flex justify-content-between mt-3" style="width: 240px;">
            <BButton class="bg-color-blue-grey border w-50 me-2">
              Sélectionner
            </BButton>
          </div>
        </BCard>
      </BCol>
    </BRow>
  </BContainer>
</template>

<style scoped>
h2 {
  font-size: 1.2rem;
}

p {
  font-size: 0.9rem;
}
</style>
