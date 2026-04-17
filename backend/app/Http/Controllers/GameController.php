<?php

namespace App\Http\Controllers;

use App\Enums\ActionType;
use App\Models\Action;
use App\Models\Game;
use App\Models\Room;
use App\Models\Player;
use App\Services\GameService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

class GameController extends Controller
{
    public function __construct(
        private readonly GameService $gameService
    ) {}

    /**
     * POST /rooms/{room}/start
     * Lance la partie depuis un salon
     */
    public function start(Request $request, Room $room): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
        ]);

        // Seul le créateur du salon peut lancer la partie
        $player = Player::findOrFail($request->input('player_id'));

        if ($room->created_by !== $player->id) {
            return response()->json([
                'message' => 'Seul le créateur du salon peut lancer la partie.',
            ], 403);
        }

        if ($room->currentGame()->exists()) {
            return response()->json([
                'message' => 'Une partie est déjà en cours dans ce salon.',
            ], 409);
        }

        $game = $this->gameService->start($room);

        return response()->json([
            'message' => 'La partie a démarré.',
            'game_id' => $game->id,
            'status'  => $game->status,
        ], 201);
    }

    /**
     * GET /games/{game}
     * Récupère l'état courant de la partie
     */
    public function show(Game $game): JsonResponse
    {
        $game->load([
            'binomes.universe',
            'binomes.players',
            'rounds' => fn($q) => $q->orderBy('number'),
        ]);

        $currentRound = $game->rounds
            ->where('is_finished', false)
            ->sortByDesc('number')
            ->first();

        // ── Charge les actions du round courant ──────────────────────────────
        $actions = collect();
        foreach ($game->rounds as $round) {
            $roundActions = $round->actions()
                ->with(['player', 'targetPlayer'])
                ->orderBy('created_at')
                ->get()
                ->map(fn($action) => $this->formatAction($action));
            $actions = $actions->merge($roundActions);
        }

        return response()->json([
            'game_id' => $game->id,
            'status'  => $game->status,
            'binomes' => $game->binomes->map(fn($binome) => [
                'id'            => $binome->id,
                'universe'      => $binome->universe->name,
                'is_discovered' => $binome->is_discovered,
                'players'       => $binome->players->map(fn($player) => [
                    'id'     => $player->id,
                    'pseudo' => $player->pseudo,
                ]),
            ]),
            'current_round' => $currentRound ? [
                'id'                => $currentRound->id,
                'number'            => $currentRound->number,
                'current_player_id' => $currentRound->current_player_id,
            ] : null,
            'actions' => $actions->values()->toArray(),
        ]);
    }

    private function formatAction(Action $action): array
    {
        $base = [
            'action_id'  => $action->id,
            'id'         => $action->id,
            'round_id'   => $action->round_id,
            'round_number' => $action->round->number,
            'player'     => [
                'id'     => $action->player->id,
                'pseudo' => $action->player->pseudo,
            ],
            'type'       => $action->type,
            'is_valid'   => $action->is_valid,
            'answer'     => $action->answer,
            'created_at' => $action->created_at->toIso8601String(),
        ];

        if ($action->type === ActionType::Question) {
            return array_merge($base, [
                'question'       => $action->is_valid ? $action->content : null,
                'refused_reason' => !$action->is_valid ? 'Mot interdit détecté.' : null,
                'target_player'  => $action->targetPlayer ? [
                    'id'     => $action->targetPlayer->id,
                    'pseudo' => $action->targetPlayer->pseudo,
                ] : null,
            ]);
        }

        return array_merge($base, [
            'target_player' => $action->targetPlayer ? [
                'id'     => $action->targetPlayer->id,
                'pseudo' => $action->targetPlayer->pseudo,
            ] : null,
            'accusation_correct' => $action->accusation_correct,
            'character_name'     => $action->accusation_correct ? $action->content : null,
        ]);
    }

    /**
     * GET /games/{game}/me
     * Retourne le personnage secret du joueur connecté
     * Endpoint privé — chaque joueur appelle ça pour lui-même au démarrage
     */
    public function myCharacter(Request $request, Game $game): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
        ]);

        $player    = Player::findOrFail($request->input('player_id'));
        $character = $player->getCharacterInGame($game);

        if (!$character) {
            return response()->json([
                'message' => 'Aucun personnage trouvé pour ce joueur dans cette partie.',
            ], 404);
        }

        return response()->json([
            'character' => [
                'id'             => $character->id,
                'name'           => $character->name,
                'universe'       => $character->universe->name,
                'forbidden_words'=> $character->forbidden_words,
            ],
        ]);
    }
}
