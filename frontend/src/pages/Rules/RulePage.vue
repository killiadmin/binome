<script>
export default {
  name: "RulePage",
  data() {
    return {
      rules: [
        {
          title: "🎯 Objectif du jeu",
          description:
              "Rester en jeu jusqu'au bout. Devine les personnages secrets des autres pour les éliminer un à un, et sois le dernier binôme encore debout.",
        },
        {
          title: "👥 Mise en place",
          description:
              "• Nombre de joueurs : pair, minimum 4\n" +
              "• Chaque joueur reçoit un personnage secret\n" +
              "• Chaque personnage a un nom, un univers (ex : Pokémon) et un cosmos, sa sur-catégorie (ex : Animé Japonais)\n" +
              "• Les joueurs sont regroupés en binômes partageant le même univers (sans le savoir)\n" +
              "• Chaque personnage possède 3 mots interdits",
        },
        {
          title: "🔄 Déroulement d’un tour",
          description:
              "À ton tour, tu peux faire UNE seule action :\n\n" +
              "1. Poser une question à un joueur (réponse libre : oui/non)\n" +
              "2. Faire une accusation : deviner le personnage d’un joueur\n\n" +
              "Les joueurs jouent chacun leur tour dans le même ordre à chaque round.",
        },
        {
          title: "🚫 Mots interdits",
          description:
              "• Chaque joueur a 3 mots interdits liés à son personnage, connus de lui seul\n" +
              "• Quand tu poses une question, ce sont les mots interdits de TA CIBLE qui comptent\n" +
              "• Si ta question en contient un :\n" +
              "  → La cible est libre de te mentir, et tu n'en sauras jamais rien\n" +
              "  → Ton tour est gâché sans que tu le saches\n\n" +
              "⚠️ Tu ignores les mots interdits des autres : c'est le risque à chaque question.\n" +
              "Seule la cible voit qu'un de ses mots a été prononcé — à elle de juger et de répondre.",
        },
        {
          title: "🎯 Accusation",
          description:
              "• Tu choisis un joueur et annonces son personnage\n" +
              "• Si tu as raison : le joueur accusé perd la partie et devient spectateur — il assiste à la suite sans plus pouvoir jouer\n" +
              "   → Son binôme, s'il est encore en jeu, n'est PAS révélé et continue de jouer\n" +
              "   → Mais son binôme est brisé : même s'il gagne, il ne touchera pas les points du binôme gagnant\n" +
              "• Sinon : rien ne se passe, le jeu continue",
        },
        {
          title: "🏆 Comment gagner",
          description:
              "• Un joueur correctement accusé est éliminé : il devient spectateur jusqu'à la fin\n" +
              "• La partie s'arrête dès qu'il ne reste qu'UN SEUL binôme complet, ses deux membres encore en jeu → ce binôme gagne\n" +
              "• Si plus aucun binôme n'est complet, le dernier survivant isolé gagne seul\n\n" +
              "Tu ne gagnes donc pas en devinant le plus vite : tu gagnes en restant en vie.\n" +
              "À la fin, tous les binômes et tous les personnages sont révélés.",
        },
        {
          title: "⭐ Marquer des points",
          description:
              "Tout le monde marque, gagnant comme éliminé :\n\n" +
              "• +1 par élimination — une accusation que la cible a confirmée\n" +
              "• +1 par round survécu — tous les rounds si tu n'es jamais éliminé, sinon jusqu'à ton round d'élimination inclus\n" +
              "• +5 si tu gagnes AVEC ton binôme intact (ton partenaire jamais éliminé)\n\n" +
              "⚠️ Une accusation ratée ne coûte rien : 0 point, aucune pénalité — mais elle gâche ton tour\n" +
              "⚠️ Un survivant isolé gagne la partie, mais pas le bonus de +5\n\n" +
              "Le classement final est trié par score : un éliminé très offensif peut finir devant un vainqueur discret.",
        },
      ]
    };
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
          <p class="rule-description">{{ rule.description }}</p>
        </div>
      </div>
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

.rules-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 1.25rem;
}

.rule-card {
  text-align: left;
  padding: 1.5rem 1.35rem;
}

.rule-title {
  font-family: 'Baloo 2', sans-serif;
  color: var(--arcade-blue-grey-dark);
  font-size: 1.2rem;
  font-weight: 800;
  margin-bottom: 0.6rem;
}

.rule-description {
  font-family: 'Baloo 2', sans-serif;
  color: #4a4438;
  font-size: 0.95rem;
  white-space: pre-line;
  line-height: 1.5;
  margin: 0;
}
</style>
