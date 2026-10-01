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

    /**
     * Le joueur a-t-il un personnage dans cette partie (éliminé ou non) ?
     */
    public function hasPlayer(int $playerId): bool
    {
        return $this->binomes()
            ->whereHas('players', fn ($q) => $q->where('player_id', $playerId))
            ->exists();
    }

    /**
     * Le joueur peut-il suivre la partie : joueur de la partie, ou membre du
     * salon arrivé après le lancement (spectateur). Même règle que le
     * PresenceChannel game.{id} dans routes/channels.php.
     */
    public function canBeWatchedBy(int $playerId): bool
    {
        return $this->hasPlayer($playerId)
            || $this->room?->players()->where('player_id', $playerId)->exists();
    }

    public function currentRound()
    {
        return $this->hasOne(Round::class)
            ->where('is_finished', false)
            ->latest();
    }
}
