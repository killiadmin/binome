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
        // Historique public : aucune authentification (cf. HistoryController).
        path: "/historique",
        name: "History",
        component: () => import("./pages/History/HistoryPage.vue"),
    },
    {
        path: '/game/:gameId',
        name: 'RoundPage',
        component: RoundPage
    },
    // Anciennes adresses de la gestion des personnages → espace admin.
    { path: '/characters', redirect: '/admin/binomes/new' },
    { path: '/characters/list', redirect: '/admin/binomes' },
    {
        // Espace d'administration (cadenas de la navbar). meta est héritée
        // par les enfants : la garde ci-dessous protège toutes les sections.
        path: '/admin',
        component: () => import('./pages/Admin/AdminLayout.vue'),
        meta: { requiresCharactersAccess: true },
        children: [
            { path: '', redirect: { name: 'AdminGames' } },
            {
                path: 'games',
                name: 'AdminGames',
                component: () => import('./pages/Admin/AdminGamesPage.vue'),
            },
            {
                path: 'binomes',
                name: 'CharacterList',
                component: CharacterListPage,
            },
            {
                path: 'binomes/new',
                name: 'Character',
                component: CharacterPage,
            },
            {
                path: 'prompts',
                name: 'AdminPrompts',
                component: () => import('./pages/Admin/AdminPromptsPage.vue'),
            },
            {
                path: 'players',
                name: 'AdminPlayers',
                component: () => import('./pages/Admin/AdminPlayersPage.vue'),
            },
            {
                path: 'reactions',
                name: 'AdminReactions',
                component: () => import('./pages/Admin/AdminReactionsPage.vue'),
            },
        ],
    },
];

const router = createRouter({
    history: createWebHistory(import.meta.env.BASE_URL),
    routes,
});

// Accès direct à une page d'administration sans token -> retour à l'accueil.
router.beforeEach((to) => {
    if (to.meta.requiresCharactersAccess && !getCharactersAccessToken()) {
        return { name: 'Home' };
    }
});

export default router;
