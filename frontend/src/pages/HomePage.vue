<script>
import { ref, onMounted } from "vue";
import api from "../api";

export default {
  name: "HomePage",
  setup() {
    const message = ref("");

    onMounted(async () => {
      try {
        const response = await api.get("/hello");
        message.value = response.data.message;
      } catch (error) {
        console.error("Erreur API", error);
      }
    });

    return { message };
  }
};
</script>

<template>
  <div class="home text-center">
    <h1 class="color-beige">Bienvenue sur Binome !</h1>
    <p class="color-grey">Votre duo vous fera gagner</p>
    <div>
      <h2 class="color-beige">{{ message }}</h2>
    </div>
  </div>
</template>

<style scoped>
.home {
  margin-top: 50px;
}
</style>
