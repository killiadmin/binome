<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class JoinRoomRequest extends FormRequest
{
    public function authorize(): bool
    {
        return true;
    }

    public function rules(): array
    {
        return [
            'pseudo' => ['required', 'string', 'min:2', 'max:20'],
            'code'   => ['required', 'string', 'size:6', 'exists:rooms,code'],
        ];
    }

    public function messages(): array
    {
        return [
            'pseudo.required' => 'Le pseudo est requis.',
            'pseudo.min'      => 'Le pseudo doit faire au moins 2 caractères.',
            'code.required'   => 'Le code du salon est requis.',
            'code.size'       => 'Le code doit faire exactement 6 caractères.',
            'code.exists'     => 'Ce salon n\'existe pas.',
        ];
    }
}
