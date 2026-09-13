<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Game extends Model
{
    protected $fillable = [
        'room_id',
        'status',
    ];

    protected $casts = [
        'status' => \App\Enums\GameStatus::class,
    ];

    public function room()
    {
        return $this->belongsTo(Room::class);
    }

    public function binomes()
    {
        return $this->hasMany(Binome::class);
    }

    public function rounds()
    {
        return $this->hasMany(Round::class);
    }

    public function gameStats()
    {
        return $this->hasMany(GameStat::class);
    }

    /**
     * La partie compte-t-elle un orphelin (binome d'un seul joueur, formé quand
     * les joueurs sont en nombre impair) ?
     */
    public function hasOrphan(): bool
    {
        return $this->relationLoaded('binomes')
            ? $this->binomes->contains(fn ($binome) => $binome->is_orphan)
            : $this->binomes()->where('is_orphan', true)->exists();
    }

    public function currentRound()
    {
        return $this->hasOne(Round::class)
            ->where('is_finished', false)
            ->latest();
    }
}
