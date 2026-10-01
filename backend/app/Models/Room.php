<?php

namespace App\Models;

use App\Enums\GameMode;
use Illuminate\Database\Eloquent\Model;

class Room extends Model
{
    protected $fillable = [
        'code',
        'is_private',
        'is_locked',
        'max_players',
        'game_mode',
        'cosmos_id',
        'created_by',
    ];

    protected $casts = [
        'is_private' => 'boolean',
        'is_locked' => 'boolean',
        'game_mode' => GameMode::class,
    ];

    public function creator(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Player::class, 'created_by');
    }

    public function cosmos(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Cosmos::class);
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

    /**
     * Joueurs du salon tels que le lobby les affiche : statut prêt + URL de
     * la photo (jamais l'image elle-même, voir PlayerAvatar).
     */
    public function playersPayload(): \Illuminate\Support\Collection
    {
        $players = $this->players()->get();
        $avatars = PlayerAvatar::urlsFor($players);

        return $players->map(fn ($p) => [
            'id' => $p->id,
            'pseudo' => $p->pseudo,
            'is_ready' => $p->pivot->is_ready,
            'avatar_url' => $avatars[$p->id] ?? null,
        ])->values();
    }
}
