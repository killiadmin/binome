<script setup>
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { useGameSocket } from '../../components/useGameSocket';
import { BButton, BCard, BContainer, BRow, BCol, BModal, BFormInput } from "bootstrap-vue-next";

const router = useRouter();
const playerName = ref('');
const showModal = ref(false);
const selectedParticipants = ref([]);
const showCreateModal = ref(false);
const showJoinModal = ref(false);
const codeToJoin = ref('');
const selectedTheme = ref(null);

const themes = ref([
  {
    id: 1,
    name: "Disney",
    description: "Plongez dans l'univers magique de Disney.",
    icon: "fa-solid fa-wand-magic-sparkles",
  },
  {
    id: 2,
    name: "Histoire",
    description: "Voyagez à travers le temps et les époques.",
    icon: "fa-solid fa-landmark",
  }
]);

const {
  connected,
  gameCode,
  players,
  gameStatus,
  isHost,
  hostId,
  error,
  connect,
  createGame,
  joinGame,
  startGame
} = useGameSocket();

onMounted(() => {
  connect();
});

const handleCreateGame = () => {
  if (!playerName.value.trim()) {
    alert('Veuillez entrer votre nom');
    return;
  }
  createGame(playerName.value);
  showCreateModal.value = false;
};

const handleJoinGame = () => {
  if (!playerName.value.trim() || !codeToJoin.value.trim()) {
    alert('Veuillez entrer votre nom et le code de la partie');
    return;
  }
  joinGame(codeToJoin.value.toUpperCase(), playerName.value);
  showJoinModal.value = false;
};

const selectTheme = (theme) => {
  selectedTheme.value = theme;
};

const handleStartGame = () => {
  if (!selectedTheme.value) {
    alert('Veuillez sélectionner un thème avant de démarrer');
    return;
  }
  if (isHost.value && players.value.length > 0) {
    startGame();
    router.push({
      name: 'GamePage',
      params: { themeId: selectedTheme.value.id }
    });
  }
};

const getGameStatusText = (status) => {
  return status === 'playing' ? 'En cours' : 'En attente';
};

const getGameStatusClass = (status) => {
  return status === 'playing' ? 'text-danger' : 'text-success';
};
</script>

<template>
  <BContainer>
    <h1 class="text-center m-5 color-beige">Salons de jeu</h1>

    <div v-if="!connected" class="alert alert-warning text-center">
      Connexion au serveur...
    </div>

    <div v-if="error" class="alert alert-danger text-center">
      {{ error }}
    </div>

    <BRow class="justify-content-center mb-4">
      <BCol >
        <BButton @click="showCreateModal = true" class="bg-color-blue-grey border m-2" :disabled="!connected">
          <i class="fa-solid fa-plus"></i> Créer une partie
        </BButton>
        <BButton @click="showJoinModal = true" class="bg-color-blue-grey border m-2" :disabled="!connected">
          <i class="fa-solid fa-right-to-bracket"></i> Rejoindre une partie
        </BButton>
      </BCol>
    </BRow>

    <BRow v-if="gameCode" class="justify-content-center" :style="{ paddingBottom: '100px' }">
      <BCol cols="12" md="6" :style="{ minWidth: '400px' }">
        <BCard class="bg-color-beige border shadow mb-4">
          <h2 class="text-center">Votre partie</h2>
          <p class="text-center fw-bold fs-4">Code: {{ gameCode }}</p>
          <p class="text-center">Joueurs: {{ players.length }}</p>
          <p class="text-center fw-bold" :class="getGameStatusClass(gameStatus)">
            {{ getGameStatusText(gameStatus) }}
          </p>

          <div class="mb-3">
            <h5>Participants:</h5>
            <ul class="list-unstyled">
              <li v-for="player in players" :key="player.id">
                {{ player.name }} {{ player.id === hostId.value ? '(Hôte)' : '' }}
              </li>
            </ul>
          </div>

          <div v-if="isHost && gameStatus === 'waiting'" class="mb-3">
            <h5 class="text-center mb-3">Choisir un thème</h5>
            <BRow>
              <BCol v-for="theme in themes" :key="theme.id" cols="12" class="mb-3">
                <BCard
                  class="border shadow p-3 cursor-pointer"
                  :class="selectedTheme?.id === theme.id ? 'border-primary bg-light' : 'bg-white'"
                  @click="selectTheme(theme)"
                  style="cursor: pointer;"
                >
                  <div class="d-flex align-items-center">
                    <i :class="[theme.icon, 'me-3', 'fa-2x', selectedTheme?.id === theme.id ? 'text-primary' : 'text-secondary']"></i>
                    <div>
                      <h6 class="m-0">{{ theme.name }}</h6>
                      <p class="m-0 small text-muted">{{ theme.description }}</p>
                    </div>
                    <i v-if="selectedTheme?.id === theme.id" class="fa-solid fa-check text-primary ms-auto fs-4"></i>
                  </div>
                </BCard>
              </BCol>
            </BRow>
          </div>

          <div v-if="!isHost && selectedTheme" class="mb-3 text-center">
            <h5>Thème sélectionné par l'hôte:</h5>
            <div class="d-flex align-items-center justify-content-center">
              <i :class="[selectedTheme.icon, 'me-2', 'fa-2x', 'text-primary']"></i>
              <span class="fs-5 fw-bold">{{ selectedTheme.name }}</span>
            </div>
          </div>

          <div class="text-center">
            <BButton
              v-if="isHost && gameStatus === 'waiting'"
              @click="handleStartGame"
              class="bg-color-blue-grey border"
              :disabled="players.length < 1 || !selectedTheme"
            >
              Démarrer la partie
            </BButton>
          </div>
        </BCard>
      </BCol>
    </BRow>

    <BModal v-model="showCreateModal" title="Créer une partie" hide-footer>
      <div class="mb-3">
        <label for="playerNameCreate" class="form-label">Votre nom:</label>
        <BFormInput
          id="playerNameCreate"
          v-model="playerName"
          placeholder="Entrez votre nom"
          @keyup.enter="handleCreateGame"
        />
      </div>
      <div class="text-center">
        <BButton @click="handleCreateGame" class="bg-color-blue-grey border m-2">
          Créer
        </BButton>
        <BButton variant="secondary" @click="showCreateModal = false" class="m-2">
          Annuler
        </BButton>
      </div>
    </BModal>

    <BModal v-model="showJoinModal" title="Rejoindre une partie" hide-footer>
      <div class="mb-3">
        <label for="playerNameJoin" class="form-label">Votre nom:</label>
        <BFormInput
          id="playerNameJoin"
          v-model="playerName"
          placeholder="Entrez votre nom"
        />
      </div>
      <div class="mb-3">
        <label for="gameCode" class="form-label">Code de la partie:</label>
        <BFormInput
          id="gameCode"
          v-model="codeToJoin"
          placeholder="Entrez le code"
          @keyup.enter="handleJoinGame"
        />
      </div>
      <div class="text-center">
        <BButton @click="handleJoinGame" class="bg-color-blue-grey border m-2">
          Rejoindre
        </BButton>
        <BButton variant="secondary" @click="showJoinModal = false" class="m-2">
          Annuler
        </BButton>
      </div>
    </BModal>

    <BModal v-model="showModal" title="Participants de la partie">
      <ul>
        <li v-for="participant in selectedParticipants" :key="participant">{{ participant }}</li>
      </ul>
      <template #modal-footer>
        <BButton variant="secondary" @click="showModal = false">Fermer</BButton>
      </template>
    </BModal>
  </BContainer>
</template>

<style scoped></style>
