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
     * Son partenaire, s'il en a un, se retrouve rescapé isolé
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
     * Vérifie la condition de fin de partie après une élimination.
     *
     * Une « équipe intacte » est un binôme dont aucun membre n'a été éliminé —
     * donc une paire complète, ou un orphelin (binôme d'un seul joueur, formé
     * au démarrage quand les joueurs sont en nombre impair) encore en vie.
     * Un rescapé est un joueur encore actif dont le binôme a perdu l'autre
     * membre : il peut gagner, mais son binôme est brisé.
     *
     * Fin si : il ne reste qu'une équipe intacte et aucun rescapé,
     *          OU plus aucune équipe intacte et un seul rescapé.
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

        // Équipes intactes : paire complète encore debout, ou orphelin en vie
        $intactTeams = $game->binomes->filter(
            fn($binome) => $binome->players->every(fn($p) => !$p->pivot->is_eliminated)
        );

        // Joueurs actifs dont le binôme a été brisé par une élimination
        $loneSurvivors = $game->binomes
            ->reject(fn($binome) => $binome->players->every(fn($p) => !$p->pivot->is_eliminated))
            ->flatMap(fn($b) => $b->players->filter(fn($p) => !$p->pivot->is_eliminated))
            ->values();

        // Fin si exactement 1 équipe intacte et aucun rescapé
        if ($intactTeams->count() === 1 && $loneSurvivors->isEmpty()) {
            return $intactTeams->first()->players->all();
        }

        // Fin si 0 équipe intacte et exactement 1 rescapé
        if ($intactTeams->isEmpty() && $loneSurvivors->count() === 1) {
            return [$loneSurvivors->first()];
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
