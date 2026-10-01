<script setup>
import {computed, ref} from 'vue'
import {copyToClipboard} from '../../services/copyToClipboard'

// Récap de fin de partie (GET /games/{game}/recap) : trophées, qui a trouvé
// qui, révélation des binômes, et un résumé texte à partager.
// Le tableau des scores reste dans RoundPage, au-dessus de ce composant.

const props = defineProps({
  recap: {type: Object, required: true},
})

const shareState = ref('idle') // 'idle' | 'copied' | 'shared' | 'error'

const plural = (n, word) => `${n} ${word}${n > 1 ? 's' : ''}`

const duration = computed(() => {
  const s = props.recap.duration_seconds
  if (s === null || s === undefined) return null
  if (s < 60) return '< 1 min'
  const minutes = Math.round(s / 60)
  if (minutes < 60) return `${minutes} min`
  return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, '0')}`
})

const shareText = computed(() => {
  const r = props.recap
  const lines = [
    `🎮 Binome — partie terminée${duration.value ? ` en ${duration.value}` : ''}`,
    `🏆 ${r.winners.length ? r.winners.join(' & ') : 'Aucun vainqueur'}`,
    '',
    ...r.stats.map((s, i) =>
        `${['🥇', '🥈', '🥉'][i] ?? `#${i + 1}`} ${s.player_pseudo} (${s.character_name}) — ${s.score} pts`),
  ]
  if (r.awards.length) {
    lines.push('', ...r.awards.map(a => `${a.emoji} ${a.title} : ${a.pseudos.join(', ')} (${a.detail})`))
  }
  return lines.join('\n')
})

async function share() {
  const text = shareText.value
  try {
    if (typeof navigator.share === 'function' && window.matchMedia('(pointer: coarse)').matches) {
      await navigator.share({title: 'Récap Binome', text})
      shareState.value = 'shared'
    } else {
      shareState.value = (await copyToClipboard(text)) ? 'copied' : 'error'
    }
  } catch (e) {
    // Partage annulé par l'utilisateur : pas une erreur à signaler.
    shareState.value = e?.name === 'AbortError' ? 'idle' : 'error'
  }
  setTimeout(() => { shareState.value = 'idle' }, 2500)
}
</script>

<template>
  <div class="recap">

    <div class="recap-figures">
      <span v-if="duration" class="recap-figure"><i class="fa-solid fa-stopwatch"></i> {{ duration }}</span>
      <span class="recap-figure"><i class="fa-solid fa-rotate"></i> {{ plural(recap.rounds_count, 'round') }}</span>
      <span class="recap-figure"><i class="fa-solid fa-comment-dots"></i> {{ plural(recap.questions_count, 'question') }}</span>
      <span class="recap-figure"><i class="fa-solid fa-crosshairs"></i> {{ plural(recap.accusations_count, 'accusation') }}</span>
    </div>

    <!-- Trophées -->
    <section v-if="recap.awards.length" class="recap-section">
      <h5 class="recap-title">Trophées</h5>
      <div class="recap-awards">
        <div v-for="award in recap.awards" :key="award.key" class="recap-award">
          <span class="recap-award__emoji">{{ award.emoji }}</span>
          <div class="recap-award__body">
            <p class="recap-award__title">{{ award.title }}</p>
            <p class="recap-award__who">{{ award.pseudos.join(', ') }}</p>
            <p class="recap-award__detail">{{ award.detail }}</p>
          </div>
        </div>
      </div>
    </section>

    <!-- Qui a trouvé qui (+ exclusions par l'hôte) -->
    <section v-if="recap.eliminations.length || recap.exclusions?.length" class="recap-section">
      <h5 class="recap-title">Qui a trouvé qui</h5>
      <ol class="recap-timeline">
        <li v-for="(e, i) in recap.eliminations" :key="`e${i}`">
          <span class="recap-round">R{{ e.round }}</span>
          <strong>{{ e.by }}</strong> a démasqué <strong>{{ e.target }}</strong>
          <em>({{ e.character }})</em>
        </li>
        <li v-for="(x, i) in recap.exclusions ?? []" :key="`x${i}`">
          <span class="recap-round">R{{ x.round }}</span>
          🚪 <strong>{{ x.pseudo }}</strong> exclu par l'hôte (déconnecté)
          <template v-if="x.partner">— <strong>{{ x.partner }}</strong> éliminé avec lui</template>
        </li>
      </ol>
    </section>

    <!-- Binômes révélés -->
    <section class="recap-section">
      <h5 class="recap-title">Les binômes</h5>
      <div class="recap-binomes">
        <div v-for="b in recap.binomes" :key="b.id" class="recap-binome">
          <p class="recap-binome__universe">
            {{ b.universe }}
            <span v-if="b.is_orphan" class="recap-binome__orphan">🚷 orphelin</span>
          </p>
          <p v-for="p in b.players" :key="p.pseudo" class="recap-binome__player"
             :class="{ 'is-eliminated': p.is_eliminated }">
            <span>{{ p.pseudo }}</span>
            <span class="recap-binome__character">{{ p.character }}</span>
          </p>
        </div>
      </div>
    </section>

    <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm recap-share" @click="share">
      <i class="fa-solid"
         :class="shareState === 'copied' || shareState === 'shared' ? 'fa-check' : 'fa-share-nodes'"></i>
      {{ shareState === 'copied' ? 'Récap copié !'
        : shareState === 'shared' ? 'Partagé !'
        : shareState === 'error' ? 'Copie impossible'
        : 'Partager le récap' }}
    </button>
  </div>
</template>

<style scoped>
.recap {
  text-align: left;
  margin-top: 1.25rem;
}

.recap-figures {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 0.4rem;
}

.recap-figure {
  padding: 0.15rem 0.5rem;
  font-size: 0.8rem;
  font-weight: 700;
  background: rgba(90, 111, 125, 0.15);
  border: 2px solid rgba(90, 111, 125, 0.35);
}

.recap-section {
  margin-top: 1.25rem;
}

.recap-title {
  font-weight: 800;
  font-size: 1rem;
  margin-bottom: 0.5rem;
}

.recap-awards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 170px), 1fr));
  gap: 0.5rem;
}

.recap-award {
  display: flex;
  gap: 0.5rem;
  min-width: 0;
  padding: 0.5rem;
  background: rgba(224, 163, 28, 0.1);
  border: 2px solid rgba(224, 163, 28, 0.45);
}

.recap-award__emoji {
  font-size: 1.6rem;
  line-height: 1;
}

.recap-award__body {
  min-width: 0;
}

.recap-award__body p {
  margin: 0;
  overflow-wrap: anywhere;
}

.recap-award__title {
  font-weight: 800;
  font-size: 0.85rem;
}

.recap-award__who {
  font-weight: 700;
}

.recap-award__detail {
  font-size: 0.75rem;
  color: #6c6259;
}

.recap-timeline {
  padding-left: 0;
  list-style: none;
  margin: 0;
}

.recap-timeline li {
  padding: 0.3rem 0;
  border-bottom: 1px dashed rgba(90, 111, 125, 0.35);
  overflow-wrap: anywhere;
}

.recap-round {
  display: inline-block;
  min-width: 2.2rem;
  margin-right: 0.3rem;
  font-size: 0.75rem;
  font-weight: 800;
  color: #a8720f;
}

.recap-binomes {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 170px), 1fr));
  gap: 0.5rem;
}

.recap-binome {
  min-width: 0;
  padding: 0.5rem;
  border: 2px solid rgba(90, 111, 125, 0.35);
}

.recap-binome p {
  margin: 0;
}

.recap-binome__universe {
  font-weight: 800;
  font-size: 0.85rem;
  margin-bottom: 0.25rem !important;
}

.recap-binome__orphan {
  font-size: 0.75rem;
  font-weight: 700;
  color: #6c6259;
}

.recap-binome__player {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.85rem;
}

.recap-binome__player > span {
  min-width: 0;
  overflow-wrap: anywhere;
}

.recap-binome__player.is-eliminated {
  text-decoration: line-through;
  opacity: 0.6;
}

.recap-binome__character {
  font-style: italic;
  text-align: right;
}

.recap-share {
  display: block;
  margin: 1.25rem auto 0;
}
</style>
