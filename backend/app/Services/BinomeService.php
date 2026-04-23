<?php

namespace App\Services;

use App\Events\BinomeDiscovered;
use App\Events\PlayerEliminated;
use App\Models\Binome;
use App\Models\Game;
use App\Models\Player;
use Exception;

class BinomeService
{
    /**
     * Élimine un joueur (sans découvrir son binôme)
     * Son partenaire devient orphelin
     */
    public function eliminatePlayer(Game $game, Player $target, Player $eliminatedBy): void
    {
        $binome = $this->getBinomeOfPlayer($game, $target);

        // Marque uniquement ce joueur comme éliminé sur le pivot
        $binome->players()->updateExistingPivot($target->id, [
            'is_eliminated' => true,
        ]);

        broadcast(new PlayerEliminated($game, $target, $eliminatedBy));
    }

    /**
     * Vérifie la condition de fin de partie après une élimination
     * Fin si : il ne reste qu'un binôme complet OU un orphelin seul
     */
    public function checkGameOver(Game $game): ?array
    {
        $game->load(['binomes.players']);

        // Joueurs encore actifs (non éliminés)
        $activePlayers = $game->binomes
            ->flatMap(fn($b) => $b->players)
            ->filter(fn($p) => !$p->pivot->is_eliminated)
            ->values();

        $activeCount = $activePlayers->count();

        // Moins de 2 joueurs actifs → impossible de continuer
        if ($activeCount <= 1) {
            return $activePlayers->isNotEmpty()
                ? [$activePlayers->first()]
                : [];
        }

        // Cherche les binômes avec leurs deux joueurs encore actifs
        $completeBinomes = $game->binomes->filter(function ($binome) {
            $activePairs = $binome->players
                ->filter(fn($p) => !$p->pivot->is_eliminated)
                ->count();
            return $activePairs === 2;
        });

        // Joueurs actifs sans binôme complet (orphelins)
        $orphans = $game->binomes
            ->filter(function ($binome) {
                $active = $binome->players
                    ->filter(fn($p) => !$p->pivot->is_eliminated)
                    ->count();
                return $active === 1;
            })
            ->flatMap(fn($b) => $b->players->filter(fn($p) => !$p->pivot->is_eliminated))
            ->values();

        // Fin si exactement 1 binôme complet et aucun orphelin
        if ($completeBinomes->count() === 1 && $orphans->isEmpty()) {
            return $completeBinomes->first()->players->all();
        }

        // Fin si 0 binôme complet et exactement 1 orphelin
        if ($completeBinomes->isEmpty() && $orphans->count() === 1) {
            return [$orphans->first()];
        }

        return null; // Partie continue
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

    public function getRemainingBinomes(Game $game): \Illuminate\Support\Collection
    {
        return $game->binomes()->where('is_discovered', false)->get();
    }
}
