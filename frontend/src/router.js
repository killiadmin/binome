import { createRouter, createWebHistory } from "vue-router";

import HomePage from "./pages/Home/HomePage.vue";
import RoomPage from "./pages/Rooms/RoomPage.vue";
import RulePage from "./pages/Rules/RulePage.vue";
import RoundPage from "./pages/Game/RoundPage.vue";

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
    }
];

const router = createRouter({
    history: createWebHistory(),
    routes,
});

export default router;
