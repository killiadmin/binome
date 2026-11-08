<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class StoreCharacterRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    public function rules(): array
    {
        return [
            'name' => ['required', 'string', 'min:2', 'max:50'],
            // Sur une modification, l'univers du personnage peut être réassigné.
            // À la création il vient de la route (/universes/{universe}/characters) et n'est pas attendu ici.
            'universe_id' => ['nullable', 'integer', 'exists:universes,id'],
            'image' => ['nullable', 'image', 'mimes:jpeg,jpg,png,gif,webp', 'max:4096'],
            'forbidden_words' => ['required', 'array', 'size:3'],
            'forbidden_words.*' => ['required', 'string', 'min:1', 'max:50'],
            // Aucune limite du nombre de personnages par niveau : la correction manuelle
            // doit toujours passer (ex. échanger deux niveaux). Le jeu tire 2 personnages
            // au hasard si un niveau en compte plus, et la liste signale l'anomalie.
            'level_affectation' => ['required', 'integer', 'min:1'],
            // Modification seulement : force l'état de relecture (voir CharacterController::update).
            'status' => ['nullable', 'in:validated,pending,rejected'],
        ];
    }

    public function messages(): array
    {
        return [
            'name.required' => 'Le nom du personnage est requis.',
            'name.min' => 'Le nom du personnage doit faire au moins 2 caractères.',
            'name.max' => 'Le nom du personnage ne peut pas dépasser 50 caractères.',
            'universe_id.exists' => "L'univers sélectionné n'existe pas.",
            'image.image' => 'Le fichier doit être une image.',
            'image.mimes' => 'Formats acceptés : jpeg, png, gif, webp.',
            'image.max' => "L'image ne peut pas dépasser 4 Mo.",
            'forbidden_words.required' => 'Les 3 mots interdits sont requis.',
            'forbidden_words.size' => 'Il faut exactement 3 mots interdits.',
            'forbidden_words.*.required' => 'Un mot interdit ne peut pas être vide.',
            'level_affectation.required' => 'Le niveau d\'affectation est requis.',
            'level_affectation.integer' => 'Le niveau d\'affectation doit être un nombre entier.',
            'level_affectation.min' => 'Le niveau d\'affectation doit être supérieur ou égal à 1.',
            'status.in' => 'Statut inconnu.',
        ];
    }
}
