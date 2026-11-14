<script setup>
import { useRouter } from 'vue-router'
import { charactersAccess } from '../../services/charactersAccess'

// Espace d'administration (cadenas de la navbar) : une barre de sections
// collée sous la navbar, la section active s'affiche en dessous. Toutes les
// routes enfants héritent de meta.requiresCharactersAccess (cf. router.js).

const router = useRouter()

const SECTIONS = [
  { to: '/admin/games', icon: 'fa-gamepad', label: 'Parties' },
  { to: '/admin/binomes', icon: 'fa-user-group', label: 'Binômes' },
  { to: '/admin/univers', icon: 'fa-earth-europe', label: 'Univers' },
  { to: '/admin/prompts', icon: 'fa-terminal', label: 'Prompts' },
  { to: '/admin/players', icon: 'fa-users', label: 'Joueurs' },
  { to: '/admin/reactions', icon: 'fa-face-grin-stars', label: 'Emojis' },
]

function lockAccess() {
  charactersAccess.lock()
  router.push('/')
}
</script>

<template>
  <div class="admin-layout arcade-bg">
    <header class="admin-header">
      <h1 class="admin-title"><i class="fa-solid fa-screwdriver-wrench"></i> Admin</h1>
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" @click="lockAccess">
        <i class="fa-solid fa-lock"></i> Verrouiller
      </button>
    </header>

    <nav class="admin-sections" aria-label="Sections d'administration">
      <router-link
        v-for="s in SECTIONS"
        :key="s.to"
        :to="s.to"
        class="admin-section"
        active-class="is-active"
      >
        <i class="fa-solid" :class="s.icon"></i>
        <span>{{ s.label }}</span>
      </router-link>
    </nav>

    <router-view />
  </div>
</template>

<style scoped>
/* Pas de `color` ici : hérité par les sections, il rendait illisibles les
   panneaux beiges de la liste des personnages (texte beige sur beige). */
.admin-layout {
  font-family: 'Baloo 2', system-ui, sans-serif;
  min-height: 100%;
  padding-bottom: 2rem;
}

.admin-header {
  color: var(--arcade-beige);
  max-width: 900px;
  margin: 0 auto;
  padding: 0.75rem 16px 0.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.admin-title {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.9rem;
  margin: 0;
}

/* Barre de sections, collée sous la navbar. Téléphone : 2 rangées de 3,
   icône devant le libellé (6 colonnes ne tiennent pas à 320px : « Binômes »
   serait tronqué), sans défilement caché. */
.admin-sections {
  position: sticky;
  top: var(--navbar-height);
  z-index: 10;
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.25rem;
  max-width: 900px;
  margin: 0 auto;
  padding: 0.4rem 16px;
  background: var(--arcade-dark);
  border-bottom: 2px solid var(--arcade-blue-grey-dark);
}

.admin-section {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
  min-width: 0;
  min-height: 44px;
  padding: 0.2rem 0.25rem;
  border-radius: 10px;
  border: 2px solid var(--arcade-blue-grey);
  color: var(--arcade-taupe);
  font-weight: 700;
  font-size: 0.85rem;
  line-height: 1.1;
  text-decoration: none;
}

.admin-section span {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.admin-section i {
  flex: none;
  font-size: 0.95rem;
}

.admin-section:hover {
  color: var(--arcade-beige);
}

.admin-section.is-active {
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
}

/* Les pages « personnages » ont leur propre fond plein écran : l'espace
   admin le fournit déjà. */
.admin-layout :deep(.arcade-page) {
  background: none;
  min-height: 0;
  padding-top: 0;
}

.admin-layout :deep(.arcade-title-wrap) {
  margin-top: 1rem;
}

@media (min-width: 576px) {
  .admin-sections {
    grid-template-columns: repeat(6, 1fr);
  }
}

@media (min-width: 768px) {
  .admin-sections {
    display: flex;
    justify-content: center;
    gap: 0.35rem;
    padding: 0.5rem 16px;
  }

  .admin-section {
    gap: 0.4rem;
    padding: 0.4rem 0.8rem;
    font-size: 1rem;
  }

  .admin-section i {
    font-size: inherit;
  }
}
</style>
