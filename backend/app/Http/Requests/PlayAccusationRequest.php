<?php

namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;
use App\Models\Player;

class PlayAccusationRequest extends FormRequest
{
    public function authorize(): bool
    {
        $round  = $this->route('round');
        $player = Player::find($this->input('player_id'));

        if (!$player || !$round) return false;

        // Le joueur doit appartenir à la partie
        $playersInGame = $round->game->binomes
            ->flatMap(fn($b) => $b->players)
            ->pluck('id');

        // Le joueur et la cible doivent tous les deux être dans la partie
        return $playersInGame->contains($player->id)
            && $playersInGame->contains($this->input('target_player_id'));
    }

    public function rules(): array
    {
        return [
            'player_id'        => ['required', 'integer', 'exists:players,id'],
            'target_player_id' => [
                'required',
                'integer',
                'exists:players,id',
                'different:player_id',
            ],
            'character_name'   => ['required', 'string', 'max:100'],
        ];
    }

    public function messages(): array
    {
        return [
            'player_id.required'        => 'Le joueur est requis.',
            'player_id.exists'          => 'Ce joueur n\'existe pas.',
            'target_player_id.required' => 'La cible est requise.',
            'target_player_id.exists'   => 'Ce joueur cible n\'existe pas.',
            'target_player_id.different'=> 'Tu ne peux pas t\'accuser toi-même.',
            'character_name.required'   => 'Le nom du personnage est requis.',
            'character_name.max'        => 'Le nom du personnage est trop long.',
        ];
    }
}
