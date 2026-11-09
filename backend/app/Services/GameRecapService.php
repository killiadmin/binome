<?php

namespace App\Services;

use App\Enums\ActionType;
use App\Models\Action;
use App\Models\Character;
use App\Models\Game;
use App\Models\PlayerAvatar;
use App\Support\ForbiddenWordMatcher;
use Illuminate\Support\Collection;

/**
 * Récap de fin de partie : scores, révélation des binomes, qui a trouvé qui,
 * et quelques « trophées » calculés à partir du journal des actions.
 *
 * Tout est révélé (personnages, mots interdits piégés) : les appelants ne
 * doivent l'exposer qu'une fois la partie terminée (voir GameController::recap).
 */
class GameRecapService
{
    public function __construct(
        private readonly QuestionVoteService $questionVoteService,
    ) {}

    public function build(Game $game): array
    {
        $game->load([
            'binomes.universe.cosmos',
            'binomes.players',
            'rounds' => fn ($q) => $q->orderBy('number'),
            'rounds.actions' => fn ($q) => $q->orderBy('id'),
            'rounds.actions.player',
            'rounds.actions.targetPlayer',
            'gameStats',
        ]);

        $characterIds = $game->binomes
            ->flatMap(fn ($binome) => $binome->players->pluck('pivot.character_id'))
            ->filter()
            ->unique();

        $characters = Character::withoutImage()->findMany($characterIds)->keyBy('id');

        // player_id → personnage, pour repérer les mots interdits prononcés.
        $characterByPlayer = $game->binomes
            ->flatMap(fn ($binome) => $binome->players)
            ->mapWithKeys(fn ($player) => [$player->id => $characters->get($player->pivot->character_id)]);

        $actions = $game->rounds->flatMap(function ($round) {
            return $round->actions->each(fn (Action $action) => $action->setAttribute('round_number', $round->number));
        })->values();

        $questions = $actions->filter(fn (Action $a) => $a->type === ActionType::Question && $a->is_valid);
        $accusations = $actions->filter(fn (Action $a) => $a->type === ActionType::Accusation);

        // Question « piégée » : elle contient un mot interdit de la cible (exact
        // → mensonge obligatoire, proche → mensonge légal).
        $trapped = $questions->filter(fn (Action $a) => ForbiddenWordMatcher::matchesAny(
            $a->content,
            $characterByPlayer->get($a->target_player_id)?->forbidden_words ?? [],
        ));

        $correct = $accusations->filter(fn (Action $a) => $a->accusation_correct === true);
        $wrong = $accusations->filter(fn (Action $a) => $a->accusation_correct === false);

        $lastActivity = $actions->max('updated_at') ?? $game->updated_at;

        // Pour la mise en avant du gagnant : photo de chaque joueur.
        $avatarUrls = PlayerAvatar::urlsFor($game->binomes->flatMap(fn ($binome) => $binome->players));

        return [
            'game_id' => $game->id,
            'duration_seconds' => $game->created_at && $lastActivity
                ? (int) $game->created_at->diffInSeconds($lastActivity, true)
                : null,
            'rounds_count' => $game->rounds->count(),
            'questions_count' => $questions->count(),
            'accusations_count' => $accusations->count(),
            'winners' => $game->gameStats
                ->where('is_winner', true)
                ->pluck('player_pseudo')
                ->values(),
            'stats' => $game->gameStats
                ->sortByDesc('score')
                ->map(fn ($stat) => [
                    'player_pseudo' => $stat->player_pseudo,
                    'character_name' => $stat->character_name,
                    'eliminations' => $stat->eliminations,
                    'best_questions' => (int) $stat->best_questions,
                    'rounds_survived' => $stat->rounds_survived,
                    'survived_full_game' => (bool) $stat->survived_full_game,
                    'score' => $stat->score,
                    'is_winner' => (bool) $stat->is_winner,
                    'is_eliminated' => (bool) $stat->is_eliminated,
                    'is_excluded' => (bool) $stat->is_excluded,
                ])->values(),
            'binomes' => $game->binomes->map(fn ($binome) => [
                'id' => $binome->id,
                'universe' => $binome->universe->name,
                'cosmos' => $binome->universe->cosmos?->name,
                'is_orphan' => (bool) $binome->is_orphan,
                'players' => $binome->players->map(fn ($player) => [
                    'pseudo' => $player->pseudo,
                    'character' => $characters->get($player->pivot->character_id)?->name,
                    'character_image' => $characters->get($player->pivot->character_id)?->imageUrl(),
                    'avatar_url' => $avatarUrls[$player->id] ?? null,
                    'is_eliminated' => (bool) $player->pivot->is_eliminated,
                ])->values(),
            ])->values(),
            // Joueurs exclus par l'hôte (déconnectés), avec le binôme tombé avec eux
            'exclusions' => $game->binomes
                ->flatMap(fn ($binome) => $binome->players
                    ->filter(fn ($player) => $player->pivot->is_excluded)
                    ->map(fn ($player) => [
                        'round' => $player->pivot->eliminated_round,
                        'pseudo' => $player->pseudo,
                        // Seulement s'il est tombé à cause de l'exclusion (pas déjà démasqué avant)
                        'partner' => $binome->players->first(fn ($p) => $p->id !== $player->id
                            && ! $p->pivot->is_excluded
                            && $p->pivot->eliminated_round !== null)?->pseudo,
                    ]))
                ->values(),
            // Vote des éliminés : question(s) gagnante(s) de chaque round
            'best_questions' => collect($this->questionVoteService->winningQuestions($game))
                ->map(fn ($win) => [
                    'round' => $win['round'],
                    'author' => $win['action']->player?->pseudo,
                    'question' => $win['action']->content,
                    'votes' => $win['votes'],
                ])->values(),
            'eliminations' => $correct->map(fn (Action $a) => [
                'round' => $a->round_number,
                'by' => $a->player->pseudo,
                'target' => $a->targetPlayer?->pseudo,
                'character' => $a->character_name ?? $a->content,
            ])->values(),
            'awards' => array_values(array_filter([
                $this->award('detective', '🕵️', 'Fin limier', $correct, 'player_id',
                    fn ($n) => "$n accusation".($n > 1 ? 's' : '').' juste'.($n > 1 ? 's' : '')),
                $this->award('trigger_happy', '🎲', 'Tir à côté', $wrong, 'player_id',
                    fn ($n) => "$n accusation".($n > 1 ? 's' : '').' ratée'.($n > 1 ? 's' : '')),
                $this->award('suspect', '🎯', 'Le plus soupçonné', $accusations, 'target_player_id',
                    fn ($n) => "accusé $n fois"),
                $this->award('curious', '🔎', 'Le plus curieux', $questions, 'player_id',
                    fn ($n) => "$n question".($n > 1 ? 's' : '').' posée'.($n > 1 ? 's' : '')),
                $this->award('grilled', '🔥', 'Sur le grill', $questions, 'target_player_id',
                    fn ($n) => "$n question".($n > 1 ? 's' : '').' reçue'.($n > 1 ? 's' : '')),
                $this->award('clumsy', '💣', 'Maladroit', $trapped, 'player_id',
                    fn ($n) => "a prononcé $n mot".($n > 1 ? 's' : '').' interdit'.($n > 1 ? 's' : '')),
                $this->award('liar', '🤥', 'Permis de mentir', $trapped, 'target_player_id',
                    fn ($n) => "$n fois autorisé à mentir"),
            ])),
        ];
    }

    /**
     * Trophée attribué au(x) joueur(s) le(s) plus représenté(s) dans $actions
     * pour la colonne $column. Null si personne n'est concerné, ou si trop de
     * joueurs sont ex æquo pour que le trophée ait un sens.
     */
    private function award(string $key, string $emoji, string $title, Collection $actions, string $column, callable $detail): ?array
    {
        $counts = $actions
            ->filter(fn (Action $a) => $a->{$column} !== null)
            ->groupBy($column)
            ->map->count();

        if ($counts->isEmpty()) {
            return null;
        }

        $best = $counts->max();
        $relation = $column === 'player_id' ? 'player' : 'targetPlayer';

        $pseudos = $counts
            ->filter(fn ($n) => $n === $best)
            ->keys()
            ->map(fn ($playerId) => $actions->first(fn (Action $a) => $a->{$column} === $playerId)?->{$relation}?->pseudo)
            ->filter()
            ->values();

        // Un trophée partagé par plus de deux joueurs ne distingue personne.
        if ($pseudos->count() > 2) {
            return null;
        }

        return [
            'key' => $key,
            'emoji' => $emoji,
            'title' => $title,
            'pseudos' => $pseudos,
            'detail' => $detail($best),
        ];
    }
}
