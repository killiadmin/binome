<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class StoreCosmosRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    public function rules(): array
    {
        return [
            'name' => ['required', 'string', 'min:2', 'max:50', 'unique:cosmos,name'],
        ];
    }

    public function messages(): array
    {
        return [
            'name.required' => 'Le nom du cosmos est requis.',
            'name.min' => 'Le nom du cosmos doit faire au moins 2 caractères.',
            'name.max' => 'Le nom du cosmos ne peut pas dépasser 50 caractères.',
            'name.unique' => 'Un cosmos avec ce nom existe déjà.',
        ];
    }
}
