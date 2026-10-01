<?php

namespace App\Http\Controllers;

use App\Models\Game;
use App\Services\GameHistoryService;
use Illuminate\Database\QueryException;
use Illuminate\Http\JsonResponse;
use Illuminate\Routing\Controller;

class AdminGameController extends Controller
{
    public function __construct(
        private readonly GameHistoryService $history,
    ) {}

    /**
     * GET /admin/games
     * Liste de toutes les parties (en cours + terminées), la plus récente d'abord.
     */
    public function index(): JsonResponse
    {
        return response()->json([
            'games' => $this->history->listGames(),
        ]);
    }

    /**
     * GET /admin/games/{game}
     * Détail complet d'une partie — aucun masquage (contexte admin) :
     * binomes + personnages révélés + journal des actions. Contrairement à
     * HistoryController, les parties en cours sont visibles ici.
     */
    public function show(Game $game): JsonResponse
    {
        return response()->json($this->history->detail($game));
    }

    /**
     * DELETE /admin/games/{game}
     * Supprime la partie. La cascade DB nettoie binomes, binome_player, rounds,
     * actions et game_stats — ce qui libère les FK vers `characters` et retire
     * du même coup la partie du classement public.
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
}
