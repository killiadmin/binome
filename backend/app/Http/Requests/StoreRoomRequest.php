<?php


namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class StoreRoomRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    public function rules(): array
    {
        return [
            'pseudo' => ['required', 'string', 'min:2', 'max:20'],
            'is_private' => ['boolean'],
            'max_players' => ['integer', 'min:4', 'max:12'],
        ];
    }

    public function messages(): array
    {
        return [
            'pseudo.required' => 'Le pseudo est requis.',
            'pseudo.min' => 'Le pseudo doit faire au moins 2 caractères.',
            'pseudo.max' => 'Le pseudo ne peut pas dépasser 20 caractères.',
            'max_players.min' => 'Il faut au minimum 4 joueurs.',
            'max_players.max' => 'Le salon ne peut pas dépasser 12 joueurs.',
        ];
    }
}
