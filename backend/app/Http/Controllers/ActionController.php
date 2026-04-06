<?php

namespace App\Http\Controllers;

use App\Http\Requests\PlayQuestionRequest;
use App\Http\Requests\PlayAccusationRequest;
use App\Models\Round;
use App\Models\Player;
use App\Models\Character;
use App\Services\ActionService;
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
    public function question(PlayQuestionRequest $request, Round $round): JsonResponse
    {
        $player   = Player::findOrFail($request->validated('player_id'));
        $question = $request->validated('question');

        $action = $this->actionService->playQuestion($round, $player, $question);

        return response()->json([
            'action'  => $action->load('player'),
            'is_valid' => $action->is_valid,
            'message' => $action->is_valid
                ? 'Question posée avec succès.'
                : 'Question refusée : elle contient un mot interdit.',
        ]);
    }

    /**
     * POST /games/{game}/rounds/{round}/accusation
     */
    public function accusation(PlayAccusationRequest $request, Round $round): JsonResponse
    {
        $player          = Player::findOrFail($request->validated('player_id'));
        $targetPlayer    = Player::findOrFail($request->validated('target_player_id'));
        $guessedCharacter = Character::findOrFail($request->validated('character_id'));

        $action = $this->actionService->playAccusation(
            $round,
            $player,
            $targetPlayer,
            $guessedCharacter,
        );

        return response()->json([
            'action'             => $action->load('player', 'targetPlayer'),
            'accusation_correct' => $action->accusation_correct,
            'message'            => $action->accusation_correct
                ? 'Bonne accusation ! Le personnage est découvert.'
                : 'Mauvaise accusation.',
        ]);
    }
}
