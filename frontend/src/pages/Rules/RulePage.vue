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
  <div class="rule-page d-flex flex-column rounded text-center shadow p-5 bg-color-beige">
    <h1 class="title color-taupe fs-3 mb-5">📜 Règles du jeu</h1>
    <div class="text-start">
      <div v-for="(rule, index) in rules" :key="index" class="rule">
        <h2 class="rule-title">{{ rule.title }}</h2>
        <p class="rule-description">{{ rule.description }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.rule-page {
  margin: 50px auto;
  max-width: 800px;
}

.rule {
  background: white;
  padding: 15px;
  margin: 10px 0;
  border-radius: 8px;
  box-shadow: 0px 2px 5px rgba(0, 0, 0, 0.1);
}

.rule-title {
  color: #5e3b1f;
  font-size: 1.5em;
}

.rule-description {
  color: #333;
  font-size: 1.1em;
  white-space: pre-line;
}
</style>
