<script>
import { BButton, BCard, BContainer, BRow, BCol, BModal } from "bootstrap-vue-next";

export default {
  name: "RoomPage",
  components: {
    BButton,
    BCard,
    BContainer,
    BRow,
    BCol,
    BModal
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
        {id: 3, name: 'Salon 3', players: 6, maxPlayers: 10, inProgress: false, participants: ['Ivy', 'Jack']},
      ],
      showModal: false,
      selectedParticipants: [],
      currentRoom: null
    };
  },
  methods: {
    viewParticipants(participants) {
      this.selectedParticipants = participants;
      this.showModal = true;
    },
    joinRoom(roomId) {
      this.$router.push({name: "RoomDetails", params: {id: roomId}});
    },
  },
};
</script>

<template>
  <BContainer>
    <h1 class="text-center m-5 color-beige">{{ currentRoom ? currentRoom.name : "Rejoindre un salon" }}</h1>

    <BRow class="justify-content-center d-flex m-4">
      <BCol v-for="room in rooms" :key="room.id" cols="12" md="4" class="mb-4 d-flex" style="width:250px;">
        <BCard class="w-100 bg-color-beige border shadow">
          <h2>{{ room.name }}</h2>
          <p>Joueurs: </p>
          <p>{{ room.players }} / {{ room.maxPlayers }}</p>
          <p v-if="room.inProgress" class="text-danger fw-bold">En cours</p>
          <p v-else class="text-success fw-bold">Disponible</p>
          <BButton v-if="!room.inProgress" @click="joinRoom(room.id)" :disabled="room.players >= room.maxPlayers"
                   class="bg-color-blue-grey border">
            Rejoindre
          </BButton>
          <BButton v-if="room.inProgress" @click="viewParticipants(room.participants)" class="bg-color-blue-grey w-50">
            <i class="fa-solid fa-users"></i>
          </BButton>
        </BCard>
      </BCol>
    </BRow>

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
