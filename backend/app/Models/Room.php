<?php

namespace App\Models;
use Illuminate\Database\Eloquent\Model;

class Room extends Model
{
    protected $fillable = [
        'code',
        'is_private',
        'is_locked',
        'max_players',
        'created_by'
    ];

    protected $casts = [
        'is_private' => 'boolean',
        'is_locked' => 'boolean',
    ];

    public function creator(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Player::class, 'created_by');
    }

    public function players(): \Illuminate\Database\Eloquent\Relations\BelongsToMany
    {
        return $this->belongsToMany(Player::class, 'room_player') // 👈 Spécifiez le nom de la table
        ->withPivot('is_ready')
            ->withTimestamps();
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
