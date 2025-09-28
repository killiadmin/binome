<script>
import { BButton, BCard, BContainer } from "bootstrap-vue-next";

export default {
  name: "RoundPage",
  components: {
    BButton,
    BCard,
    BContainer,
  },
  data() {
    return {
      roundTitle: "Qui suis-je ?",
      characterImage: "https://via.placeholder.com/250x250.png?text=Personnage",
      clues: ["Indice 1", "Indice 2", "Indice 3"],
      currentPlayer: "Alice",
    };
  },
  methods: {
    characterNotFound() {
      this.$router.push({ name: "NextPlayer" });
    },
    characterFound() {
      this.$router.push({ name: "NextPlayer" });
    },
  },
};
</script>

<template>
  <BContainer class="text-center p-4">
    <h1 class="mb-5 color-beige">{{ roundTitle }}</h1>
    <h2 class="mb-3">Joueur actuel : {{ currentPlayer }}</h2>

    <BCard class="mb-4 bg-color-beige border shadow p-3">
      <img :src="characterImage" alt="Personnage mystère" class="img-fluid rounded mx-auto d-block" style="max-width: 250px;"/>
    </BCard>

    <BCard class="mb-4 bg-color-beige border shadow p-3">
      <h3 class="h5 mb-3">Indices :</h3>
      <ul class="list-unstyled">
        <li v-for="(clue, index) in clues" :key="index" class="mb-2">
          {{ clue }}
        </li>
      </ul>
    </BCard>

    <div class="d-flex flex-column gap-3">
      <BButton variant="danger" class="fw-bold" @click="characterNotFound">
        Personnage non trouvé
      </BButton>
      <BButton variant="success" class="fw-bold" @click="characterFound">
        Personnage trouvé
      </BButton>
    </div>
  </BContainer>
</template>

<style scoped>
h1 {
  font-size: 1.8rem;
}
h2 {
  font-size: 1.2rem;
  color: #555;
}
ul {
  font-size: 1rem;
}
</style>
