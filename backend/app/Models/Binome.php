<?php

namespace App\Models;
use App\Models\Game;
use Illuminate\Database\Eloquent\Model;
use App\Models\Player;
use App\Models\Universe;

class Binome extends Model
{
    protected $fillable = [
        'game_id',
        'universe_id',
        'discovered_by_player_id',
        'is_discovered', // si présent en DB
        'is_orphan',
    ];

    protected $casts = [
        'is_discovered' => 'boolean',
        'is_orphan' => 'boolean',
    ];

    public function game()
    {
        return $this->belongsTo(Game::class);
    }

    public function universe()
    {
        return $this->belongsTo(Universe::class);
    }

    public function players()
    {
        return $this->belongsToMany(Player::class)
            ->withPivot('character_id', 'score', 'is_eliminated', 'is_excluded', 'eliminated_round');
    }

    public function discoveredBy()
    {
        return $this->belongsTo(Player::class, 'discovered_by_player_id');
    }
}
