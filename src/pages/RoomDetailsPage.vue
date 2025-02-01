<script>
import { BButton, BCard } from "bootstrap-vue-next";

export default {
  name: 'RoomPage',
  components: {
    BButton,
    BCard,
  },
  data() {
    return {
      rooms: [
        {
          id: 1,
          name: 'Salon 1',
          players: 8,
          maxPlayers: 10,
          inProgress: false,
          participants: ['Alice', 'Bob', 'Charlie']
        },
        {
          id: 2,
          name: 'Salon 2',
          players: 10,
          maxPlayers: 10,
          inProgress: true,
          participants: ['David', 'Emma', 'Frank', 'Grace', 'Hugo']
        },
        {
          id: 3,
          name: 'Salon 3',
          players: 6,
          maxPlayers: 10,
          inProgress: false,
          participants: ['Ivy', 'Jack']
        },
      ],
      showModal: false,
      selectedParticipants: [],
      currentRoom: null,
    };
  },
  created() {
    const roomId = this.$route.params.id;
    this.currentRoom = this.rooms.find(room => room.id === parseInt(roomId));
  },
  methods: {
    leaveRoom() {
      this.$router.push({ path: "/rooms" });
    }
  }
};
</script>

<template>
  <BCard class="p-4 text-center m-5 bg-color-beige border shadow" style="max-width: 600px;">
    <h2>Préparez-vous</h2>
    <p>Joueurs actuellement dans la salle :</p>
    <ul class="list-unstyled border" v-if="currentRoom">
      <li v-for="participant in currentRoom.participants" :key="participant">{{ participant }}</li>
    </ul>
    <div class="d-flex flex-column justify-content-between p-3 gap-3">
      <BButton variant="success">Démarrer</BButton>
      <BButton variant="danger" @click="leaveRoom">Quitter</BButton>
    </div>
  </BCard>
</template>

<style scoped></style>
