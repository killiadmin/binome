<script setup>
import { ref, computed } from 'vue'
import { BContainer } from 'bootstrap-vue-next'
import { copyToClipboard } from '../../services/copyToClipboard'

// Prompts de génération de personnages, repris de
// backend/database/data/GENERATION.md — garder les deux synchronisés.

// ─── PARAMÈTRES ───────────────────────────────────────────────────────────────

const binomeCount = ref(10)
const theme = ref('')
const universe = ref('')
const universeCosmos = ref('')

const now = new Date()
const batchPrefix = `proposals-${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-batch-NN`

const safeCount = computed(() => Math.min(Math.max(parseInt(binomeCount.value, 10) || 1, 1), 50))

// ─── COMMANDES ────────────────────────────────────────────────────────────────

const CMD_LIST = `docker compose exec -T backend php artisan tinker --execute="App\\Models\\Character::with('universe')->get()->each(fn(\\$c)=>print(\\$c->universe->name.' / '.\\$c->name.(\\$c->hidden?' [refusé]':'').PHP_EOL));"`
const CMD_IMPORT = `docker compose exec -T backend php artisan characters:import database/data/${batchPrefix}.json`
const CMD_PENDING = `docker compose exec -T backend php artisan tinker --execute="echo App\\Models\\Character::pendingValidation()->get()->groupBy(fn(\\$c)=>\\$c->universe_id.'-'.\\$c->level_affectation)->count();"`

const RULES = `RÈGLES DE GÉNÉRATION
- Personnages TRÈS connus uniquement, cohérents avec leur univers et leur cosmos.
- Un binôme = 2 personnages du MÊME univers et du MÊME level_affectation (max 2 par niveau/univers).
- cosmos = sur-catégorie qui regroupe plusieurs univers
  (ex : Disney, Pixar, Marvel, DC Comics, Star Wars, Animé Japonais, Fantasy, Jeux vidéo, Bande dessinée, Série animée).
- 3 mots interdits par personnage, en français, fortement liés au personnage,
  si possible différents entre les 2 membres d'un binôme. Pas de champ "image".
- Format : tableau JSON d'objets { name, universe, cosmos, forbidden_words:[3 chaînes], level_affectation:entier }.`

const CONTEXT = `CONTEXTE
- Commande d'import : docker compose exec -T backend php artisan characters:import database/data/<fichier>.json
- Les personnages importés arrivent en verif_manual=false → je les valide ensuite un par un
  dans la popup de /admin/binomes (relecture + correction des mots interdits + import des images).
- L'import est idempotent (clé = univers + nom) et ignore automatiquement les personnages déjà refusés.`

const BEFORE = `AVANT DE GÉNÉRER
- Liste d'abord TOUS les personnages déjà en base, y compris les masqués (hidden), pour ne rien reproposer :
  ${CMD_LIST}`

const AFTER = `ENSUITE
- Écris le lot dans backend/database/data/proposals-AAAA-MM-batch-NN.json (incrémente NN par rapport au dernier).
- Lance l'import et vérifie le rapport (0 erreur attendu).
- Dis-moi combien de binômes sont désormais en attente de validation.`

// ─── PROMPTS ──────────────────────────────────────────────────────────────────

const prompts = computed(() => {
  const n = safeCount.value
  const themeLine = theme.value.trim() ? `\n\nCONSIGNE DE THÈME\n- ${theme.value.trim()}` : ''
  const uni = universe.value.trim() || '<univers>'
  const cosmosLine = universeCosmos.value.trim()
    ? `cosmos « ${universeCosmos.value.trim()} »`
    : 'cosmos adapté (réutilise celui déjà en base si l\'univers existe)'

  return [
    {
      id: 'batch',
      icon: 'fa-wand-magic-sparkles',
      title: 'Générer un lot complet',
      hint: 'Prompt principal : liste l\'existant, génère, écrit le fichier JSON et importe.',
      text: `Génère un nouveau lot de ${n * 2} personnages (soit ${n} binômes) pour le jeu Binome, puis importe-les.${themeLine}

${CONTEXT}

${RULES}

${BEFORE}

${AFTER}`,
    },
    {
      id: 'universe',
      icon: 'fa-earth-europe',
      title: 'Compléter un univers',
      hint: 'Ajoute des binômes à un univers précis (nouveaux niveaux de level_affectation).',
      text: `Génère ${n} nouveaux binômes (${n * 2} personnages) de l'univers « ${uni} », ${cosmosLine}, pour le jeu Binome, puis importe-les.

- Regarde d'abord quels level_affectation sont déjà occupés pour cet univers (2 personnages max par niveau)
  et utilise les niveaux libres suivants.${themeLine}

${CONTEXT}

${RULES}

${BEFORE}

${AFTER}`,
    },
    {
      id: 'words',
      icon: 'fa-ban',
      title: 'Revoir les mots interdits',
      hint: 'Relecture des mots interdits des personnages en attente de validation.',
      text: `Relis les mots interdits de tous les personnages en attente de validation (verif_manual=false, hidden=false) du jeu Binome.

RAPPEL
- Les mots interdits filtrent les questions posées par le joueur qui POSSÈDE le personnage :
  ce sont des mots trop révélateurs de son identité (3 par personnage, en français).
- Ils doivent être fortement liés au personnage et, si possible, différents entre les 2 membres d'un binôme.

À FAIRE
- Liste les personnages concernés avec leurs mots actuels.
- Propose une correction uniquement pour ceux dont les mots sont faibles, trop génériques ou dupliqués dans le binôme.
- N'applique rien sans mon accord : présente-moi un tableau avant / après.`,
    },
    {
      id: 'pending',
      icon: 'fa-list-check',
      title: 'État des propositions',
      hint: 'Point rapide sur ce qui reste à valider.',
      text: `Fais le point sur les personnages du jeu Binome :
- nombre de binômes en attente de validation :
  ${CMD_PENDING}
- répartition par cosmos / univers des personnages jouables (verif_manual=true, hidden=false),
- univers qui n'ont qu'un seul binôme jouable (candidats à compléter).`,
    },
  ]
})

const commands = [
  { id: 'list', title: 'Lister l\'existant (inclut les refusés)', text: CMD_LIST },
  { id: 'import', title: 'Importer un lot', text: CMD_IMPORT },
  { id: 'pending', title: 'Compter les binômes en attente', text: CMD_PENDING },
]

// ─── COPIE ────────────────────────────────────────────────────────────────────

const copied = ref(null)
const copyError = ref(null)

async function copy(id, text) {
  copyError.value = null
  if (await copyToClipboard(text)) {
    copied.value = id
    setTimeout(() => {
      if (copied.value === id) copied.value = null
    }, 2000)
  } else {
    copyError.value = 'Copie impossible : sélectionne le texte à la main.'
  }
}
</script>

<template>
  <BContainer class="admin-prompts py-4">
    <h1 class="page-title">Prompts</h1>
    <p class="page-subtitle">
      Prompts prêts à copier-coller dans Claude Code pour générer de nouveaux
      personnages. Les lots importés arrivent ensuite dans la popup de validation
      de <router-link to="/admin/binomes">Binômes</router-link>.
    </p>

    <section class="settings">
      <label class="field">
        <span>Nombre de binômes</span>
        <input v-model.number="binomeCount" type="number" min="1" max="50" class="input" />
      </label>
      <label class="field">
        <span>Consigne de thème (facultatif)</span>
        <input v-model="theme" type="text" class="input" placeholder="ex : plutôt des jeux vidéo, évite Disney" />
      </label>
      <label class="field">
        <span>Univers ciblé</span>
        <input v-model="universe" type="text" class="input" placeholder="ex : Pokémon" />
      </label>
      <label class="field">
        <span>Cosmos de l'univers</span>
        <input v-model="universeCosmos" type="text" class="input" placeholder="ex : Animé Japonais" />
      </label>
    </section>

    <p v-if="copyError" class="alert-line alert-line--error">{{ copyError }}</p>

    <ul class="prompt-list">
      <li v-for="p in prompts" :key="p.id" class="prompt-card">
        <div class="prompt-head">
          <div class="prompt-title">
            <i class="fa-solid" :class="p.icon"></i>
            <h2>{{ p.title }}</h2>
          </div>
          <button
            type="button"
            class="cabinet-btn cabinet-btn--sm"
            :class="{ 'is-done': copied === p.id }"
            @click="copy(p.id, p.text)"
          >
            <i class="fa-solid" :class="copied === p.id ? 'fa-check' : 'fa-copy'"></i>
            {{ copied === p.id ? 'Copié !' : 'Copier' }}
          </button>
        </div>
        <p class="muted hint">{{ p.hint }}</p>
        <pre class="prompt-text">{{ p.text }}</pre>
      </li>
    </ul>

    <h2 class="section-title">Commandes utiles</h2>
    <ul class="prompt-list">
      <li v-for="c in commands" :key="c.id" class="prompt-card">
        <div class="prompt-head">
          <h3 class="cmd-title">{{ c.title }}</h3>
          <button
            type="button"
            class="cabinet-btn cabinet-btn--ghost cabinet-btn--sm"
            @click="copy('cmd-' + c.id, c.text)"
          >
            <i class="fa-solid" :class="copied === 'cmd-' + c.id ? 'fa-check' : 'fa-copy'"></i>
            {{ copied === 'cmd-' + c.id ? 'Copié !' : 'Copier' }}
          </button>
        </div>
        <pre class="prompt-text prompt-text--cmd">{{ c.text }}</pre>
      </li>
    </ul>
  </BContainer>
</template>

<style scoped>
.admin-prompts {
  max-width: 900px;
  color: var(--arcade-beige);
}

.page-title {
  font-family: 'Press Start 2P', cursive;
  font-size: 1rem;
  margin-bottom: 0.5rem;
}

.page-subtitle {
  color: var(--arcade-taupe);
  margin-bottom: 1.25rem;
}

.page-subtitle a {
  color: var(--arcade-gold);
}

.section-title {
  font-size: 0.95rem;
  font-weight: 700;
  margin: 1.5rem 0 0.75rem;
}

.settings {
  display: grid;
  grid-template-columns: 1fr;
  gap: 0.75rem;
  margin-bottom: 1.25rem;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-width: 0;
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--arcade-taupe);
}

.input {
  width: 100%;
  min-width: 0;
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 8px;
  color: var(--arcade-beige);
  padding: 0.4rem 0.6rem;
  font-weight: 400;
}

.input:focus {
  outline: none;
  border-color: var(--arcade-blue-grey);
}

.alert-line {
  border-radius: 8px;
  padding: 0.5rem 0.75rem;
  font-weight: 700;
}

.alert-line--error {
  background: var(--arcade-danger-dark);
  color: #fff;
}

.muted {
  color: var(--arcade-taupe);
}

.prompt-list {
  list-style: none;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.prompt-card {
  background: var(--arcade-dark-2);
  border: 2px solid var(--arcade-blue-grey-dark);
  border-radius: 12px;
  padding: 0.9rem 1rem;
  min-width: 0;
}

.prompt-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.prompt-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.prompt-title h2,
.cmd-title {
  font-size: 0.95rem;
  font-weight: 700;
  margin: 0;
}

.hint {
  font-size: 0.85rem;
  margin: 0.35rem 0 0.5rem;
}

.prompt-text {
  background: rgba(0, 0, 0, 0.3);
  border-radius: 8px;
  padding: 0.6rem 0.75rem;
  margin: 0;
  max-height: 260px;
  overflow: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  font-size: 0.75rem;
  color: var(--arcade-beige);
  text-align: left;
  user-select: all;
}

.prompt-text--cmd {
  max-height: none;
  margin-top: 0.5rem;
}

.cabinet-btn.is-done {
  background: var(--arcade-success);
}

@media (min-width: 576px) {
  .settings {
    grid-template-columns: 1fr 1fr;
  }
}
</style>
