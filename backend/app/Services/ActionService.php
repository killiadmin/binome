<?php

namespace App\Services;

use App\Enums\ActionType;
use App\Events\ActionPlayed;
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

    public function playQuestion(Round $round, Player $player, string $question): Action
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
        );

        broadcast(new ActionPlayed($action));

        $this->advanceRound($round, $game);

        return $action;
    }

    // -------------------------------------------------------------------------
    // ACCUSATION
    // -------------------------------------------------------------------------

    public function playAccusation(
        Round    $round,
        Player   $player,
        Player   $targetPlayer,
        Character $guessedCharacter,
    ): Action {
        $game = $round->game;

        $this->validateTurn($round, $player);
        $this->validateHasNotPlayedThisRound($round, $player);

        // Vérifie si le personnage accusé est bien celui du joueur ciblé
        $realCharacter   = $targetPlayer->getCharacterInGame($game);
        $isCorrect       = $realCharacter?->id === $guessedCharacter->id;

        $action = $this->storeAction(
            round:              $round,
            player:             $player,
            type:               ActionType::Accusation,
            content:            $guessedCharacter->name,
            isValid:            true,
            targetPlayer:       $targetPlayer,
            accusationCorrect:  $isCorrect,
        );

        broadcast(new ActionPlayed($action));

        if ($isCorrect) {
            $this->handleCorrectAccusation($game, $player, $targetPlayer);
        }

        $this->advanceRound($round, $game);

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
        return Action::create([
            'round_id'            => $round->id,
            'player_id'           => $player->id,
            'type'                => $type,
            'content'             => $content,
            'is_valid'            => $isValid,
            'target_player_id'    => $targetPlayer?->id,
            'accusation_correct'  => $accusationCorrect,
        ]);
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
            $binome = $this->binomeService->discoverBinome($game, $target, $accuser);

            // Vérifie si c'était le dernier binome non découvert
            $allDiscovered = $game->binomes()->where('is_discovered', false)->doesntExist();

            if ($allDiscovered) {
                // Le dernier binome découvert PERD : les autres gagnent
                $this->endGame($game, losingBinomeId: $binome->id);
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
            // Vérifie que la partie n'est pas terminée avant de créer un round
            $game->refresh();
            if ($game->status === \App\Enums\GameStatus::InProgress) {
                $this->roundService->createRound($game, $round->number + 1);
            }
        }
    }

    /**
     * Clôture la partie
     */
    private function endGame(Game $game, int $losingBinomeId): void
    {
        // Les binomes gagnants = tous sauf le dernier découvert
        $winningBinomes = $game->binomes
            ->where('id', '!=', $losingBinomeId)
            ->values();

        $game->update(['status' => \App\Enums\GameStatus::Finished]);

        broadcast(new GameEnded($game, $winningBinomes));
    }
}
