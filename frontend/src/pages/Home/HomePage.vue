<script>
import { ref, onMounted } from "vue";
import { api } from "../../services/api.js";
import logo192 from "../../assets/logo/binome-logo-192.webp";
import logo320 from "../../assets/logo/binome-logo-320.webp";
import logo640 from "../../assets/logo/binome-logo-640.webp";

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

    const logoSrcset = `${logo192} 192w, ${logo320} 320w, ${logo640} 640w`;

    return { message, logo320, logoSrcset };
  }
};
</script>

<template>
  <div class="home arcade-bg text-center">
    <div class="home-hero">
      <span class="home-badge">🎲 Jeu de déduction en binôme</span>
      <h1 class="home-title">
        <img
          :src="logo320"
          :srcset="logoSrcset"
          sizes="(max-height: 500px) 160px, (min-width: 768px) 280px, 60vw"
          alt="Binome"
          class="home-logo"
          width="320"
          height="320"
          fetchpriority="high"
        />
      </h1>
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
  margin: 0;
  line-height: 0;
}

/* Mobile first : 60vw (192px sur un écran de 320px), plafonné à 280px. */
.home-logo {
  display: block;
  width: min(60vw, 280px);
  height: auto;
  filter: drop-shadow(0 6px 0 rgba(0, 0, 0, 0.35));
}

/* Téléphone en paysage : la hauteur est la contrainte. */
@media (max-height: 500px) {
  .home-logo {
    width: min(160px, 40vh);
  }
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
