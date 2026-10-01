<?php

namespace App\Services;

use App\Models\Game;
use App\Models\GameStat;
use Illuminate\Support\Collection;

class ScoreService
{
    /**
     * Calcule et persiste les stats de fin de partie
     * Retourne la collection des GameStat créées
     */
    public function computeAndStore(Game $game, array $winnerIds): Collection
    {
        $game->load([
            'binomes.players',
            'rounds.actions',
        ]);

        $totalRounds = $game->rounds->count();
        $stats       = collect();

        foreach ($game->binomes as $binome) {

            // Une équipe est « intacte » si aucun de ses membres n'a été
            // éliminé. Un rescapé isolé (partenaire éliminé) peut gagner la
            // partie, mais il ne touche pas les points de l'équipe gagnante :
            // son binôme est brisé. L'orphelin, lui, est une équipe d'un seul
            // joueur : tant qu'il est en vie son équipe est intacte, et il
            // gagne donc le bonus complet s'il est le dernier en jeu.
            $teamIntact = $binome->players
                ->every(fn($p) => ! $p->pivot->is_eliminated);

            foreach ($binome->players as $player) {

                $isEliminated = (bool) $player->pivot->is_eliminated;
                $isExcluded   = (bool) $player->pivot->is_excluded;
                $isWinner     = in_array($player->id, $winnerIds);

                // Personnage snapshot
                $character = \App\Models\Character::find($player->pivot->character_id);

                // Rounds survécus = rounds où le joueur n'était pas encore éliminé
                // On compte les rounds où le joueur a joué OU était encore actif
                $roundsSurvived = $this->countRoundsSurvived($game, $player);

                // Éliminations causées par ce joueur
                $eliminations = $game->rounds
                    ->flatMap(fn($r) => $r->actions)
                    ->filter(fn($a) =>
                        $a->player_id === $player->id &&
                        $a->type->value === 'accusation' &&
                        $a->accusation_correct === true
                    )
                    ->count();

                // Score — un joueur exclu par l'hôte (déconnecté) marque 0.
                $score = $isExcluded ? 0 : $eliminations * 1
                    + $roundsSurvived * 1
                    + ($isWinner && !$isEliminated && $teamIntact ? 5 : 0);

                $stat = GameStat::create([
                    'game_id'            => $game->id,
                    'player_pseudo'      => $player->pseudo,
                    'character_name'     => $character?->name ?? '?',
                    'eliminations'       => $eliminations,
                    'rounds_survived'    => $roundsSurvived,
                    'survived_full_game' => $isWinner && !$isEliminated,
                    'score'              => $score,
                    'is_winner'          => $isWinner,
                    'is_eliminated'      => $isEliminated,
                    'is_excluded'        => $isExcluded,
                ]);

                $stats->push($stat);
            }
        }

        return $stats;
    }

    private function countRoundsSurvived(Game $game, \App\Models\Player $player): int
    {
        $playerId = $player->id;

        // Éliminé hors accusation : exclu par l'hôte, ou binôme d'un exclu
        if ($player->pivot->eliminated_round !== null) {
            return (int) $player->pivot->eliminated_round;
        }

        // Cherche dans quelle action le joueur a été éliminé
        $eliminationAction = $game->rounds
            ->flatMap(fn($r) => $r->actions)
            ->filter(fn($a) =>
                $a->target_player_id === $playerId &&
                $a->type->value === 'accusation' &&
                $a->accusation_correct === true
            )
            ->first();

        if (!$eliminationAction) {
            // Non éliminé → a survécu tous les rounds
            return $game->rounds->count();
        }

        // Survécu jusqu'au round de son élimination (inclus)
        $eliminationRound = $game->rounds
            ->firstWhere('id', $eliminationAction->round_id);

        return $eliminationRound?->number ?? 0;
    }
}
