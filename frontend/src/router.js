import { createRouter, createWebHistory } from "vue-router";
import { getCharactersAccessToken } from "./services/charactersAccess";
import { getProfileToken, isAdminProfile, isDelegateAdmin } from "./services/profile";

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
        // Boîte à idées : lecture libre, poster / voter une fois connecté.
        path: "/idees",
        name: "Ideas",
        component: () => import("./pages/Ideas/IdeasPage.vue"),
    },
    {
        path: "/connexion",
        name: "Login",
        component: () => import("./pages/Profile/LoginPage.vue"),
    },
    {
        // Visible seulement une fois connecté (garde ci-dessous).
        path: "/profil",
        name: "Profile",
        component: () => import("./pages/Profile/ProfilePage.vue"),
        meta: { requiresProfile: true },
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

// Administration : réservée au profil propriétaire connecté (et à son token
// d'accès) ou à un administrateur désigné (son jeton de profil suffit) ; accès
// direct par URL sinon -> retour à l'accueil. Le serveur vérifie de toute façon
// chaque requête d'administration.
router.beforeEach((to) => {
    if (to.meta.requiresCharactersAccess && !((getCharactersAccessToken() && isAdminProfile.value) || isDelegateAdmin.value)) {
        return { name: 'Home' };
    }
    if (to.meta.requiresProfile && !getProfileToken()) {
        return { name: 'Login' };
    }
});

export default router;
