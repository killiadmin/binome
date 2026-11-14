<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { adminService } from '../../services/adminService'

// Réglages du jeu (onglet Parties) : modes de jeu proposés aux hôtes, cosmos
// disponibles, binômes de personnages tirables. Le serveur les applique au
// choix des réglages du salon et au lancement de la partie.

const modes = ref([])
const cosmos = ref([])
const binomes = ref([])
const loading = ref(true)
const error = ref(null)
const success = ref(null)
const busy = reactive({})
// Cosmos dépliés (liste de leurs binômes)
const open = reactive({})

const NO_COSMOS = 'none'

async function load() {
  loading.value = true
  error.value = null
  try {
    const { data } = await adminService.getSettings()
    modes.value = data.game_modes
    cosmos.value = data.cosmos
    binomes.value = data.binomes
  } catch (e) {
    error.value = e.response?.data?.message || 'Impossible de charger les réglages du jeu.'
  } finally {
    loading.value = false
  }
}

function flash(message) {
  success.value = message
  setTimeout(() => (success.value = null), 3000)
}

// Binômes rangés par cosmos ; les univers sans cosmos à part.
const binomesByCosmos = computed(() => {
  const map = new Map()
  for (const b of binomes.value) {
    const key = b.cosmos_id ?? NO_COSMOS
    if (!map.has(key)) map.set(key, [])
    map.get(key).push(b)
  }
  return map
})

const looseBinomes = computed(() => binomesByCosmos.value.get(NO_COSMOS) ?? [])

function enabledCount(key) {
  return (binomesByCosmos.value.get(key) ?? []).filter((b) => b.is_enabled).length
}

function totalCount(key) {
  return (binomesByCosmos.value.get(key) ?? []).length
}

const enabledModesCount = computed(() => modes.value.filter((m) => m.enabled).length)
const enabledCosmosCount = computed(() => cosmos.value.filter((c) => c.is_enabled).length)

// Univers jouables d'un cosmos : au moins un binôme actif.
function playableUniverses(key) {
  return new Set((binomesByCosmos.value.get(key) ?? []).filter((b) => b.is_enabled).map((b) => b.universe_id)).size
}

async function run(key, fn) {
  if (busy[key]) return
  busy[key] = true
  error.value = null
  try {
    const { data } = await fn()
    flash(data.message)
    return data
  } catch (e) {
    error.value = e.response?.data?.message || 'La modification a échoué.'
  } finally {
    busy[key] = false
  }
}

async function toggleMode(mode) {
  const data = await run(`mode-${mode.key}`, () => adminService.setGameMode(mode.key, !mode.enabled))
  if (data) mode.enabled = data.game_modes.includes(mode.key)
}

async function toggleCosmos(c) {
  const data = await run(`cosmos-${c.id}`, () => adminService.setCosmos(c.id, !c.is_enabled))
  if (data) c.is_enabled = data.cosmos.is_enabled
}

function binomeKey(b) {
  return `binome-${b.universe_id}-${b.level_affectation}`
}

async function toggleBinome(b) {
  const data = await run(binomeKey(b), () => adminService.setBinome(b.universe_id, b.level_affectation, !b.is_enabled))
  if (data) b.is_enabled = data.binome.is_enabled
}

onMounted(load)
</script>

<template>
  <section class="controls" aria-labelledby="controls-title">
    <div class="controls-head">
      <h2 id="controls-title" class="controls-title">Réglages du jeu</h2>
      <button type="button" class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm" :disabled="loading" @click="load">
        <i class="fa-solid fa-rotate-right"></i> Rafraîchir
      </button>
    </div>
    <p class="muted hint">
      Ce que les hôtes peuvent choisir dans leur salon. Une partie déjà lancée
      n'est pas affectée.
    </p>

    <p v-if="error" class="alert-line alert-line--error">{{ error }}</p>
    <p v-if="success" class="alert-line alert-line--success">{{ success }}</p>
    <p v-if="loading" class="muted">Chargement…</p>

    <template v-else>
      <!-- Modes de jeu -->
      <h3 class="block-title">Modes de jeu <span class="muted">· {{ enabledModesCount }}/{{ modes.length }} actifs</span></h3>
      <div class="switch-list">
        <button
          v-for="m in modes"
          :key="m.key"
          type="button"
          class="switch-row"
          :class="{ 'is-on': m.enabled }"
          role="switch"
          :aria-checked="m.enabled"
          :disabled="busy[`mode-${m.key}`] || (m.enabled && enabledModesCount === 1)"
          :title="m.enabled && enabledModesCount === 1 ? 'Au moins un mode doit rester disponible' : ''"
          @click="toggleMode(m)"
        >
          <span class="switch-label">
            <i class="fa-solid" :class="m.key === 'cosmos' ? 'fa-globe' : 'fa-shuffle'"></i>
            {{ m.label }}
          </span>
          <span class="switch" aria-hidden="true"><span class="switch-knob"></span></span>
        </button>
      </div>

      <!-- Cosmos et leurs binômes -->
      <h3 class="block-title">Cosmos <span class="muted">· {{ enabledCosmosCount }}/{{ cosmos.length }} actifs</span></h3>
      <p class="muted hint">
        Un cosmos désactivé n'est plus proposé aux hôtes ni tiré en mode Aléatoire.
        Déplie un cosmos pour activer ou désactiver ses binômes.
      </p>
      <ul class="cosmos-list">
        <li v-for="c in cosmos" :key="c.id" class="cosmos-item" :class="{ 'is-off': !c.is_enabled }">
          <div class="cosmos-row">
            <button
              type="button"
              class="cosmos-expand"
              :aria-expanded="!!open[c.id]"
              :disabled="totalCount(c.id) === 0"
              @click="open[c.id] = !open[c.id]"
            >
              <i class="fa-solid" :class="open[c.id] ? 'fa-chevron-down' : 'fa-chevron-right'"></i>
              <span class="cosmos-name">{{ c.name }}</span>
              <span class="cosmos-meta">
                {{ enabledCount(c.id) }}/{{ totalCount(c.id) }} binôme{{ totalCount(c.id) > 1 ? 's' : '' }}
                · {{ playableUniverses(c.id) }} univers jouable{{ playableUniverses(c.id) > 1 ? 's' : '' }}
              </span>
            </button>
            <button
              type="button"
              class="switch"
              :class="{ 'is-on': c.is_enabled }"
              role="switch"
              :aria-checked="c.is_enabled"
              :aria-label="`${c.is_enabled ? 'Désactiver' : 'Activer'} le cosmos ${c.name}`"
              :disabled="busy[`cosmos-${c.id}`]"
              @click="toggleCosmos(c)"
            >
              <span class="switch-knob"></span>
            </button>
          </div>

          <ul v-if="open[c.id]" class="binome-list">
            <li v-for="b in binomesByCosmos.get(c.id)" :key="binomeKey(b)" class="binome-row" :class="{ 'is-off': !b.is_enabled }">
              <div class="binome-text">
                <span class="binome-universe">{{ b.universe_name }} <span class="muted">· niv. {{ b.level_affectation }}</span></span>
                <span class="binome-characters">{{ b.characters.join(' & ') }}</span>
              </div>
              <button
                type="button"
                class="switch"
                :class="{ 'is-on': b.is_enabled }"
                role="switch"
                :aria-checked="b.is_enabled"
                :aria-label="`${b.is_enabled ? 'Désactiver' : 'Activer'} le binôme ${b.universe_name} niveau ${b.level_affectation}`"
                :disabled="busy[binomeKey(b)]"
                @click="toggleBinome(b)"
              >
                <span class="switch-knob"></span>
              </button>
            </li>
          </ul>
        </li>

        <!-- Univers sans cosmos : tirés seulement en mode Aléatoire -->
        <li v-if="looseBinomes.length" class="cosmos-item">
          <div class="cosmos-row">
            <button type="button" class="cosmos-expand" :aria-expanded="!!open[NO_COSMOS]" @click="open[NO_COSMOS] = !open[NO_COSMOS]">
              <i class="fa-solid" :class="open[NO_COSMOS] ? 'fa-chevron-down' : 'fa-chevron-right'"></i>
              <span class="cosmos-name">Sans cosmos</span>
              <span class="cosmos-meta">{{ enabledCount(NO_COSMOS) }}/{{ looseBinomes.length }} binômes · mode Aléatoire seulement</span>
            </button>
          </div>
          <ul v-if="open[NO_COSMOS]" class="binome-list">
            <li v-for="b in looseBinomes" :key="binomeKey(b)" class="binome-row" :class="{ 'is-off': !b.is_enabled }">
              <div class="binome-text">
                <span class="binome-universe">{{ b.universe_name }} <span class="muted">· niv. {{ b.level_affectation }}</span></span>
                <span class="binome-characters">{{ b.characters.join(' & ') }}</span>
              </div>
              <button
                type="button"
                class="switch"
                :class="{ 'is-on': b.is_enabled }"
                role="switch"
                :aria-checked="b.is_enabled"
                :aria-label="`${b.is_enabled ? 'Désactiver' : 'Activer'} le binôme ${b.universe_name} niveau ${b.level_affectation}`"
                :disabled="busy[binomeKey(b)]"
                @click="toggleBinome(b)"
              >
                <span class="switch-knob"></span>
              </button>
            </li>
          </ul>
        </li>
      </ul>
    </template>
  </section>
</template>

<style scoped>
.controls {
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.9rem 1rem;
  margin-bottom: 1.5rem;
}

.controls-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.controls-title {
  font-family: 'Press Start 2P', cursive;
  font-size: 0.8rem;
  margin: 0;
}

.block-title {
  font-size: 0.95rem;
  font-weight: 700;
  margin: 1rem 0 0.4rem;
}

.hint {
  font-size: 0.85rem;
  margin: 0.35rem 0 0.5rem;
}

.muted {
  color: var(--arcade-taupe);
  font-weight: 400;
}

.alert-line {
  border-radius: 8px;
  padding: 0.5rem 0.75rem;
  font-weight: 700;
}
.alert-line--error { background: var(--arcade-danger-dark); color: #fff; }
.alert-line--success { background: var(--arcade-success-dark); color: #fff; }

/* Interrupteur : piste + bouton rond, vert quand actif */
.switch {
  flex: none;
  position: relative;
  width: 44px;
  height: 24px;
  padding: 0;
  border: 2px solid var(--arcade-blue-grey);
  border-radius: 999px;
  background: var(--arcade-dark);
  cursor: pointer;
}
.switch-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--arcade-taupe);
  transition: transform 0.15s ease;
}
.switch.is-on,
.switch-row.is-on .switch {
  background: var(--arcade-success);
  border-color: var(--arcade-success);
}
.switch.is-on .switch-knob,
.switch-row.is-on .switch-knob {
  transform: translateX(20px);
  background: #fff;
}
.switch:disabled { opacity: 0.5; cursor: default; }

.switch-list {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  width: 100%;
  min-height: 44px;
  padding: 0.4rem 0.7rem;
  background: rgba(0, 0, 0, 0.25);
  border: none;
  border-radius: 8px;
  color: var(--arcade-beige);
  font-weight: 700;
  text-align: left;
  cursor: pointer;
}
.switch-row:disabled { cursor: default; }
.switch-row:disabled .switch { opacity: 0.5; }
.switch-label i { width: 1.2em; }

.cosmos-list,
.binome-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.cosmos-list {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.cosmos-item {
  background: rgba(0, 0, 0, 0.25);
  border-radius: 8px;
  padding: 0.3rem 0.7rem 0.3rem 0.3rem;
}

.cosmos-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.cosmos-expand {
  flex: 1;
  min-width: 0;
  min-height: 44px;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0 0.5rem;
  padding: 0.3rem 0.4rem;
  background: transparent;
  border: none;
  color: var(--arcade-beige);
  text-align: left;
  cursor: pointer;
}
.cosmos-expand:disabled { cursor: default; }
.cosmos-expand i { width: 0.9em; color: var(--arcade-taupe); }

.cosmos-name { font-weight: 700; overflow-wrap: anywhere; }

.cosmos-meta {
  flex-basis: 100%;
  padding-left: 1.4em;
  font-size: 0.75rem;
  color: var(--arcade-taupe);
}

.cosmos-item.is-off .cosmos-name { color: var(--arcade-taupe); text-decoration: line-through; }

.binome-list {
  margin: 0.2rem 0 0.4rem 1.6rem;
  border-left: 2px dashed var(--arcade-blue-grey-dark);
}

.binome-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.35rem 0 0.35rem 0.6rem;
}

.binome-text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  font-size: 0.85rem;
}

.binome-universe { font-weight: 700; overflow-wrap: anywhere; }
.binome-characters { color: var(--arcade-taupe); overflow-wrap: anywhere; }
.binome-row.is-off .binome-text { opacity: 0.55; }
</style>
