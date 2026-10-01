<?php

namespace App\Services;

use App\Enums\ActionType;
use App\Enums\GameStatus;
use App\Models\Action;
use App\Models\Character;
use App\Models\Game;
use App\Models\GameStat;
use App\Models\PlayerAvatar;
use Illuminate\Support\Facades\DB;

/**
 * Lecture de l'historique des parties et du classement cumulé des joueurs.
 *
 * Le même formatage sert à la vue admin (toutes les parties) et à l'historique
 * public (parties terminées uniquement) : c'est le controller qui décide de ce
 * qu'il autorise, pas ce service.
 */
class GameHistoryService
{
    /**
     * Liste des parties, la plus récente d'abord.
     *
     * @param  bool  $finishedOnly  Restreint aux parties terminées (historique public).
     */
    public function listGames(bool $finishedOnly = false): array
    {
        $games = Game::query()
            ->with('room:id,code')
            ->withCount(['binomes', 'rounds'])
            ->when($finishedOnly, fn ($q) => $q->where('status', GameStatus::Finished->value))
            ->orderByDesc('created_at')
            ->get();

        // Compteurs joueurs / binomes découverts par partie, en 2 requêtes agrégées.
        $playersCount = DB::table('binome_player')
            ->join('binomes', 'binomes.id', '=', 'binome_player.binome_id')
            ->select('binomes.game_id', DB::raw('count(*) as total'))
            ->groupBy('binomes.game_id')
            ->pluck('total', 'game_id');

        $discoveredCount = DB::table('binomes')
            ->where('is_discovered', true)
            ->select('game_id', DB::raw('count(*) as total'))
            ->groupBy('game_id')
            ->pluck('total', 'game_id');

        // Vainqueurs par partie, lus depuis game_stats (donc uniquement pour les
        // parties terminées — ScoreService n'écrit qu'à la clôture).
        $winners = GameStat::query()
            ->where('is_winner', true)
            ->whereIn('game_id', $games->pluck('id'))
            ->get(['game_id', 'player_pseudo'])
            ->groupBy('game_id')
            ->map(fn ($rows) => $rows->pluck('player_pseudo')->values()->all());

        return $games->map(fn (Game $game) => [
            'id' => $game->id,
            'room_id' => $game->room_id,
            'room_code' => $game->room?->code,
            'status' => $game->status->value,
            'players_count' => (int) ($playersCount[$game->id] ?? 0),
            'binomes_count' => $game->binomes_count,
            'binomes_discovered_count' => (int) ($discoveredCount[$game->id] ?? 0),
            'rounds_count' => $game->rounds_count,
            'winners' => $winners[$game->id] ?? [],
            'created_at' => $game->created_at?->toIso8601String(),
            'updated_at' => $game->updated_at?->toIso8601String(),
        ])->values()->all();
    }

    /**
     * Détail complet d'une partie : binomes, personnages révélés, journal des
     * actions et stats finales. Aucun masquage — les appelants qui exposent
     * cette charge utile publiquement doivent d'abord vérifier que la partie
     * est terminée (voir HistoryController).
     */
    public function detail(Game $game): array
    {
        $game->load([
            'room:id,code',
            'binomes.universe.cosmos',
            'binomes.players',
            'rounds' => fn ($q) => $q->orderBy('number'),
            'rounds.actions' => fn ($q) => $q->orderBy('created_at'),
            'rounds.actions.player',
            'rounds.actions.targetPlayer',
            'gameStats',
        ]);

        // Résolution des personnages assignés en une seule requête.
        $characterIds = $game->binomes
            ->flatMap(fn ($binome) => $binome->players->pluck('pivot.character_id'))
            ->filter()
            ->unique();

        $characters = Character::with('universe:id,name')
            ->findMany($characterIds)
            ->keyBy('id');

        // Photos de profil (par pseudo) : joueurs, scores et journal.
        $avatars = PlayerAvatar::urlsForPseudos(
            $game->binomes->flatMap(fn ($binome) => $binome->players->pluck('pseudo'))
                ->merge($game->gameStats->pluck('player_pseudo'))
        );

        return [
            'id' => $game->id,
            'room_id' => $game->room_id,
            'room_code' => $game->room?->code,
            'status' => $game->status->value,
            'created_at' => $game->created_at?->toIso8601String(),
            'updated_at' => $game->updated_at?->toIso8601String(),
            'binomes' => $game->binomes->map(fn ($binome) => [
                'id' => $binome->id,
                'universe' => $binome->universe->name,
                'cosmos' => $binome->universe->cosmos?->name,
                'is_discovered' => (bool) $binome->is_discovered,
                'is_orphan' => (bool) $binome->is_orphan,
                'discovered_by_player_id' => $binome->discovered_by_player_id,
                'players' => $binome->players->map(function ($player) use ($characters, $avatars) {
                    $character = $characters->get($player->pivot->character_id);

                    return [
                        'id' => $player->id,
                        'pseudo' => $player->pseudo,
                        'avatar_url' => $avatars[$player->pseudo] ?? null,
                        'is_eliminated' => (bool) $player->pivot->is_eliminated,
                        'score' => $player->pivot->score,
                        'character' => $character ? [
                            'id' => $character->id,
                            'name' => $character->name,
                            'universe' => $character->universe?->name,
                            'forbidden_words' => $character->forbidden_words,
                        ] : null,
                    ];
                })->values(),
            ])->values(),
            'rounds' => $game->rounds->map(fn ($round) => [
                'id' => $round->id,
                'number' => $round->number,
                'is_finished' => (bool) $round->is_finished,
                'current_player_id' => $round->current_player_id,
                'actions' => $round->actions->map(fn ($action) => $this->formatAction($action, $avatars))->values(),
            ])->values(),
            'game_stats' => $game->gameStats
                ->sortByDesc('score')
                ->map(fn ($stat) => [
                    'player_pseudo' => $stat->player_pseudo,
                    'avatar_url' => $avatars[$stat->player_pseudo] ?? null,
                    'character_name' => $stat->character_name,
                    'eliminations' => $stat->eliminations,
                    'rounds_survived' => $stat->rounds_survived,
                    'survived_full_game' => $stat->survived_full_game,
                    'score' => $stat->score,
                    'is_winner' => $stat->is_winner,
                    'is_eliminated' => $stat->is_eliminated,
                ])->values(),
        ];
    }

    /**
     * Classement cumulé, une ligne par pseudo.
     *
     * Le pseudo est la clé d'identité : `players.pseudo` est unique et
     * `Player::firstOrCreate(['pseudo' => …])` réutilise donc la même personne
     * d'une partie à l'autre. On agrège sur `game_stats.player_pseudo`, qui est
     * dénormalisé et survit à la suppression du joueur (RoomController::leave
     * supprime la ligne `players` quand il quitte le salon).
     *
     * La collation de la base est `utf8mb4_0900_ai_ci` : « Killian », « killian »
     * et « Killián » sont regroupés, exactement comme la contrainte d'unicité
     * sur `players.pseudo` les considère déjà comme un seul joueur.
     */
    public function leaderboard(): array
    {
        $rows = GameStat::query()
            // Restriction explicite aux parties terminées. En pratique
            // ScoreService n'écrit qu'à la clôture, mais l'endpoint est public :
            // il impose l'invariant au lieu de le supposer.
            ->join('games', 'games.id', '=', 'game_stats.game_id')
            ->where('games.status', GameStatus::Finished->value)
            ->selectRaw('MAX(game_stats.player_pseudo) as pseudo')
            ->selectRaw('COUNT(*) as games_played')
            ->selectRaw('COALESCE(SUM(game_stats.score), 0) as total_score')
            ->selectRaw('COALESCE(SUM(game_stats.is_winner), 0) as wins')
            ->selectRaw('COALESCE(SUM(game_stats.eliminations), 0) as eliminations')
            ->selectRaw('COALESCE(SUM(game_stats.rounds_survived), 0) as rounds_survived')
            ->groupBy('game_stats.player_pseudo')
            ->orderByDesc('total_score')
            ->orderByDesc('wins')
            ->orderBy('pseudo')
            ->get();

        $avatars = PlayerAvatar::urlsForPseudos($rows->pluck('pseudo'));

        return $this->withRanks($rows->map(fn ($row) => [
            'pseudo' => $row->pseudo,
            'avatar_url' => $avatars[$row->pseudo] ?? null,
            'games_played' => (int) $row->games_played,
            'total_score' => (int) $row->total_score,
            'wins' => (int) $row->wins,
            'eliminations' => (int) $row->eliminations,
            'rounds_survived' => (int) $row->rounds_survived,
            'average_score' => $row->games_played > 0
                ? round($row->total_score / $row->games_played, 1)
                : 0.0,
        ])->all());
    }

    /**
     * Détail d'un joueur : totaux cumulés + une ligne par partie jouée.
     *
     * C'est la contrepartie « splittée » du classement : le même pseudo cumule
     * ses points globalement, tout en conservant le détail partie par partie.
     */
    public function playerBreakdown(string $pseudo): ?array
    {
        $stats = GameStat::query()
            // Comparaison insensible à la casse via la collation de la base,
            // cohérente avec le regroupement du classement.
            ->where('player_pseudo', $pseudo)
            // Même restriction que le classement : une partie en cours ne doit
            // pas fuiter son personnage ni son score par cet endpoint public.
            ->whereHas('game', fn ($q) => $q->where('status', GameStatus::Finished->value))
            ->with('game.room:id,code')
            ->get();

        if ($stats->isEmpty()) {
            return null;
        }

        $games = $stats
            ->sortByDesc(fn (GameStat $stat) => $stat->game?->created_at)
            ->map(fn (GameStat $stat) => [
                'game_id' => $stat->game_id,
                'room_code' => $stat->game?->room?->code,
                'played_at' => $stat->game?->created_at?->toIso8601String(),
                'character_name' => $stat->character_name,
                'score' => (int) $stat->score,
                'eliminations' => (int) $stat->eliminations,
                'rounds_survived' => (int) $stat->rounds_survived,
                'is_winner' => (bool) $stat->is_winner,
                'is_eliminated' => (bool) $stat->is_eliminated,
            ])->values()->all();

        return [
            // Casse telle qu'enregistrée lors de la partie la plus récente.
            'pseudo' => $stats->sortByDesc(fn (GameStat $s) => $s->game?->created_at)->first()->player_pseudo,
            'totals' => [
                'games_played' => $stats->count(),
                'total_score' => (int) $stats->sum('score'),
                'wins' => $stats->where('is_winner', true)->count(),
                'eliminations' => (int) $stats->sum('eliminations'),
                'rounds_survived' => (int) $stats->sum('rounds_survived'),
                'average_score' => round($stats->sum('score') / $stats->count(), 1),
            ],
            'games' => $games,
        ];
    }

    /**
     * Rang sportif : les ex æquo partagent le même rang, le suivant saute
     * d'autant (1, 2, 2, 4).
     */
    private function withRanks(array $rows): array
    {
        $rank = 0;
        $previousScore = null;

        foreach ($rows as $index => &$row) {
            if ($row['total_score'] !== $previousScore) {
                $rank = $index + 1;
                $previousScore = $row['total_score'];
            }
            $row['rank'] = $rank;
        }

        return $rows;
    }

    /**
     * Version non masquée de GameController::formatAction() — le contenu des
     * questions et le personnage accusé sont toujours exposés.
     */
    /**
     * @param  array<string, string|null>  $avatars  pseudo → url de la photo
     */
    private function formatAction(Action $action, array $avatars = []): array
    {
        $base = [
            'id' => $action->id,
            'round_id' => $action->round_id,
            'type' => $action->type->value,
            'player' => [
                'id' => $action->player->id,
                'pseudo' => $action->player->pseudo,
                'avatar_url' => $avatars[$action->player->pseudo] ?? null,
            ],
            'target_player' => $action->targetPlayer ? [
                'id' => $action->targetPlayer->id,
                'pseudo' => $action->targetPlayer->pseudo,
                'avatar_url' => $avatars[$action->targetPlayer->pseudo] ?? null,
            ] : null,
            'is_valid' => (bool) $action->is_valid,
            'answer' => $action->answer,
            'created_at' => $action->created_at?->toIso8601String(),
        ];

        if ($action->type === ActionType::Question) {
            return array_merge($base, [
                'content' => $action->content,
                'refused_reason' => $action->is_valid ? null : 'Mot interdit détecté.',
            ]);
        }

        return array_merge($base, [
            'content' => $action->content,
            'character_name' => $action->character_name ?? $action->content,
            'accusation_correct' => $action->accusation_correct,
            'accusation_confirmed' => $action->accusation_confirmed,
        ]);
    }
}
