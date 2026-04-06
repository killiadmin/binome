<?php

namespace App\Services;

use App\Events\BinomeDiscovered;
use App\Models\Binome;
use App\Models\Game;
use App\Models\Player;
use Exception;

class BinomeService
{
    /**
     * Marque le binome du joueur ciblé comme découvert
     */
    public function discoverBinome(Game $game, Player $target, Player $discoveredBy): Binome
    {
        $binome = $this->getBinomeOfPlayer($game, $target);

        if ($binome->is_discovered) {
            throw new Exception("Ce binome a déjà été découvert.");
        }

        $binome->update([
            'is_discovered' => true,
            'discovered_by_player_id' => $discoveredBy->id,
        ]);

        broadcast(new BinomeDiscovered($binome->load('players', 'universe')));

        return $binome;
    }

    /**
     * Retrouve le binome d'un joueur dans une partie
     */
    public function getBinomeOfPlayer(Game $game, Player $player): Binome
    {
        $binome = $game->binomes()
            ->whereHas('players', fn($q) => $q->where('player_id', $player->id))
            ->first();

        if (!$binome) {
            throw new Exception("Aucun binome trouvé pour ce joueur dans cette partie.");
        }

        return $binome;
    }

    /**
     * Vérifie si les deux joueurs d'un binome ont tous les deux été découverts
     * (utile si tu veux afficher la progression côté front)
     */
    public function getRemainingBinomes(Game $game): \Illuminate\Support\Collection
    {
        return $game->binomes()->where('is_discovered', false)->get();
    }
}
