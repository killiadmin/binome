<script>
export default {
  name: "RulePage",
  data() {
    return {
      rules: [
        {
          title: "🎯 Objectif du jeu",
          description:
              "Deviner le personnage secret des autres joueurs avant que ton propre binôme soit découvert.",
        },
        {
          title: "👥 Mise en place",
          description:
              "• Nombre de joueurs : pair, minimum 4\n" +
              "• Chaque joueur reçoit un personnage secret\n" +
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
              "• Chaque joueur a 3 mots interdits liés à son personnage\n" +
              "• Si tu utilises un de TES mots interdits dans une question :\n" +
              "  → La question est refusée\n" +
              "  → Tu perds ton tour\n\n" +
              "⚠️ Seuls TES mots sont vérifiés, pas ceux de l’autre joueur.",
        },
        {
          title: "🎯 Accusation",
          description:
              "• Tu choisis un joueur et annonces son personnage\n" +
              "• Si tu as raison : son binôme est révélé\n" +
              "• Sinon : rien ne se passe, le jeu continue",
        },
        {
          title: "🏆 Conditions de victoire",
          description:
              "• La partie se termine quand tous les binômes sauf un sont découverts\n" +
              "• Le dernier binôme non découvert gagne la partie",
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
