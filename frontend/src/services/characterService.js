import { api } from './api'

// Sur une requête FormData, on retire le Content-Type JSON par défaut de
// l'instance axios pour laisser le navigateur poser le bon boundary multipart.
const multipart = { headers: { 'Content-Type': undefined } }

export const characterService = {
    /**
     * Liste des univers existants
     * GET /api/universes
     */
    listUniverses() {
        return api.get('/universes')
    },

    /**
     * Créer un univers
     * POST /api/universes
     * body: { name }
     */
    createUniverse(name) {
        return api.post('/universes', { name })
    },

    /**
     * Créer un personnage rattaché à un univers
     * POST /api/universes/{universe}/characters
     * body (multipart): name, image? (File), forbidden_words[], level_affectation
     */
    createCharacter(universeId, { name, imageFile = null, forbiddenWords, levelAffectation }) {
        const formData = new FormData()
        formData.append('name', name)
        if (imageFile) {
            formData.append('image', imageFile)
        }
        forbiddenWords.forEach(word => formData.append('forbidden_words[]', word))
        formData.append('level_affectation', levelAffectation)

        return api.post(`/universes/${universeId}/characters`, formData, multipart)
    },

    /**
     * Liste tous les personnages, toutes univers confondus
     * GET /api/characters
     */
    listCharacters() {
        return api.get('/characters')
    },

    /**
     * Modifier un personnage (l'image n'est remplacée que si un nouveau fichier est fourni)
     * PUT /api/characters/{character} (envoyé en POST + _method=PUT pour supporter l'upload de fichier)
     * body (multipart): name, image? (File), forbidden_words[], level_affectation
     */
    updateCharacter(characterId, { name, imageFile = null, forbiddenWords, levelAffectation }) {
        const formData = new FormData()
        formData.append('_method', 'PUT')
        formData.append('name', name)
        if (imageFile) {
            formData.append('image', imageFile)
        }
        forbiddenWords.forEach(word => formData.append('forbidden_words[]', word))
        formData.append('level_affectation', levelAffectation)

        return api.post(`/characters/${characterId}`, formData, multipart)
    },

    /**
     * Supprimer un personnage
     * DELETE /api/characters/{character}
     */
    deleteCharacter(characterId) {
        return api.delete(`/characters/${characterId}`)
    },
}
