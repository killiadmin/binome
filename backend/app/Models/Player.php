<?php

namespace App\Models;
use App\Models\Action;
use App\Models\Binome;
use App\Models\Character;
use App\Models\Game;
use Illuminate\Database\Eloquent\Model;
use App\Models\Room;
use App\Models\Round;

class Player extends Model
{
    protected $fillable = ['pseudo'];

    public function rooms()
    {
        return $this->belongsToMany(Room::class)->withPivot('is_ready');
    }

    public function binomes()
    {
        return $this->belongsToMany(Binome::class)
            ->withPivot('character_id', 'score');
    }

    public function character()
    {
        return $this->hasOneThrough(Character::class, Binome::class);
    }

    public function actions()
    {
        return $this->hasMany(Action::class);
    }

    public function rounds()
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
