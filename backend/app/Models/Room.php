<?php

namespace App\Models;
use App\Models\Game;
use Illuminate\Database\Eloquent\Model;
use App\Models\Player;

class Room extends Model
{
    public function players()
    {
        return $this->belongsToMany(Player::class)->withPivot('is_ready');
    }

    public function games()
    {
        return $this->hasMany(Game::class);
    }

    public function currentGame()
    {
        return $this->hasOne(Game::class)->where('status', 'in_progress');
    }
}
