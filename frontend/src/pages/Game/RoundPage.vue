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
      roundTitle: "Alice de Alice au Pays des merveilles",
      characterImage: "https://lecoindescritiquescine.com/wp-content/uploads/2020/05/replique-alice-au-pays-des-merveilles-01.jpg",
      deathWords: ["Lapin", "Chat", "Reine"],
      currentPlayer: "Disney",
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
    <h1 class="mt-5 mx-5 color-beige">{{ roundTitle }}</h1>
    <h2 class="mb-3">Thème de la partie: {{ currentPlayer }}</h2>

    <BCard class="mb-4 bg-color-beige border shadow p-3">
      <img :src="characterImage" alt="Personnage mystère" class="img-fluid rounded mx-auto d-block" style="max-width: 250px;"/>
    </BCard>

    <BCard class="mb-4 bg-color-beige border shadow p-3">
      <h3 class="h5 mb-3">Mots interdits :</h3>
      <ul class="list-unstyled">
        <li v-for="(word, index) in deathWords" :key="index" class="mb-2">
          {{ word }}
        </li>
      </ul>
    </BCard>

    <div class="d-flex flex-column gap-3">
      <BButton variant="warning" class="fw-bold" @click="">
        Faire une accusation
      </BButton>
      <BButton variant="info" class="fw-bold" @click="">
        Poser une question
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
