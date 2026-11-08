<?php

namespace App\Http\Controllers;

use App\Events\CardWordShared;
use App\Http\Requests\PlayQuestionRequest;
use App\Http\Requests\PlayAccusationRequest;
use App\Models\Action;
use App\Models\Game;
use App\Models\Round;
use App\Models\Player;
use App\Models\Character;
use App\Services\ActionService;
use Illuminate\Http\Request;
use Illuminate\Http\JsonResponse;
use Illuminate\Routing\Controller;

class ActionController extends Controller
{
    public function __construct(
        private readonly ActionService $actionService
    ) {}

    /**
     * POST /games/{game}/rounds/{round}/question
     */
    public function question(PlayQuestionRequest $request, Game $game, Round $round): JsonResponse
    {
        $player   = Player::findOrFail($request->validated('player_id'));
        $targetPlayer = Player::findOrFail($request->validated('target_player_id'));
        $question = $request->validated('question');

        $action = $this->actionService->playQuestion($round, $player, $question, $targetPlayer);

        return response()->json([
            'action'   => $action->load('player'),
            'is_valid' => $action->is_valid,
            'message'  => 'Question posée avec succès.',
        ]);
    }

    /**
     * POST /games/{game}/rounds/{round}/accusation
     */
    public function accusation(PlayAccusationRequest $request, Game $game, Round $round): JsonResponse
    {
        $player        = Player::findOrFail($request->validated('player_id'));
        $targetPlayer  = Player::findOrFail($request->validated('target_player_id'));
        $characterName = $request->validated('character_name');

        $action = $this->actionService->playAccusation(
            $round,
            $player,
            $targetPlayer,
            $characterName,
        );

        return response()->json([
            'action'  => $action->load('player', 'targetPlayer'),
            'message' => 'Accusation envoyée — en attente de confirmation.',
        ]);
    }

    public function confirmAccusation(Request $request, Game $game, Round $round, Action $action): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'confirmed' => ['required', 'boolean'],
        ]);

        $player    = Player::findOrFail($request->input('player_id'));
        $confirmed = $request->boolean('confirmed');

        $action = $this->actionService->confirmAccusation($action, $player, $confirmed);

        return response()->json([
            'action'  => $action,
            'message' => $confirmed ? 'Accusation confirmée.' : 'Accusation niée.',
        ]);
    }

    public function answer(Request $request, Game $game, Round $round, Action $action): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'answer'    => ['required', 'string', 'in:yes,no,dont_know'],
        ]);

        $player = Player::findOrFail($request->input('player_id'));
        $answer = $request->input('answer');
        $action = $this->actionService->playAnswer($action, $player, $answer);

        return response()->json([
            'action'  => $action,
            'message' => 'Réponse enregistrée.',
        ]);
    }

    /**
     * POST /games/{game}/rounds/{round}/actions/{action}/draw
     * Le questionneur retourne une carte après un « Je ne sais pas ».
     * Le mot n'est renvoyé qu'ici, à lui seul.
     */
    public function drawCard(Request $request, Game $game, Round $round, Action $action): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'card'      => ['required', 'integer', 'min:0'],
        ]);

        $player = Player::findOrFail($request->input('player_id'));
        $word   = $this->actionService->drawCard($action, $player, $request->integer('card'));

        return response()->json([
            'word'           => $word,
            'reveal_seconds' => CardWordShared::REVEAL_SECONDS,
        ]);
    }

    /**
     * POST /games/{game}/rounds/{round}/actions/{action}/draw/decision
     * Montrer le mot tiré à tous sauf la cible (share: true), ou le garder.
     */
    public function decideCardDraw(Request $request, Game $game, Round $round, Action $action): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'share'     => ['required', 'boolean'],
        ]);

        $player = Player::findOrFail($request->input('player_id'));
        $share  = $request->boolean('share');
        $this->actionService->decideCardDraw($action, $player, $share);

        return response()->json([
            'message' => $share ? 'Mot montré aux autres joueurs.' : 'Mot gardé pour toi.',
        ]);
    }
}
