<script>
/*
 * Chaque règle = une suite de blocs :
 *   { p: "texte" }            paragraphe
 *   { ul: [item…] }           liste à puces
 *   { ol: [item…] }           liste numérotée
 *   { note: "texte" }         encadré d'avertissement / de minuteur
 * Un item est une chaîne, ou { text, sub: [chaîne…] } pour une sous-liste
 * (affichée en retrait, avec des flèches).
 *
 * Sous les règles, « Univers jouables » : vignettes des cosmos activés par
 * l'admin (GET /cosmos), un clic ouvre leur fiche de présentation.
 */
import { BModal } from "bootstrap-vue-next";
import { characterService } from "../../services/characterService";
import UniverseGallery from "../../components/universe/UniverseGallery.vue";
import UniverseSheet from "../../components/universe/UniverseSheet.vue";

export default {
  name: "RulePage",
  components: { BModal, UniverseGallery, UniverseSheet },
  data() {
    return {
      universes: [],
      universesLoading: true,
      universesError: false,
      openedUniverse: null,
      rules: [
        {
          title: "🎯 Objectif du jeu",
          blocks: [
            { p: "Rester en jeu jusqu'au bout. Devine les personnages secrets des autres pour les éliminer un à un, et sois le dernier binôme encore debout." },
          ],
        },
        {
          title: "👥 Mise en place",
          blocks: [
            {
              ul: [
                "Nombre de joueurs : minimum 4, pair ou impair",
                "Chaque joueur reçoit un personnage secret",
                {
                  text: "Chaque personnage a :",
                  sub: [
                    "un nom",
                    "un univers (ex : Pokémon)",
                    "un cosmos, sa sur-catégorie (ex : Animé Japonais)",
                    "3 mots interdits",
                  ],
                },
                "Les joueurs sont regroupés en binômes partageant le même univers (sans le savoir)",
              ],
            },
          ],
        },
        {
          title: "🕳️ L’orphelin (nombre impair)",
          blocks: [
            { p: "Si vous êtes en nombre impair, un joueur tiré au sort n'a aucun binôme : c'est l'orphelin." },
            {
              ul: [
                "Tout le monde sait qu'un orphelin existe, personne ne sait qui c'est",
                "L'orphelin lui-même l'ignore : il a un personnage et un univers comme les autres, et cherche un partenaire qui n'existe pas",
                "Il joue et peut être accusé exactement comme les autres",
                "Il compte comme une équipe à lui tout seul : s'il est le dernier en jeu, il gagne et touche le bonus complet, comme un binôme intact",
                "Tout est révélé à la fin : son identité apparaît au tableau des scores",
              ],
            },
          ],
        },
        {
          title: "🔄 Déroulement d’un tour",
          blocks: [
            { p: "À ton tour, tu peux faire UNE seule action :" },
            {
              ol: [
                "Poser une question à un joueur (il répond oui, non ou « je ne sais pas »)",
                "Faire une accusation : deviner le personnage d’un joueur",
              ],
            },
            { p: "Les joueurs jouent chacun leur tour dans le même ordre à chaque round." },
          ],
        },
        {
          title: "🃏 « Je ne sais pas »",
          blocks: [
            { p: "Si le joueur interrogé répond « Je ne sais pas », celui qui a posé la question tire une carte :" },
            {
              ul: [
                "Il choisit une carte face cachée parmi les mots interdits du joueur interrogé",
                "Il est le seul à voir le mot, pendant 5 secondes : à lui de le retenir !",
                {
                  text: "Ensuite, au choix :",
                  sub: [
                    "il le montre à tous les autres joueurs (5 secondes aussi)… sauf au joueur interrogé",
                    "ou il le garde pour lui",
                  ],
                },
                "Le joueur interrogé sait qu'une carte a été tirée, mais jamais laquelle ni si elle a été montrée",
              ],
            },
            { note: "⏳ 30 secondes pour tirer et décider, sinon la carte est perdue." },
          ],
        },
        {
          title: "🚫 Mots interdits",
          blocks: [
            {
              ul: [
                "Chaque joueur a 3 mots interdits liés à son personnage, connus de lui seul",
                "Quand tu poses une question, ce sont les mots interdits de TA CIBLE qui comptent",
                {
                  text: "Si ta question contient un de ses mots TEL QUEL (accents, majuscules et pluriel ignorés) :",
                  sub: [
                    "La cible DOIT te mentir, et tu n'en sauras jamais rien",
                    "Ton tour est gâché sans que tu le saches",
                  ],
                },
                {
                  text: "Si ta question contient un mot TRÈS PROCHE (faute de frappe, conjugaison : « vol » pour « voler ») :",
                  sub: [
                    "La cible PEUT te mentir : le mensonge est légal, à elle de juger",
                  ],
                },
              ],
            },
            { note: "⚠️ Tu ignores les mots interdits des autres : c'est le risque à chaque question." },
            { p: "Seule la cible voit qu'un de ses mots a été prononcé. Mentir un coup sur deux selon ce qui l'arrange est interdit : seule une faute de frappe évidente ou un cas exceptionnel l'autorise à répondre librement." },
          ],
        },
        {
          title: "🎯 Accusation",
          blocks: [
            {
              ul: [
                "Tu choisis un joueur et annonces son personnage",
                {
                  text: "Si tu as raison : le joueur accusé perd la partie et devient spectateur — il assiste à la suite sans plus pouvoir jouer",
                  sub: [
                    "Son binôme, s'il est encore en jeu, n'est PAS révélé et continue de jouer",
                    "Mais son binôme est brisé : même s'il gagne, il ne touchera pas les points du binôme gagnant",
                  ],
                },
                "Sinon : rien ne se passe, le jeu continue",
              ],
            },
          ],
        },
        {
          title: "🏆 Comment gagner",
          blocks: [
            {
              ul: [
                {
                  text: "Un joueur correctement accusé est éliminé : il devient spectateur jusqu'à la fin",
                  sub: [
                    "Selon les options de l'hôte, il peut voir tous les personnages (vue omnisciente) et voter pour la meilleure question de chaque round",
                  ],
                },
                {
                  text: "La partie s'arrête dès qu'il ne reste qu'UNE SEULE équipe intacte, et cette équipe gagne. Une équipe intacte, c'est :",
                  sub: [
                    "un binôme dont les deux membres sont encore en jeu",
                    "ou l'orphelin encore en vie",
                  ],
                },
                "Si plus aucune équipe n'est intacte, le dernier survivant isolé gagne seul",
              ],
            },
            { p: "Tu ne gagnes donc pas en devinant le plus vite : tu gagnes en restant en vie. À la fin, tous les binômes et tous les personnages sont révélés." },
          ],
        },
        {
          title: "⭐ Marquer des points",
          blocks: [
            { p: "Tout le monde marque, gagnant comme éliminé :" },
            {
              ul: [
                "+1 par élimination — une accusation que la cible a confirmée",
                {
                  text: "+1 par round survécu :",
                  sub: [
                    "tous les rounds si tu n'es jamais éliminé",
                    "sinon jusqu'à ton round d'élimination inclus",
                  ],
                },
                {
                  text: "+5 si tu gagnes AVEC ton équipe intacte :",
                  sub: [
                    "ton binôme dont le partenaire n'a jamais été éliminé",
                    "ou l'orphelin qui va au bout",
                  ],
                },
                "+3 si tu gagnes seul, en rescapé (ton partenaire a été éliminé)",
                "+1 par round où ta question est élue meilleure question par les éliminés (si l'hôte a activé le vote)",
              ],
            },
            { note: "⚠️ Une accusation ratée ne coûte rien : 0 point, aucune pénalité — mais elle gâche ton tour" },
            { note: "⚠️ Un rescapé dont le partenaire a été éliminé gagne la partie, mais touche +3 au lieu de +5" },
            { note: "⚠️ L'orphelin, lui, n'a jamais eu de partenaire à perdre : son bonus est acquis s'il gagne" },
            { p: "Le classement final est trié par score : un éliminé très offensif peut finir devant un vainqueur discret." },
          ],
        },
      ]
    };
  },
  computed: {
    showUniverse: {
      get() {
        return this.openedUniverse !== null;
      },
      set(v) {
        if (!v) this.openedUniverse = null;
      },
    },
  },
  async mounted() {
    try {
      const { data } = await characterService.listCosmos();
      this.universes = (data.cosmos ?? []).filter((c) => c.is_enabled);
    } catch {
      this.universesError = true;
    } finally {
      this.universesLoading = false;
    }
  },
};
</script>

<template>
  <div class="rule-page arcade-bg">
    <div class="rule-page-inner">
      <div class="arcade-title-wrap">
        <h1 class="arcade-title">Règles du jeu</h1>
        <p class="arcade-subtitle">Tout ce qu'il faut savoir avant de jouer</p>
      </div>

      <div class="rules-grid">
        <div v-for="(rule, index) in rules" :key="index" class="board-card rule-card">
          <div class="board-card__rivet board-card__rivet--tl"></div>
          <div class="board-card__rivet board-card__rivet--tr"></div>
          <div class="board-card__rivet board-card__rivet--bl"></div>
          <div class="board-card__rivet board-card__rivet--br"></div>
          <h2 class="rule-title">{{ rule.title }}</h2>
          <div class="rule-body">
            <template v-for="(block, b) in rule.blocks" :key="b">
              <p v-if="block.p" class="rule-p">{{ block.p }}</p>
              <p v-else-if="block.note" class="rule-note">{{ block.note }}</p>
              <component
                  :is="block.ol ? 'ol' : 'ul'"
                  v-else
                  class="rule-list"
                  :class="block.ol ? 'rule-list--ol' : 'rule-list--ul'"
              >
                <li v-for="(item, i) in block.ol || block.ul" :key="i" class="rule-item">
                  <span>{{ typeof item === 'string' ? item : item.text }}</span>
                  <ul v-if="item.sub" class="rule-sublist">
                    <li v-for="(sub, s) in item.sub" :key="s" class="rule-subitem">{{ sub }}</li>
                  </ul>
                </li>
              </component>
            </template>
          </div>
        </div>
      </div>

      <section id="univers" class="board-card rule-card universes-card" aria-labelledby="universes-title">
        <div class="board-card__rivet board-card__rivet--tl"></div>
        <div class="board-card__rivet board-card__rivet--tr"></div>
        <div class="board-card__rivet board-card__rivet--bl"></div>
        <div class="board-card__rivet board-card__rivet--br"></div>
        <h2 id="universes-title" class="rule-title">🌍 Univers jouables</h2>
        <div class="rule-body">
          <p class="rule-p">Les univers dans lesquels l'hôte peut piocher les personnages. Touche une vignette pour lire sa fiche : elle te dit jusqu'où chercher pendant ton enquête.</p>
          <p v-if="universesLoading" class="rule-p universes-status">Chargement…</p>
          <p v-else-if="universesError" class="rule-p universes-status">Impossible de charger les univers pour le moment.</p>
          <p v-else-if="universes.length === 0" class="rule-p universes-status">Aucun univers jouable pour le moment.</p>
          <UniverseGallery v-else :cosmos="universes" @open="openedUniverse = $event" />
        </div>
      </section>

      <BModal v-model="showUniverse" title="Fiche de l'univers" no-footer class="arcade-modal">
        <UniverseSheet v-if="openedUniverse" :cosmos="openedUniverse" heading-tag="h3" />
      </BModal>
    </div>
  </div>
</template>

<style scoped>
.rule-page {
  min-height: 100vh;
  padding: 1.5rem 1rem 3rem;
}

.rule-page-inner {
  max-width: 900px;
  margin: 0 auto;
}

.arcade-title {
  font-size: clamp(1.25rem, 6.5vw, 1.9rem);
}

.arcade-subtitle {
  margin-top: 0.75rem;
}

.rules-grid {
  display: grid;
  /* min(280px, 100%) : sous 280px de large la piste suit le conteneur au lieu
     de forcer une colonne plus large que l'écran. */
  grid-template-columns: repeat(auto-fit, minmax(min(280px, 100%), 1fr));
  gap: 1.25rem;
}

.rule-card {
  text-align: left;
  padding: 1.5rem 1.35rem;
}

.universes-card {
  margin-top: 1.25rem;
}

.universes-status {
  font-style: italic;
  color: var(--arcade-taupe);
}

.rule-title {
  font-family: 'Baloo 2', sans-serif;
  color: var(--arcade-blue-grey-dark);
  font-size: 1.2rem;
  font-weight: 800;
  margin-bottom: 0.6rem;
}

.rule-body {
  font-family: 'Baloo 2', sans-serif;
  color: #4a4438;
  font-size: 0.95rem;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.rule-body > * + * {
  margin-top: 0.6rem;
}

.rule-p {
  margin: 0;
}

/* Encadré ⚠️ / ⏳ : se détache du texte courant. */
.rule-note {
  margin: 0;
  padding: 0.4rem 0.6rem;
  background: rgba(224, 163, 28, 0.12);
  border-left: 3px solid var(--arcade-gold);
  border-radius: 4px;
}

.rule-body > .rule-note + .rule-note {
  margin-top: 0.35rem;
}

/* Listes : puces et numéros dessinés à la main pour un retrait identique
   quelle que soit la police. */
.rule-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.rule-item {
  position: relative;
  padding-left: 1.1rem;
}

.rule-item + .rule-item {
  margin-top: 0.35rem;
}

.rule-list--ul > .rule-item::before {
  content: "•";
  position: absolute;
  left: 0.15rem;
  font-weight: 800;
  color: var(--arcade-blue-grey-dark);
}

.rule-list--ol {
  counter-reset: rule-step;
}

.rule-list--ol > .rule-item {
  counter-increment: rule-step;
  padding-left: 1.4rem;
}

.rule-list--ol > .rule-item::before {
  content: counter(rule-step) ".";
  position: absolute;
  left: 0;
  font-weight: 800;
  color: var(--arcade-blue-grey-dark);
}

/* Sous-catégories : retrait + filet vertical qui les rattache à leur puce,
   flèche devant chaque ligne. */
.rule-sublist {
  list-style: none;
  margin: 0.3rem 0 0 0.2rem;
  padding: 0 0 0 0.75rem;
  border-left: 2px solid rgba(71, 87, 95, 0.25);
  font-size: 0.92rem;
}

.rule-subitem {
  position: relative;
  padding-left: 1.15rem;
}

.rule-subitem + .rule-subitem {
  margin-top: 0.2rem;
}

.rule-subitem::before {
  content: "→";
  position: absolute;
  left: 0;
  color: var(--arcade-taupe);
  font-weight: 700;
}
</style>
