<?php

namespace App\Models;
use Illuminate\Database\Eloquent\Model;
use App\Models\Player;
use App\Models\Round;

class Action extends Model
{
    public function round(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Round::class);
    }

    public function player(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Player::class);
    }

    public function targetPlayer(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Player::class, 'target_player_id');
    }
}
