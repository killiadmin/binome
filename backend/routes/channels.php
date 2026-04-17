<?php

use App\Models\Game;
use App\Models\Player;
use App\Models\Room;

/*
 * PresenceChannel game.{gameId}
 * Vérifie que le joueur appartient bien à cette partie
 * Retourne les infos du joueur pour la presence list côté front
 */
Broadcast::channel('game.{gameId}', function ($user, int $gameId) {
    $playerId = request()->header('X-Player-Id');
    if (!$playerId) return false;

    $player = Player::find($playerId);
    $game   = Game::with('binomes.players')->find($gameId);

    if (!$player || !$game) return false;

    $isInGame = $game->binomes
        ->flatMap(fn($b) => $b->players)
        ->contains('id', $player->id);

    if ($isInGame) {
        return ['id' => $player->id, 'pseudo' => $player->pseudo];
    }

    $isInRoom = $game->room->players->contains('id', $player->id);

    if (!$isInRoom) return false;

    return ['id' => $player->id, 'pseudo' => $player->pseudo];
});

Broadcast::channel('room.{roomId}', function ($user, int $roomId) {
    $playerId = request()->header('X-Player-Id');
    if (!$playerId) return false;

    $player = Player::find($playerId);
    $room   = Room::with('players')->find($roomId);

    if (!$player || !$room) return false;
    if (!$room->players->contains('id', $player->id)) return false;

    return [
        'id'     => $player->id,
        'pseudo' => $player->pseudo,
    ];
});
