<script setup>
// Vignettes des univers jouables (image + nom) ; un clic émet `open` avec le
// cosmos, la page hôte affiche alors sa fiche (UniverseSheet).

defineProps({
  // [{ id, name, description, image }]
  cosmos: { type: Array, required: true },
  // Vignette mise en avant (cosmos choisi dans le salon).
  selectedId: { type: Number, default: null },
})

const emit = defineEmits(['open'])
</script>

<template>
  <ul class="universe-gallery">
    <li v-for="c in cosmos" :key="c.id">
      <button
          type="button"
          class="universe-tile"
          :class="{ 'is-selected': c.id === selectedId }"
          :aria-label="`Voir la fiche de l'univers ${c.name}`"
          @click="emit('open', c)"
      >
        <span class="universe-tile__thumb">
          <img v-if="c.image" :src="c.image" alt="" class="universe-tile__img" loading="lazy" />
          <i v-else class="fa-solid fa-earth-europe" aria-hidden="true"></i>
        </span>
        <span class="universe-tile__name">{{ c.name }}</span>
        <span v-if="c.id === selectedId" class="universe-tile__badge"><i class="fa-solid fa-check"></i> Choisi</span>
      </button>
    </li>
  </ul>
</template>

<style scoped>
.universe-gallery {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  /* 2 vignettes par ligne même à 320px, plus sur grand écran. */
  grid-template-columns: repeat(auto-fill, minmax(min(130px, 100%), 1fr));
  gap: 0.75rem;
}

.universe-tile {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  padding: 0;
  overflow: hidden;
  background: var(--arcade-beige);
  border: 3px solid var(--arcade-blue-grey);
  border-radius: 12px;
  cursor: pointer;
  text-align: left;
  transition: transform 0.12s ease, box-shadow 0.12s ease;
}

.universe-tile:hover,
.universe-tile:focus-visible {
  transform: translateY(-2px);
  box-shadow: 0 6px 0 rgba(0, 0, 0, 0.25);
}

.universe-tile.is-selected {
  border-color: var(--arcade-success, #2e7d32);
}

.universe-tile__thumb {
  display: flex;
  align-items: center;
  justify-content: center;
  aspect-ratio: 16 / 9;
  width: 100%;
  background: linear-gradient(135deg, var(--arcade-blue-grey), var(--arcade-blue-grey-dark));
  color: rgba(245, 245, 220, 0.55);
  font-size: 1.6rem;
}

.universe-tile__img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.universe-tile__name {
  padding: 0.4rem 0.55rem 0.5rem;
  font-family: 'Baloo 2', sans-serif;
  font-weight: 800;
  font-size: 0.95rem;
  line-height: 1.2;
  color: var(--arcade-blue-grey-dark);
  overflow-wrap: anywhere;
}

.universe-tile__badge {
  position: absolute;
  top: 0.35rem;
  right: 0.35rem;
  padding: 0.05rem 0.45rem;
  border-radius: 999px;
  background: var(--arcade-success, #2e7d32);
  color: #fff;
  font-size: 0.7rem;
  font-weight: 700;
}
</style>
