import { api } from './api'

// Profil joueur : connexion par pseudo (mot de passe facultatif), page profil.
// Le jeton de profil part dans l'en-tête X-Profile-Token (intercepteur d'api.js).
export const profileService = {
    /**
     * Connexion (crée le profil si le pseudo est inconnu)
     * POST /api/profile/login — body: { pseudo, password? }
     * → { token, player, current, admin_token? }
     */
    login(pseudo, password) {
        return api.post('/profile/login', { pseudo, password: password || undefined })
    },

    /** GET /api/profile → { player, current, totals, games } */
    show() {
        return api.get('/profile')
    },

    /** PATCH /api/profile — body: { pseudo } → { token, player } */
    rename(pseudo) {
        return api.patch('/profile', { pseudo })
    },

    /** PUT /api/profile/password — body: { password, current_password? } → { token, player } */
    setPassword(password, currentPassword) {
        return api.put('/profile/password', { password, current_password: currentPassword || undefined })
    },

    /** DELETE /api/profile/password — body: { current_password } → { token, player } */
    removePassword(currentPassword) {
        return api.delete('/profile/password', { data: { current_password: currentPassword } })
    },

    /** POST /api/profile/password-prompt — la proposition de mot de passe a été vue */
    dismissPasswordPrompt() {
        return api.post('/profile/password-prompt')
    },
}
