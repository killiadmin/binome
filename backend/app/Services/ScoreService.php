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
            foreach ($binome->players as $player) {

                $isEliminated = (bool) $player->pivot->is_eliminated;
                $isWinner     = in_array($player->id, $winnerIds);

                // Personnage snapshot
                $character = \App\Models\Character::find($player->pivot->character_id);

                // Rounds survécus = rounds où le joueur n'était pas encore éliminé
                // On compte les rounds où le joueur a joué OU était encore actif
                $roundsSurvived = $this->countRoundsSurvived($game, $player->id);

                // Éliminations causées par ce joueur
                $eliminations = $game->rounds
                    ->flatMap(fn($r) => $r->actions)
                    ->filter(fn($a) =>
                        $a->player_id === $player->id &&
                        $a->type->value === 'accusation' &&
                        $a->accusation_correct === true
                    )
                    ->count();

                // Score
                $score = $eliminations * 1
                    + $roundsSurvived * 1
                    + ($isWinner && !$isEliminated ? 5 : 0);

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
                ]);

                $stats->push($stat);
            }
        }

        return $stats;
    }

    private function countRoundsSurvived(Game $game, int $playerId): int
    {
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
