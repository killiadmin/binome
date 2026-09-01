<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class UpdateUniverseRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    public function rules(): array
    {
        return [
            'cosmos_id' => ['nullable', 'integer', 'exists:cosmos,id'],
        ];
    }

    public function messages(): array
    {
        return [
            'cosmos_id.exists' => "Le cosmos sélectionné n'existe pas.",
        ];
    }
}
