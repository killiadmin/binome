<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class StoreChatMessageRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    public function rules(): array
    {
        return [
            'body' => ['required', 'string', 'max:500'],
            // Présent dès que l'auteur a créé ou rejoint un salon.
            'player_id' => ['nullable', 'integer', 'exists:players,id'],
            // Repli pour les visiteurs sans salon : identifiant tiré au sort
            // et conservé dans le localStorage du navigateur.
            'anon_id' => ['required_without:player_id', 'nullable', 'string', 'regex:/^[A-Za-z0-9]{1,12}$/'],
        ];
    }

    public function messages(): array
    {
        return [
            'body.required' => 'Le message ne peut pas être vide.',
            'body.max' => 'Le message ne peut pas dépasser 500 caractères.',
            'anon_id.required_without' => 'Identifiant anonyme manquant.',
        ];
    }
}
