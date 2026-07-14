<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class GameStat extends Model
{
    protected $fillable = [
        'game_id',
        'player_pseudo',
        'character_name',
        'eliminations',
        'rounds_survived',
        'survived_full_game',
        'score',
        'is_winner',
        'is_eliminated',
    ];

    protected $casts = [
        'survived_full_game' => 'boolean',
        'is_winner'          => 'boolean',
        'is_eliminated'      => 'boolean',
    ];

    public function game()
    {
        return $this->belongsTo(Game::class);
    }
}
