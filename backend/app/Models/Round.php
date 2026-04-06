<?php

namespace App\Models;
use App\Models\Action;
use App\Models\Game;
use Illuminate\Database\Eloquent\Model;
use App\Models\Player;

class Round extends Model
{
    public function game()
    {
        return $this->belongsTo(Game::class);
    }

    public function currentPlayer()
    {
        return $this->belongsTo(Player::class, 'current_player_id');
    }

    public function actions()
    {
        return $this->hasMany(Action::class);
    }
}
