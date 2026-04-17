<?php

namespace App\Http\Requests;

use App\Models\Player;
use Illuminate\Foundation\Http\FormRequest;

class PlayQuestionRequest extends FormRequest
{
    public function authorize(): bool
    {
        $round  = $this->route('round');
        $player = Player::find($this->input('player_id'));

        if (!$player || !$round) return false;

        return $round->game->binomes
            ->flatMap(fn($b) => $b->players)
            ->contains('id', $player->id);
    }

    public function rules(): array
    {
        return [
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'target_player_id' => ['required', 'integer', 'exists:players,id'],
            'question'  => ['required', 'string', 'min:5', 'max:500'],
        ];
    }

    public function messages(): array
    {
        return [
            'player_id.required' => 'Le joueur est requis.',
            'player_id.exists'   => 'Ce joueur n\'existe pas.',
            'question.required'  => 'La question est requise.',
            'question.min'       => 'La question doit faire au moins 5 caractères.',
            'question.max'       => 'La question ne peut pas dépasser 500 caractères.',
        ];
    }
}
