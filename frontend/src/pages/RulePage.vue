<script>
export default {
  name: "RulePage",
  data() {
    return {
      rules: [
        { title: "Objectif du jeu", description: "En cours d'écriture ..." },
        { title: "Déroulement d'un tour", description: "En cours d'écriture ..." },
        { title: "Conditions de victoire", description: "En cours d'écriture ..." },
      ],
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
}
</style>
