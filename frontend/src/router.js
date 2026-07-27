import { createRouter, createWebHistory } from "vue-router";

import HomePage from "./pages/Home/HomePage.vue";
import RoomPage from "./pages/Rooms/RoomPage.vue";
import RulePage from "./pages/Rules/RulePage.vue";
import RoundPage from "./pages/Game/RoundPage.vue";
import CharacterPage from "./pages/Characters/CharacterPage.vue";

const routes = [
    {
        path: "/",
        name: "Home",
        component: HomePage,
    },
    {
        path: "/rooms",
        name: "Room",
        component: RoomPage,
    },
    {
        path: "/rules",
        name: "Rule",
        component: RulePage,
    },
    {
        path: '/game/:gameId',
        name: 'RoundPage',
        component: RoundPage
    },
    {
        path: '/characters',
        name: 'Character',
        component: CharacterPage,
    }
];

const router = createRouter({
    history: createWebHistory(),
    routes,
});

export default router;
