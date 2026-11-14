<script setup>
// Mentions légales (LCEN) et politique de confidentialité (RGPD), publiques.
// Garder ce texte aligné sur ce que l'application fait réellement : durée du
// chat (services/social.rs, CHAT_RETENTION_MINUTES), avatars publics,
// clés localStorage, droits exerçables en libre-service.
// L'adresse de contact est dans ContactEmail.vue (dévoilée au clic, anti-robots).
import ContactEmail from '../../components/legal/ContactEmail.vue'

const UPDATED_AT = '14 novembre 2026'
</script>

<template>
  <div class="legal-page arcade-bg">
    <div class="legal-page-inner">
      <div class="arcade-title-wrap">
        <h1 class="arcade-title">Mentions légales</h1>
        <p class="arcade-subtitle">et confidentialité · mise à jour le {{ UPDATED_AT }}</p>
      </div>

      <section class="board-card legal-card" aria-labelledby="legal-short">
        <h2 id="legal-short" class="legal-title">🔎 En bref</h2>
        <ul>
          <li>Binome est un jeu amateur, gratuit et sans publicité.</li>
          <li>On ne te demande ni e-mail ni vrai nom : un pseudo suffit.</li>
          <li>Aucun cookie, aucun traceur, aucune mesure d'audience.</li>
          <li>Tes données ne sont ni vendues ni transmises à qui que ce soit, hormis l'hébergeur du site.</li>
          <li>Tu peux demander à tout moment qu'on supprime tes données, par e-mail : <ContactEmail />.</li>
        </ul>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-editor">
        <h2 id="legal-editor" class="legal-title">✍️ Éditeur</h2>
        <p>
          Site personnel et non commercial édité par un particulier. Comme le permet l'article 6-III-2
          de la loi n° 2004-575 du 21 juin 2004 pour la confiance dans l'économie numérique (LCEN),
          l'éditeur ne publie pas son identité : elle a été communiquée à l'hébergeur.
        </p>
        <p>Contact par e-mail : <ContactEmail /></p>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-host">
        <h2 id="legal-host" class="legal-title">🖥️ Hébergeur</h2>
        <p>
          Railway Corporation<br>
          548 Market St PMB 68956, San Francisco, CA 94104, États-Unis<br>
          +1 (415) 707-7675 · <a href="https://railway.com" rel="noopener" target="_blank">railway.com</a>
        </p>
        <p>
          Les serveurs et la base de données sont situés dans l'Union européenne (région EU West,
          Amsterdam, Pays-Bas).
        </p>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-data">
        <h2 id="legal-data" class="legal-title">📦 Données traitées</h2>
        <p>Le responsable du traitement est l'éditeur, joignable à l'adresse e-mail ci-dessus.</p>
        <dl class="legal-list">
          <dt>Pseudo</dt>
          <dd>Il est obligatoire pour jouer et visible par les autres joueurs. Il figure aussi dans l'historique des parties.</dd>

          <dt>Mot de passe (facultatif)</dt>
          <dd>Il sert à protéger ton profil. Seule une empreinte irréversible (PBKDF2) est conservée, jamais le mot de passe lui-même.</dd>

          <dt>Photo de profil (facultative)</dt>
          <dd>
            Elle est visible par tous les joueurs. Attention : quelqu'un qui connaît son adresse peut l'afficher
            sans être connecté. Tu peux la retirer à tout moment depuis ton profil.
          </dd>

          <dt>Parties</dt>
          <dd>
            On conserve les questions et réponses, les accusations, les personnages tirés et les scores.
            Ces données alimentent l'historique et le classement.
          </dd>

          <dt>Chat du hall</dt>
          <dd>Les messages sont visibles par toutes les personnes connectées et sont <strong>effacés automatiquement au bout d'une heure</strong>.</dd>

          <dt>Boîte à idées</dt>
          <dd>Chaque idée est publiée avec ton pseudo, et tes +1 sont enregistrés. Tu peux supprimer tes idées toi-même.</dd>

          <dt>Emojis personnalisés</dt>
          <dd>
            Une image et un nom, ajoutés à la barre de réactions de tous les joueurs. Le pseudo de leur
            créateur est conservé. Ne prends personne en photo sans son accord.
          </dd>
        </dl>
        <p>
          L'application n'enregistre pas ton adresse IP. L'hébergeur peut tenir des journaux techniques
          qui la contiennent, pour assurer la sécurité de son infrastructure.
        </p>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-purpose">
        <h2 id="legal-purpose" class="legal-title">🎯 Pourquoi</h2>
        <ul>
          <li><strong>Faire fonctionner le jeu</strong> (profil, salons, parties, historique, chat, idées) : c'est le service que tu demandes en jouant.</li>
          <li><strong>Photo et emojis</strong> : ton consentement. Ils sont facultatifs et tu peux les retirer quand tu veux.</li>
          <li><strong>Prévenir les abus</strong> (limites d'envoi, filtre d'injures, validation des profils par l'administrateur) : l'intérêt légitime de garder le jeu agréable pour tous.</li>
        </ul>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-retention">
        <h2 id="legal-retention" class="legal-title">⏳ Durée de conservation</h2>
        <ul>
          <li>Messages du chat du hall : une heure.</li>
          <li>Salons : supprimés quand plus personne n'y est.</li>
          <li>
            Profil, photo, historique des parties, idées et emojis : tant que ton profil existe,
            ou jusqu'à ce que tu en demandes la suppression.
          </li>
        </ul>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-storage">
        <h2 id="legal-storage" class="legal-title">🍪 Cookies et stockage local</h2>
        <p>
          Le site ne dépose <strong>aucun cookie</strong> et n'utilise aucun traceur. Ton navigateur garde
          seulement, dans son stockage local, ce qu'il faut pour que le jeu marche : ton profil connecté,
          la partie en cours, tes notes de partie, tes filtres d'affichage et un identifiant de chat.
          Ce stockage est strictement nécessaire et ne demande donc pas de consentement. Tu peux l'effacer
          depuis les réglages de ton navigateur, mais tu seras alors déconnecté.
        </p>
        <p>Les polices de caractères sont hébergées sur le site lui-même : aucune requête n'est envoyée à un service tiers.</p>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-rights">
        <h2 id="legal-rights" class="legal-title">🛡️ Tes droits</h2>
        <p>
          Le RGPD te donne le droit d'accéder à tes données, de les rectifier, de les faire effacer,
          de t'opposer à leur traitement, d'en limiter l'usage et de les récupérer.
        </p>
        <ul>
          <li>Dans ton profil, tu peux changer de pseudo, retirer ta photo et gérer ton mot de passe.</li>
          <li>Dans la boîte à idées, tu peux supprimer tes idées toi-même.</li>
          <li>
            Pour tout le reste (suppression du profil, copie de tes données…), écris par
            e-mail (<ContactEmail />) en indiquant ton pseudo.
            On pourra te demander de prouver que le profil est bien le tien. Réponse sous un mois.
          </li>
        </ul>
        <p>
          Si tu estimes que tes droits ne sont pas respectés, tu peux adresser une réclamation à la CNIL
          (<a href="https://www.cnil.fr" rel="noopener" target="_blank">cnil.fr</a>).
        </p>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-transfer">
        <h2 id="legal-transfer" class="legal-title">🌍 Hors de l'Union européenne</h2>
        <p>
          Les données sont hébergées dans l'Union européenne, mais l'hébergeur est une société américaine.
          S'il devait y accéder depuis les États-Unis, ce transfert serait encadré par le Data Privacy
          Framework UE–États-Unis, auquel Railway adhère, et par des clauses contractuelles types.
        </p>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-minors">
        <h2 id="legal-minors" class="legal-title">🧒 Moins de 15 ans</h2>
        <p>
          Si tu as moins de 15 ans, demande l'accord d'un parent avant d'ajouter une photo de profil ou
          un emoji personnalisé.
        </p>
      </section>

      <section class="board-card legal-card" aria-labelledby="legal-ip">
        <h2 id="legal-ip" class="legal-title">©️ Propriété intellectuelle</h2>
        <p>
          Les personnages et univers cités dans le jeu appartiennent à leurs ayants droit respectifs.
          Binome est un projet amateur sans aucun lien avec eux. Les polices Press Start 2P et Baloo 2
          sont distribuées sous licence SIL Open Font License.
        </p>
      </section>
    </div>
  </div>
</template>

<style scoped>
.legal-page {
  min-height: 100vh;
  padding: 1.5rem 1rem 3rem;
}

.legal-page-inner {
  max-width: 760px;
  margin: 0 auto;
}

.arcade-title {
  font-size: clamp(1.1rem, 6vw, 1.9rem);
}

.arcade-subtitle {
  margin-top: 0.75rem;
}

.legal-card {
  text-align: left;
  padding: 1.5rem 1.35rem;
  margin-bottom: 1.5rem;
  font-family: 'Baloo 2', sans-serif;
  color: #4a4438;
  font-size: 0.95rem;
  line-height: 1.5;
  /* Adresse e-mail et URL : coupées plutôt que d'élargir la page à 320 px. */
  overflow-wrap: anywhere;
}

.legal-title {
  font-family: 'Baloo 2', sans-serif;
  color: var(--arcade-blue-grey-dark);
  font-size: 1.2rem;
  font-weight: 800;
  margin-bottom: 0.6rem;
}

.legal-card p:last-child,
.legal-card ul:last-child,
.legal-card dl:last-child {
  margin-bottom: 0;
}

.legal-card ul {
  padding-left: 1.2rem;
}

.legal-card li + li {
  margin-top: 0.3rem;
}

.legal-list dt {
  font-weight: 800;
  color: var(--arcade-blue-grey-dark);
}

.legal-list dd {
  margin: 0 0 0.6rem;
}

.legal-card a {
  color: var(--arcade-blue-grey-dark);
  font-weight: 700;
  text-decoration: underline;
}
</style>
