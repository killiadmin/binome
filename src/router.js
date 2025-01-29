import { createRouter, createWebHistory } from "vue-router";

import HomePage from "./pages/HomePage.vue";
import RoomPage from "./pages/RoomPage.vue";
import RulePage from "./pages/RulePage.vue";

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
];

const router = createRouter({
    history: createWebHistory(),
    routes,
});

export default router;
