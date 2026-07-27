<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class StoreUniverseRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    public function rules(): array
    {
        return [
            'name' => ['required', 'string', 'min:2', 'max:50', 'unique:universes,name'],
        ];
    }

    public function messages(): array
    {
        return [
            'name.required' => "Le nom de l'univers est requis.",
            'name.min'      => "Le nom de l'univers doit faire au moins 2 caractères.",
            'name.max'      => "Le nom de l'univers ne peut pas dépasser 50 caractères.",
            'name.unique'   => 'Un univers avec ce nom existe déjà.',
        ];
    }
}
