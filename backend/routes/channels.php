<?php

use App\Models\Game;
use App\Models\Player;

/*
 * PresenceChannel game.{gameId}
 * Vérifie que le joueur appartient bien à cette partie
 * Retourne les infos du joueur pour la presence list côté front
 */
Broadcast::channel('game.{gameId}', function (Player $player, int $gameId) {
    $game = Game::with('binomes.players')->find($gameId);

    if (!$game) return false;

    $isInGame = $game->binomes
        ->flatMap(fn($b) => $b->players)
        ->contains('id', $player->id);

    if (!$isInGame) return false;

    // Ce qu'on expose dans la presence list
    return [
        'id'     => $player->id,
        'pseudo' => $player->pseudo,
    ];
});
