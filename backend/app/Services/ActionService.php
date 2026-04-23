<?php

namespace App\Services;

use App\Enums\ActionType;
use App\Events\AccusationConfirmed;
use App\Events\ActionPlayed;
use App\Events\AnswerGiven;
use App\Events\GameEnded;
use App\Models\Action;
use App\Models\Character;
use App\Models\Game;
use App\Models\Player;
use App\Models\Round;
use Illuminate\Support\Facades\DB;
use Exception;

class ActionService
{
    public function __construct(
        private readonly RoundService  $roundService,
        private readonly BinomeService $binomeService,
    ) {}

    // -------------------------------------------------------------------------
    // QUESTION
    // -------------------------------------------------------------------------

    public function playQuestion(Round $round, Player $player, string $question, Player $targetPlayer): Action
    {
        $game = $round->game;

        $this->validateTurn($round, $player);
        $this->validateHasNotPlayedThisRound($round, $player);

        // Récupère le personnage du joueur qui pose la question
        $character = $player->getCharacterInGame($game);

        // Vérifie les mots interdits du joueur questionneur
        if ($character->checkForbiddenWords($question)) {
            // On enregistre quand même l'action comme invalide
            $action = $this->storeAction(
                round:   $round,
                player:  $player,
                type:    ActionType::Question,
                content: $question,
                isValid: false,
            );

            broadcast(new ActionPlayed($action));

            // La question est refusée : le joueur perd son tour
            $this->advanceRound($round, $game);

            return $action;
        }

        // Question valide
        $action = $this->storeAction(
            round:   $round,
            player:  $player,
            type:    ActionType::Question,
            content: $question,
            isValid: true,
            targetPlayer: $targetPlayer,
        );

        broadcast(new ActionPlayed($action));

        return $action;
    }

    // -------------------------------------------------------------------------
    // ACCUSATION
    // -------------------------------------------------------------------------

    public function playAccusation(
        Round  $round,
        Player $player,
        Player $targetPlayer,
        string $characterName,
    ): Action {
        $game = $round->game;

        $this->validateTurn($round, $player);
        $this->validateHasNotPlayedThisRound($round, $player);

        // Enregistre l'accusation sans vérifier — on attend la confirmation
        $action = $this->storeAction(
            round:        $round,
            player:       $player,
            type:         ActionType::Accusation,
            content:      $characterName,
            isValid:      true,
            targetPlayer: $targetPlayer,
        );

        // Stocke le nom du personnage accusé
        $action->update(['character_name' => $characterName]);
        $action->load(['player', 'targetPlayer', 'round']);

        broadcast(new ActionPlayed($action));

        return $action;
    }

    // -------------------------------------------------------------------------
    // LOGIQUE INTERNE
    // -------------------------------------------------------------------------

    /**
     * Vérifie que c'est bien le tour de ce joueur
     */
    private function validateTurn(Round $round, Player $player): void
    {
        if ($round->current_player_id !== $player->id) {
            throw new Exception("Ce n'est pas ton tour de jouer.");
        }

        if ($round->is_finished) {
            throw new Exception("Ce round est déjà terminé.");
        }
    }

    /**
     * Vérifie que le joueur n'a pas déjà joué dans ce round
     */
    private function validateHasNotPlayedThisRound(Round $round, Player $player): void
    {
        $alreadyPlayed = $round->actions()
            ->where('player_id', $player->id)
            ->where('is_valid', true)
            ->exists();

        if ($alreadyPlayed) {
            throw new Exception("Tu as déjà joué ce round.");
        }
    }

    /**
     * Persiste l'action en base
     */
    private function storeAction(
        Round      $round,
        Player     $player,
        ActionType $type,
        string     $content,
        bool       $isValid,
        ?Player    $targetPlayer      = null,
        ?bool      $accusationCorrect = null,
    ): Action {
        $action = Action::create([
            'round_id'            => $round->id,
            'player_id'           => $player->id,
            'type'                => $type,
            'content'             => $content,
            'is_valid'            => $isValid,
            'target_player_id'    => $targetPlayer?->id,
            'accusation_correct'  => $accusationCorrect,
        ]);

        $action->load(['player', 'targetPlayer', 'round']);

        return $action;
    }

    /**
     * Une accusation correcte : vérifie si le binome est découvert
     * et si la partie est terminée
     */
    private function handleCorrectAccusation(
        Game   $game,
        Player $accuser,
        Player $target,
    ): void {
        DB::transaction(function () use ($game, $accuser, $target) {

            // Élimine le joueur ciblé (sans toucher au binôme)
            $this->binomeService->eliminatePlayer($game, $target, $accuser);

            // Vérifie si la partie est terminée
            $winners = $this->binomeService->checkGameOver($game);

            if ($winners !== null) {
                $this->endGame($game, $winners);
            }
        });
    }

    /**
     * Avance au joueur suivant ou crée un nouveau round
     */
    private function advanceRound(Round $round, Game $game): void
    {
        $roundFinished = $this->roundService->nextTurn($round);

        if ($roundFinished) {
            $game->refresh();
            if ($game->status === \App\Enums\GameStatus::InProgress) {
                $this->roundService->createRound($game, $round->number + 1);
            }
        }
    }

    /**
     * Clôture la partie
     */
    private function endGame(Game $game, array $winners): void
    {
        $game->update(['status' => \App\Enums\GameStatus::Finished]);
        broadcast(new GameEnded($game, collect($winners)));
    }

    public function playAnswer(Action $action, Player $player, string $answer): Action
    {
        if ($action->target_player_id !== $player->id) {
            throw new \RuntimeException("Tu n'es pas le joueur ciblé par cette question.");
        }

        if ($action->answer !== null) {
            throw new \RuntimeException("Cette question a déjà reçu une réponse.");
        }

        $action->update(['answer' => $answer]);
        $action->load(['player', 'targetPlayer', 'round']);

        broadcast(new AnswerGiven($action));

        $round = $action->round;
        $this->advanceRound($round, $round->game);

        return $action;
    }

    public function confirmAccusation(Action $action, Player $player, bool $confirmed): Action
    {
        if ($action->target_player_id !== $player->id) {
            throw new \RuntimeException("Tu n'es pas le joueur accusé.");
        }

        if ($action->accusation_confirmed !== null) {
            throw new \RuntimeException("Cette accusation a déjà été confirmée.");
        }

        $round = $action->round;
        $game  = $round->game;

        if ($confirmed) {
            $action->update([
                'accusation_confirmed' => true,
                'accusation_correct'   => true,
            ]);

            $action->load(['player', 'targetPlayer', 'round']);
            broadcast(new AccusationConfirmed($action));

            $this->handleCorrectAccusation($game, $action->player, $player);

        } else {
            $action->update([
                'accusation_confirmed' => false,
                'accusation_correct'   => false,
            ]);

            $action->load(['player', 'targetPlayer', 'round']);
            broadcast(new AccusationConfirmed($action));
        }

        // Dans tous les cas on avance le round après confirmation
        $this->advanceRound($round, $game);

        return $action;
    }
}
