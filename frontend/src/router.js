import { createRouter, createWebHistory } from "vue-router";
import { getCharactersAccessToken } from "./services/charactersAccess";

import HomePage from "./pages/Home/HomePage.vue";
import RoomPage from "./pages/Rooms/RoomPage.vue";
import RulePage from "./pages/Rules/RulePage.vue";
import RoundPage from "./pages/Game/RoundPage.vue";
import CharacterPage from "./pages/Characters/CharacterPage.vue";
import CharacterListPage from "./pages/Characters/CharacterListPage.vue";

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
        meta: { requiresCharactersAccess: true },
    },
    {
        path: '/characters/list',
        name: 'CharacterList',
        component: CharacterListPage,
        meta: { requiresCharactersAccess: true },
    }
];

const router = createRouter({
    history: createWebHistory(),
    routes,
});

// Accès direct à une page « personnages » sans token -> retour à l'accueil.
router.beforeEach((to) => {
    if (to.meta.requiresCharactersAccess && !getCharactersAccessToken()) {
        return { name: 'Home' };
    }
});

export default router;
