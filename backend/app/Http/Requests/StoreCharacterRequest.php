<?php

namespace App\Http\Requests;

use App\Models\Character;
use Illuminate\Contracts\Validation\Validator;
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
            'level_affectation' => ['required', 'integer', 'min:1'],
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
        ];
    }

    /**
     * Un niveau d'affectation ne peut relier que 2 personnages au sein d'un même univers
     * (c'est ce qui forme le binome en jeu) : on bloque toute 3e assignation au même niveau.
     */
    public function withValidator(Validator $validator): void
    {
        $validator->after(function (Validator $validator) {
            $universeId = $this->route('universe')?->id
                ?? $this->input('universe_id')
                ?? $this->route('character')?->universe_id;
            $level = $this->input('level_affectation');

            if (! $universeId || $level === null) {
                return;
            }

            $query = Character::where('universe_id', $universeId)
                ->where('level_affectation', $level);

            if ($character = $this->route('character')) {
                $query->where('id', '!=', $character->id);
            }

            if ($query->count() >= 2) {
                $validator->errors()->add(
                    'level_affectation',
                    "Il y a déjà 2 personnages avec ce niveau d'affectation dans cet univers."
                );
            }
        });
    }
}
