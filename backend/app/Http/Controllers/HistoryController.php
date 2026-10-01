<?php

namespace App\Http\Controllers;

use App\Enums\GameStatus;
use App\Models\Game;
use App\Services\GameHistoryService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

/**
 * Historique public : consultable sans aucune authentification.
 *
 * Invariant de sécurité : seules les parties TERMINÉES sont exposées ici.
 * Publier le détail d'une partie en cours révélerait les personnages à ses
 * propres joueurs (il suffirait d'ouvrir un second onglet pour tricher). La
 * vue admin (`/api/admin/games`, protégée par `characters.access`) reste la
 * seule à voir les parties en cours.
 */
class HistoryController extends Controller
{
    public function __construct(
        private readonly GameHistoryService $history,
    ) {}

    /**
     * GET /history/games
     */
    public function games(): JsonResponse
    {
        return response()->json([
            'games' => $this->history->listGames(finishedOnly: true),
        ]);
    }

    /**
     * GET /history/games/{game}
     */
    public function game(Game $game): JsonResponse
    {
        if ($game->status !== GameStatus::Finished) {
            return response()->json([
                'message' => "Cette partie n'est pas terminée.",
            ], 404);
        }

        return response()->json($this->history->detail($game));
    }

    /**
     * GET /history/leaderboard
     */
    public function leaderboard(): JsonResponse
    {
        return response()->json([
            'leaderboard' => $this->history->leaderboard(),
        ]);
    }

    /**
     * GET /history/players?pseudo=…
     *
     * Le pseudo passe en query string : il est saisi librement par le joueur
     * (`min:2|max:20`) et peut contenir des caractères qu'un segment d'URL ne
     * peut pas porter.
     */
    public function player(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'pseudo' => ['required', 'string', 'max:20'],
        ]);

        $breakdown = $this->history->playerBreakdown($validated['pseudo']);

        if ($breakdown === null) {
            return response()->json(['message' => 'Aucune partie pour ce joueur.'], 404);
        }

        return response()->json($breakdown);
    }
}
