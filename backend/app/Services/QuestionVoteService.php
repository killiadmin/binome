<?php

namespace App\Services;

use App\Enums\ActionType;
use App\Enums\GameStatus;
use App\Events\QuestionVoted;
use App\Exceptions\GameRuleException;
use App\Models\Action;
use App\Models\Game;
use App\Models\Player;
use App\Models\QuestionVote;
use App\Models\Round;

/**
 * « Meilleure question du round » : les joueurs éliminés votent (option de
 * l'hôte, rooms.best_question_vote). À la fin, l'auteur de la question la plus
 * votée de chaque round marque +1 (tous les ex æquo).
 *
 * Un éliminé ne vote ni pour lui-même ni pour son binôme : sinon il voterait
 * à chaque round pour son partenaire encore en jeu.
 */
class QuestionVoteService
{
    public function vote(Game $game, Player $voter, Action $action): void
    {
        if (! $game->room?->best_question_vote) {
            throw new GameRuleException('Le vote des éliminés est désactivé pour cette partie.');
        }

        if ($game->status !== GameStatus::InProgress || $game->isVotingCharacters()) {
            throw new GameRuleException("La partie n'est pas en cours.");
        }

        $binome = $game->binomes()
            ->whereHas('players', fn ($q) => $q->where('player_id', $voter->id))
            ->with('players')
            ->first();

        if (! $binome) {
            throw new GameRuleException('Tu ne joues pas dans cette partie.');
        }

        if (! $binome->players->firstWhere('id', $voter->id)->pivot->is_eliminated) {
            throw new GameRuleException('Seuls les joueurs éliminés votent.');
        }

        $round = $action->round;

        if ($round->game_id !== $game->id) {
            throw new GameRuleException("Cette question n'appartient pas à cette partie.");
        }

        if ($action->type !== ActionType::Question || ! $action->is_valid) {
            throw new GameRuleException('On ne vote que pour une question.');
        }

        if ($action->player_id === $voter->id) {
            throw new GameRuleException('Tu ne peux pas voter pour ta propre question.');
        }

        if ($binome->players->contains('id', $action->player_id)) {
            throw new GameRuleException('Tu ne peux pas voter pour ton binôme.');
        }

        // Round en cours, ou le précédent : la dernière question d'un round
        // reste votable pendant le round suivant.
        if ($round->number < (int) $game->rounds()->max('number') - 1) {
            throw new GameRuleException('Le vote est clos pour ce round.');
        }

        // Changer d'avis remplace le vote (unique round_id + voter_id)
        QuestionVote::updateOrCreate(
            ['round_id' => $round->id, 'voter_id' => $voter->id],
            ['action_id' => $action->id],
        );

        broadcast(new QuestionVoted($game, $round, $this->roundVotes($round)));
    }

    /**
     * Votes d'un round, publics : qui a voté pour quelle question.
     *
     * @return array<int, array{action_id: int, voter_id: int}>
     */
    public function roundVotes(Round $round): array
    {
        return $round->questionVotes()
            ->orderBy('id')
            ->get(['action_id', 'voter_id'])
            ->map(fn ($vote) => ['action_id' => $vote->action_id, 'voter_id' => $vote->voter_id])
            ->all();
    }

    /**
     * Question(s) gagnante(s) de chaque round : la ou les plus votées (≥ 1 vote).
     *
     * @return array<int, array{round: int, action: Action, votes: int}>
     */
    public function winningQuestions(Game $game): array
    {
        $votes = QuestionVote::query()
            ->whereIn('round_id', $game->rounds()->pluck('id'))
            ->with(['round:id,number', 'action.player'])
            ->get();

        return $votes->groupBy('round_id')
            ->flatMap(function ($roundVotes) {
                $counts = $roundVotes->countBy('action_id');
                $best = $counts->max();

                return $counts->filter(fn ($n) => $n === $best)->keys()->map(fn ($actionId) => [
                    'round' => $roundVotes->first()->round->number,
                    'action' => $roundVotes->firstWhere('action_id', $actionId)->action,
                    'votes' => $best,
                ]);
            })
            ->sortBy('round')
            ->values()
            ->all();
    }

    /**
     * player_id → nombre de rounds remportés (points « meilleure question »).
     *
     * @return array<int, int>
     */
    public function bestQuestionCounts(Game $game): array
    {
        return collect($this->winningQuestions($game))
            ->countBy(fn ($win) => $win['action']->player_id)
            ->all();
    }
}
