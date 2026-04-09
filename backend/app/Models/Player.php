<?php

namespace App\Models;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Foundation\Auth\User as Authenticatable;

class Player extends Authenticatable
{
    protected $fillable = ['pseudo'];

    public function rooms(): BelongsToMany
    {
        return $this->belongsToMany(Room::class, 'room_player')
        ->withPivot('is_ready')
            ->withTimestamps();
    }

    public function createdRooms(): HasMany
    {
        return $this->hasMany(Room::class, 'created_by');
    }

    public function binomes(): BelongsToMany
    {
        return $this->belongsToMany(Binome::class)
            ->withPivot('character_id', 'score');
    }

    public function character(): \Illuminate\Database\Eloquent\Relations\HasOneThrough
    {
        return $this->hasOneThrough(Character::class, Binome::class);
    }

    public function actions(): HasMany
    {
        return $this->hasMany(Action::class);
    }

    public function rounds(): HasMany
    {
        return $this->hasMany(Round::class, 'current_player_id');
    }

    public function discoveredBinomes()
    {
        return $this->hasMany(Binome::class, 'discovered_by_player_id');
    }

    public function getCharacterInGame(Game $game): ?Character
    {
        $binome = $this->binomes()
            ->where('game_id', $game->id)
            ->first();

        return $binome
            ? Character::find($binome->pivot->character_id)
            : null;
    }

    public function questionContainsForbiddenWord(string $question, Game $game): bool
    {
        $character = $this->getCharacterInGame($game);
        return $character?->checkForbiddenWords($question) ?? false;
    }
}
