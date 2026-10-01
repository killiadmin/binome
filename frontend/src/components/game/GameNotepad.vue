<script setup>
import {BModal} from 'bootstrap-vue-next'
import PlayerAvatar from '../player/PlayerAvatar.vue'

// Vue d'ensemble du bloc-notes privé : une note libre + une note par joueur.
// Les mêmes notes par joueur sont éditables depuis sa fiche (RoundPage).
const show = defineModel({type: Boolean, default: false})

defineProps({
  notes:      {type: Object, required: true},
  players:    {type: Array, required: true},
  myPlayerId: {type: Number, default: null},
})
</script>

<template>
  <BModal v-model="show" title="📝 Mon bloc-notes" no-footer class="arcade-modal" centered scrollable>
    <p class="notepad-private">
      <i class="fa-solid fa-lock"></i>
      Privé : reste sur ce téléphone, personne d'autre ne le voit.
    </p>

    <label class="notepad-label" for="notepad-general">Notes libres</label>
    <textarea
        id="notepad-general"
        v-model="notes.general"
        class="form-control notepad-input"
        rows="3"
        maxlength="2000"
        placeholder="Univers repérés, pistes, alliances…"
    ></textarea>

    <div
        v-for="p in players.filter(p => p.id !== myPlayerId)"
        :key="p.id"
        class="notepad-player"
        :class="{ 'is-eliminated': p.is_eliminated }"
    >
      <label class="notepad-label" :for="`notepad-${p.id}`">
        <span class="notepad-avatar"><PlayerAvatar :pseudo="p.pseudo" :url="p.avatar_url" /></span>
        {{ p.pseudo }}
        <span v-if="p.is_eliminated" class="notepad-tag">éliminé</span>
      </label>
      <textarea
          :id="`notepad-${p.id}`"
          v-model="notes.players[p.id]"
          class="form-control notepad-input"
          rows="2"
          maxlength="500"
          placeholder="Personnage ? Univers ?"
      ></textarea>
    </div>
  </BModal>
</template>

<style scoped>
.notepad-private {
  font-size: 0.8rem;
  color: #6c6259;
  margin-bottom: 0.75rem;
}

.notepad-label {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-weight: 700;
  font-size: 0.85rem;
  margin-bottom: 0.25rem;
}

.notepad-avatar {
  display: inline-grid;
  place-items: center;
  width: 1.6rem;
  height: 1.6rem;
  font-size: 0.65rem;
  background: var(--arcade-blue-grey);
  color: var(--arcade-beige);
  border: 2px solid var(--arcade-blue-grey-dark);
  overflow: hidden;
}

.notepad-tag {
  font-size: 0.7rem;
  color: var(--arcade-danger);
}

.notepad-input {
  margin-bottom: 0.8rem;
  resize: vertical;
}

.notepad-player.is-eliminated {
  opacity: 0.6;
}
</style>
