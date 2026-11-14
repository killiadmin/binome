<script setup>
// Fiche de présentation d'un univers de jeu (un cosmos : Disney, Films saga…) :
// nom, image, courte présentation rédigée par l'admin. Montrée dans le salon
// (choix du cosmos), dans les règles (« Univers jouables ») et en aperçu dans
// l'admin. Sans image : bandeau de remplacement ; sans texte : « fiche à venir ».

defineProps({
  // { name, description, image } — `image` = URL (null sans image)
  cosmos: { type: Object, required: true },
  // Variante resserrée (salon) : bandeau moins haut, titre plus petit.
  compact: { type: Boolean, default: false },
  // Titre de niveau h2 / h3… selon la page hôte.
  headingTag: { type: String, default: 'h3' },
})
</script>

<template>
  <article class="universe-sheet" :class="{ 'universe-sheet--compact': compact }">
    <div class="universe-sheet__banner">
      <img v-if="cosmos.image" :src="cosmos.image" :alt="`Illustration de l'univers ${cosmos.name}`" class="universe-sheet__img" />
      <div v-else class="universe-sheet__placeholder" aria-hidden="true">
        <i class="fa-solid fa-earth-europe"></i>
      </div>
    </div>
    <div class="universe-sheet__body">
      <component :is="headingTag" class="universe-sheet__name">{{ cosmos.name }}</component>
      <p v-if="cosmos.description" class="universe-sheet__text">{{ cosmos.description }}</p>
      <p v-else class="universe-sheet__text universe-sheet__text--empty">Fiche de présentation à venir.</p>
    </div>
  </article>
</template>

<style scoped>
.universe-sheet {
  background: var(--arcade-beige);
  border: 3px solid var(--arcade-blue-grey);
  border-radius: 14px;
  overflow: hidden;
  font-family: 'Baloo 2', sans-serif;
  color: #4a4438;
  text-align: left;
}

/* Bandeau au format exact de l'image enregistrée (recadrée en 16/9 dans
   l'admin) : jamais de hauteur plafonnée, qui rognerait le haut et le bas
   (visages près des bords coupés). La largeur suit la fiche. */
.universe-sheet__banner {
  aspect-ratio: 16 / 9;
  width: 100%;
  background: var(--arcade-blue-grey-dark);
}

/* `contain` : l'image entière, même si une ancienne image n'était pas en 16/9. */
.universe-sheet__img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.universe-sheet__placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  background: linear-gradient(135deg, var(--arcade-blue-grey), var(--arcade-blue-grey-dark));
  color: rgba(245, 245, 220, 0.55);
  font-size: 2.5rem;
}

.universe-sheet--compact .universe-sheet__placeholder {
  font-size: 1.8rem;
}

.universe-sheet__body {
  padding: 0.75rem 0.9rem 0.85rem;
}

.universe-sheet--compact .universe-sheet__body {
  padding: 0.55rem 0.75rem 0.65rem;
}

.universe-sheet__name {
  font-family: 'Baloo 2', sans-serif;
  font-weight: 800;
  font-size: 1.2rem;
  color: var(--arcade-blue-grey-dark);
  margin: 0 0 0.3rem;
  overflow-wrap: anywhere;
}

.universe-sheet--compact .universe-sheet__name {
  font-size: 1rem;
  margin-bottom: 0.15rem;
}

.universe-sheet__text {
  margin: 0;
  font-size: 0.95rem;
  line-height: 1.45;
  /* Retours à la ligne saisis par l'admin conservés. */
  white-space: pre-line;
  overflow-wrap: anywhere;
}

.universe-sheet--compact .universe-sheet__text {
  font-size: 0.85rem;
}

.universe-sheet__text--empty {
  font-style: italic;
  color: var(--arcade-taupe);
}
</style>
