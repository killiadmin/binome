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
            'name'              => ['required', 'string', 'min:2', 'max:50'],
            'image'             => ['nullable', 'image', 'mimes:jpeg,jpg,png,gif,webp', 'max:4096'],
            'forbidden_words'   => ['required', 'array', 'size:3'],
            'forbidden_words.*' => ['required', 'string', 'min:1', 'max:50'],
        ];
    }

    public function messages(): array
    {
        return [
            'name.required'                => 'Le nom du personnage est requis.',
            'name.min'                     => 'Le nom du personnage doit faire au moins 2 caractères.',
            'name.max'                     => 'Le nom du personnage ne peut pas dépasser 50 caractères.',
            'image.image'                  => 'Le fichier doit être une image.',
            'image.mimes'                  => 'Formats acceptés : jpeg, png, gif, webp.',
            'image.max'                    => "L'image ne peut pas dépasser 4 Mo.",
            'forbidden_words.required'     => 'Les 3 mots interdits sont requis.',
            'forbidden_words.size'         => 'Il faut exactement 3 mots interdits.',
            'forbidden_words.*.required'   => 'Un mot interdit ne peut pas être vide.',
        ];
    }
}
