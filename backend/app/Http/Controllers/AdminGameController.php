<?php

namespace App\Http\Controllers;

use App\Enums\ActionType;
use App\Models\Character;
use App\Models\Game;
use Illuminate\Database\QueryException;
use Illuminate\Http\JsonResponse;
use Illuminate\Routing\Controller;
use Illuminate\Support\Facades\DB;

class AdminGameController extends Controller
{
    /**
     * GET /admin/games
     * Liste de toutes les parties (en cours + terminées), la plus récente d'abord.
     */
    public function index(): JsonResponse
    {
        $games = Game::query()
            ->with('room:id,code')
            ->withCount(['binomes', 'rounds'])
            ->orderByDesc('created_at')
            ->get();

        // Compteurs joueurs / binomes découverts par partie, en 2 requêtes agrégées.
        $playersCount = DB::table('binome_player')
            ->join('binomes', 'binomes.id', '=', 'binome_player.binome_id')
            ->select('binomes.game_id', DB::raw('count(*) as total'))
            ->groupBy('binomes.game_id')
            ->pluck('total', 'game_id');

        $discoveredCount = DB::table('binomes')
            ->where('is_discovered', true)
            ->select('game_id', DB::raw('count(*) as total'))
            ->groupBy('game_id')
            ->pluck('total', 'game_id');

        return response()->json([
            'games' => $games->map(fn (Game $game) => [
                'id' => $game->id,
                'room_id' => $game->room_id,
                'room_code' => $game->room?->code,
                'status' => $game->status->value,
                'players_count' => (int) ($playersCount[$game->id] ?? 0),
                'binomes_count' => $game->binomes_count,
                'binomes_discovered_count' => (int) ($discoveredCount[$game->id] ?? 0),
                'rounds_count' => $game->rounds_count,
                'created_at' => $game->created_at?->toIso8601String(),
                'updated_at' => $game->updated_at?->toIso8601String(),
            ])->values(),
        ]);
    }

    /**
     * GET /admin/games/{game}
     * Détail complet d'une partie — aucun masquage (contexte admin) :
     * binomes + personnages révélés + journal des actions.
     */
    public function show(Game $game): JsonResponse
    {
        $game->load([
            'room:id,code',
            'binomes.universe.cosmos',
            'binomes.players',
            'rounds' => fn ($q) => $q->orderBy('number'),
            'rounds.actions' => fn ($q) => $q->orderBy('created_at'),
            'rounds.actions.player',
            'rounds.actions.targetPlayer',
            'gameStats',
        ]);

        // Résolution des personnages assignés en une seule requête.
        $characterIds = $game->binomes
            ->flatMap(fn ($binome) => $binome->players->pluck('pivot.character_id'))
            ->filter()
            ->unique();

        $characters = Character::with('universe:id,name')
            ->findMany($characterIds)
            ->keyBy('id');

        return response()->json([
            'id' => $game->id,
            'room_id' => $game->room_id,
            'room_code' => $game->room?->code,
            'status' => $game->status->value,
            'created_at' => $game->created_at?->toIso8601String(),
            'updated_at' => $game->updated_at?->toIso8601String(),
            'binomes' => $game->binomes->map(fn ($binome) => [
                'id' => $binome->id,
                'universe' => $binome->universe->name,
                'cosmos' => $binome->universe->cosmos?->name,
                'is_discovered' => (bool) $binome->is_discovered,
                'discovered_by_player_id' => $binome->discovered_by_player_id,
                'players' => $binome->players->map(function ($player) use ($characters) {
                    $character = $characters->get($player->pivot->character_id);

                    return [
                        'id' => $player->id,
                        'pseudo' => $player->pseudo,
                        'is_eliminated' => (bool) $player->pivot->is_eliminated,
                        'score' => $player->pivot->score,
                        'character' => $character ? [
                            'id' => $character->id,
                            'name' => $character->name,
                            'universe' => $character->universe?->name,
                            'forbidden_words' => $character->forbidden_words,
                        ] : null,
                    ];
                })->values(),
            ])->values(),
            'rounds' => $game->rounds->map(fn ($round) => [
                'id' => $round->id,
                'number' => $round->number,
                'is_finished' => (bool) $round->is_finished,
                'current_player_id' => $round->current_player_id,
                'actions' => $round->actions->map(fn ($action) => $this->formatAction($action))->values(),
            ])->values(),
            'game_stats' => $game->gameStats->map(fn ($stat) => [
                'player_pseudo' => $stat->player_pseudo,
                'character_name' => $stat->character_name,
                'eliminations' => $stat->eliminations,
                'rounds_survived' => $stat->rounds_survived,
                'survived_full_game' => $stat->survived_full_game,
                'score' => $stat->score,
                'is_winner' => $stat->is_winner,
                'is_eliminated' => $stat->is_eliminated,
            ])->values(),
        ]);
    }

    /**
     * DELETE /admin/games/{game}
     * Supprime la partie. La cascade DB nettoie binomes, binome_player, rounds,
     * actions et game_stats — ce qui libère les FK vers `characters`.
     */
    public function destroy(Game $game): JsonResponse
    {
        try {
            $game->delete();
        } catch (QueryException) {
            return response()->json([
                'message' => 'Impossible de supprimer cette partie.',
            ], 409);
        }

        return response()->json([
            'message' => 'Partie supprimée.',
        ]);
    }

    /**
     * Version non masquée de GameController::formatAction() — le contenu des
     * questions et le personnage accusé sont toujours exposés.
     */
    private function formatAction($action): array
    {
        $base = [
            'id' => $action->id,
            'round_id' => $action->round_id,
            'type' => $action->type->value,
            'player' => [
                'id' => $action->player->id,
                'pseudo' => $action->player->pseudo,
            ],
            'target_player' => $action->targetPlayer ? [
                'id' => $action->targetPlayer->id,
                'pseudo' => $action->targetPlayer->pseudo,
            ] : null,
            'is_valid' => (bool) $action->is_valid,
            'answer' => $action->answer,
            'created_at' => $action->created_at?->toIso8601String(),
        ];

        if ($action->type === ActionType::Question) {
            return array_merge($base, [
                'content' => $action->content,
                'refused_reason' => $action->is_valid ? null : 'Mot interdit détecté.',
            ]);
        }

        return array_merge($base, [
            'content' => $action->content,
            'character_name' => $action->character_name ?? $action->content,
            'accusation_correct' => $action->accusation_correct,
            'accusation_confirmed' => $action->accusation_confirmed,
        ]);
    }
}
