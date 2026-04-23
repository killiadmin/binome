<?php

namespace App\Models;

use App\Enums\ActionType;
use Illuminate\Database\Eloquent\Model;

class Action extends Model
{
    protected $fillable = [
        'round_id',
        'player_id',
        'target_player_id',
        'type',
        'content',
        'character_name',
        'is_valid',
        'accusation_correct',
        'accusation_confirmed',
        'answer',
    ];

    protected $casts = [
        'type'                 => ActionType::class,
        'is_valid'             => 'boolean',
        'accusation_correct'   => 'boolean',
        'accusation_confirmed' => 'boolean',
    ];

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
