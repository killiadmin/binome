<?php

namespace App\Services;

use App\Events\RoundStarted;
use App\Models\Game;
use App\Models\Round;
use Illuminate\Support\Collection;

class RoundService
{
    /**
     * Crée un nouveau round avec l'ordre des joueurs
     */
    public function createRound(Game $game, int $roundNumber): Round
    {
        // Récupère tous les joueurs de la partie via les binomes
        $players = $this->getOrderedPlayers($game, $roundNumber);

        // Le premier joueur à jouer dans ce round
        $firstPlayer = $players->first();

        $round = Round::create([
            'game_id'           => $game->id,
            'number'            => $roundNumber,
            'current_player_id' => $firstPlayer->id,
            'is_finished'       => false,
        ]);

        broadcast(new RoundStarted($round->load('currentPlayer'), isNewRound: true));

        return $round;
    }

    /**
     * Passe au joueur suivant dans le round
     * Retourne true si le round est terminé (tous ont joué)
     */
    public function nextTurn(Round $round): bool
    {
        $game    = $round->game;
        $players = $this->getOrderedPlayers($game, $round->number);

        $currentIndex = $players->search(
            fn($p) => $p->id === $round->current_player_id
        );

        $nextIndex = $currentIndex + 1;

        if ($nextIndex >= $players->count()) {
            $round->update(['is_finished' => true]);
            return true;
        }

        $round->update([
            'current_player_id' => $players[$nextIndex]->id,
        ]);

        $round->refresh();
        broadcast(new RoundStarted($round->load('currentPlayer'), isNewRound: false));

        return false;
    }

    /**
     * Ordre des joueurs : toujours le même au sein d'une partie
     * (basé sur l'id pour être déterministe)
     */
    private function getOrderedPlayers(Game $game, int $roundNumber): Collection
    {
        $game->load(['binomes.players']);

        return $game->binomes
            ->flatMap(fn($binome) => $binome->players)
            ->filter(fn($player) => !$player->pivot->is_eliminated)
            ->sortBy('id')
            ->values();
    }
}
