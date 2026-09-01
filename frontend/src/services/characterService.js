import { api } from './api'

// Sur une requête FormData, on retire le Content-Type JSON par défaut de
// l'instance axios pour laisser le navigateur poser le bon boundary multipart.
const multipart = { headers: { 'Content-Type': undefined } }

export const characterService = {
    /**
     * Liste des cosmos existants (sur-catégorie regroupant plusieurs univers)
     * GET /api/cosmos
     */
    listCosmos() {
        return api.get('/cosmos')
    },

    /**
     * Créer un cosmos
     * POST /api/cosmos
     * body: { name }
     */
    createCosmos(name) {
        return api.post('/cosmos', { name })
    },

    /**
     * Liste des univers existants
     * GET /api/universes
     */
    listUniverses() {
        return api.get('/universes')
    },

    /**
     * Créer un univers, éventuellement rattaché à un cosmos
     * POST /api/universes
     * body: { name, cosmos_id? }
     */
    createUniverse(name, cosmosId = null) {
        return api.post('/universes', { name, cosmos_id: cosmosId })
    },

    /**
     * Mettre à jour le cosmos d'un univers existant
     * PATCH /api/universes/{universe}
     * body: { cosmos_id }
     */
    updateUniverseCosmos(universeId, cosmosId) {
        return api.patch(`/universes/${universeId}`, { cosmos_id: cosmosId })
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
     * body (multipart): name, universe_id?, image? (File), forbidden_words[], level_affectation
     */
    updateCharacter(characterId, { name, universeId = null, imageFile = null, forbiddenWords, levelAffectation }) {
        const formData = new FormData()
        formData.append('_method', 'PUT')
        formData.append('name', name)
        if (universeId) {
            formData.append('universe_id', universeId)
        }
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

    /**
     * Binômes proposés en attente de validation manuelle
     * GET /api/characters/pending
     */
    listPendingBinomes() {
        return api.get('/characters/pending')
    },

    /**
     * Valider un binôme proposé (chaque personnage doit déjà avoir une image)
     * POST /api/characters/validation/accept
     */
    acceptBinome(characterIds) {
        return api.post('/characters/validation/accept', { character_ids: characterIds })
    },

    /**
     * Refuser un binôme proposé (masqué définitivement, jamais reproposé)
     * POST /api/characters/validation/reject
     */
    rejectBinome(characterIds) {
        return api.post('/characters/validation/reject', { character_ids: characterIds })
    },

    /**
     * Mettre à jour uniquement les 3 mots interdits d'un personnage
     * PATCH /api/characters/{character}/forbidden-words
     */
    updateForbiddenWords(characterId, forbiddenWords) {
        return api.patch(`/characters/${characterId}/forbidden-words`, { forbidden_words: forbiddenWords })
    },

    /**
     * Importer / remplacer l'image d'un personnage
     * POST /api/characters/{character}/image
     */
    uploadCharacterImage(characterId, imageFile) {
        const formData = new FormData()
        formData.append('image', imageFile)

        return api.post(`/characters/${characterId}/image`, formData, multipart)
    },
}
