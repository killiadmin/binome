<?php

namespace App\Http\Controllers;

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
            'rounds' => fn($q) => $q->where('is_finished', false)->latest(),
        ]);

        $currentRound = $game->rounds->first();

        return response()->json([
            'game_id' => $game->id,
            'status'  => $game->status,
            'binomes' => $game->binomes->map(fn($binome) => [
                'id'           => $binome->id,
                'universe'     => $binome->universe->name,
                'is_discovered'=> $binome->is_discovered,
                'players'      => $binome->players->map(fn($player) => [
                    'id'     => $player->id,
                    'pseudo' => $player->pseudo,
                ]),
            ]),
            'current_round' => $currentRound ? [
                'id'                => $currentRound->id,
                'number'            => $currentRound->number,
                'current_player_id' => $currentRound->current_player_id,
            ] : null,
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
