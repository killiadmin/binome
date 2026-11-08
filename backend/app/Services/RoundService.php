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
     * Retourne true si le round vient d'être terminé (tous ont joué)
     *
     * `$fromPlayerId` : le joueur dont le tour s'achève (l'auteur de l'action
     * résolue). La mise à jour est conditionnelle : si le tour a déjà quitté
     * ce joueur (deuxième réponse à une question doublon, requête rejouée),
     * rien ne bouge — une action ne fait jamais avancer le tour deux fois, et
     * un round clos n'est jamais clos (ni recréé) une seconde fois.
     */
    public function nextTurn(Round $round, ?int $fromPlayerId = null): bool
    {
        $from    = $fromPlayerId ?? $round->current_player_id;
        $game    = $round->game;
        $players = $this->getOrderedPlayers($game, $round->number);

        // Premier joueur actif après le joueur courant dans l'ordre par id.
        // Pas de recherche d'index : le joueur courant peut ne plus être dans
        // la liste s'il vient d'être exclu (ActionService::excludePlayer).
        $next = $players->first(fn($p) => $p->id > $from);

        $moved = Round::whereKey($round->id)
            ->where('is_finished', false)
            ->where('current_player_id', $from)
            ->update($next ? ['current_player_id' => $next->id] : ['is_finished' => true]);

        $round->refresh();

        if ($moved === 0 || ! $next) {
            return $moved > 0;
        }

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
