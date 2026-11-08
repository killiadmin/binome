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
        'reveal_to_eliminated',
        'best_question_vote',
        'created_by',
    ];

    protected $casts = [
        'is_private' => 'boolean',
        'is_locked' => 'boolean',
        'reveal_to_eliminated' => 'boolean',
        'best_question_vote' => 'boolean',
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
     * Retire un joueur du salon : salon supprimé s'il est vide, sinon l'hôte
     * passe au joueur suivant, puis broadcast PlayerLeft. Partagé par le
     * départ volontaire (RoomController::leave) et le retrait par un
     * administrateur (AdminPlayerController).
     *
     * À appeler AVANT toute suppression du joueur : rooms.created_by est en
     * cascade, supprimer l'hôte d'abord emporterait le salon et ses joueurs.
     */
    public function removePlayer(int $playerId): void
    {
        $this->players()->detach($playerId);

        if ($this->players()->count() === 0) {
            $this->delete();

            return;
        }

        $newHost = null;
        if ((int) $this->created_by === $playerId) {
            $newHost = $this->players()->first();
            $this->update(['created_by' => $newHost->id]);
        }

        broadcast(new \App\Events\PlayerLeft($this->load('players'), $newHost));
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
