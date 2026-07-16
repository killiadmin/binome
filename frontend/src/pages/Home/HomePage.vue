<script>
import { ref, onMounted } from "vue";
import { api } from "../../services/api.js";

export default {
  name: "HomePage",
  setup() {
    const message = ref("");

    onMounted(async () => {
      try {
        const response = await api.get("/welcome");
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
  <div class="home arcade-bg text-center">
    <div class="home-hero">
      <span class="home-badge">🎲 Jeu de déduction en binôme</span>
      <h1 class="arcade-title home-title">Binome</h1>
      <p class="arcade-subtitle home-subtitle">Devine le personnage secret… avant que ton binôme ne soit découvert !</p>

      <router-link to="/rooms" class="play-btn home-cta">
        <i class="fa-solid fa-play"></i> Jouer maintenant
      </router-link>

      <router-link to="/rules" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm home-rules-link">
        <i class="fa-solid fa-book"></i> Voir les règles
      </router-link>
    </div>
  </div>
</template>

<style scoped>
.home {
  min-height: calc(100vh - 140px);
  padding: 3rem 1.5rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2.5rem;
  font-family: 'Baloo 2', sans-serif;
}

.home-hero {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.1rem;
  max-width: 480px;
}

.home-badge {
  font-weight: 700;
  font-size: 0.85rem;
  color: var(--arcade-beige);
  background: var(--arcade-blue-grey);
  border: 2px solid var(--arcade-blue-grey-dark);
  padding: 0.35rem 0.9rem;
  border-radius: 999px;
}

.home-title {
  font-size: 2.6rem;
}

.home-subtitle {
  margin: 0;
}

.home-cta {
  text-decoration: none;
  margin-top: 0.5rem;
}

.home-rules-link {
  text-decoration: none;
}

.home-status-card {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem 1.5rem;
}
</style>
