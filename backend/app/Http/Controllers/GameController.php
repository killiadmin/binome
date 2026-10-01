<?php

namespace App\Http\Controllers;

use App\Enums\ActionType;
use App\Models\Action;
use App\Models\Game;
use App\Models\Player;
use App\Models\PlayerAvatar;
use App\Models\Room;
use App\Enums\GameStatus;
use App\Events\ReactionSent;
use App\Services\ActionService;
use App\Services\GameRecapService;
use App\Services\GameService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

class GameController extends Controller
{
    /** Réactions rapides autorisées — à garder aligné sur QuickReactions.vue. */
    public const REACTIONS = ['😂', '🤔', '😱', '👏', '🔥', '🤫', '😈', '💀'];

    public function __construct(
        private readonly GameService $gameService,
        private readonly ActionService $actionService,
        private readonly GameRecapService $recapService,
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

        try {
            $game = $this->gameService->start($room);
        } catch (\Throwable $e) {
            return response()->json([
                'message' => $e->getMessage(),
            ], 422);
        }

        return response()->json([
            'message' => 'La partie a démarré.',
            'game_id' => $game->id,
            'status' => $game->status,
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
            'binomes.universe.cosmos',
            'binomes.players',
            'rounds' => fn ($q) => $q->orderBy('number'),
        ]);

        $currentRound = $game->rounds
            ->where('is_finished', false)
            ->sortByDesc('number')
            ->first();

        // ── Charge les actions du round courant ──────────────────────────────
        $actions = collect();
        foreach ($game->rounds as $round) {
            $roundActions = $round->actions()
                ->with(['player', 'targetPlayer'])
                ->orderBy('created_at')
                ->get()
                ->map(fn ($action) => $this->formatAction($action));
            $actions = $actions->merge($roundActions);
        }

        $avatars = PlayerAvatar::urlsFor($game->binomes->flatMap(fn ($binome) => $binome->players));

        return response()->json([
            'game_id' => $game->id,
            'status' => $game->status,
            // Liste à plat : la composition des binomes n'est pas diffusée tant
            // qu'un binome n'est pas découvert. La renvoyer trahirait l'orphelin,
            // seul binome à ne compter qu'un joueur.
            'players' => $game->binomes
                ->flatMap(fn ($binome) => $binome->players)
                ->map(fn ($player) => [
                    'id' => $player->id,
                    'pseudo' => $player->pseudo,
                    'is_eliminated' => (bool) $player->pivot->is_eliminated,
                    'is_excluded' => (bool) $player->pivot->is_excluded,
                    'avatar_url' => $avatars[$player->id] ?? null,
                ])
                ->sortBy('id')
                ->values(),
            // Un orphelin est en jeu (nombre de joueurs impair) — sans dire qui.
            'has_orphan' => $game->hasOrphan(),
            // L'hôte décide pour un joueur déconnecté (passer son tour / l'exclure).
            'host_id' => $game->room?->created_by,
            'binomes' => $game->binomes
                ->filter(fn ($binome) => $binome->is_discovered)
                ->values()
                ->map(fn ($binome) => [
                    'id' => $binome->id,
                    'universe' => $binome->universe->name,
                    'cosmos' => $binome->universe->cosmos?->name,
                    'is_discovered' => true,
                    'players' => $binome->players->map(fn ($player) => [
                        'id' => $player->id,
                        'pseudo' => $player->pseudo,
                        'is_eliminated' => (bool) $player->pivot->is_eliminated,
                    ]),
                ]),
            'current_round' => $currentRound ? [
                'id' => $currentRound->id,
                'number' => $currentRound->number,
                'current_player_id' => $currentRound->current_player_id,
            ] : null,
            'actions' => $actions->values()->toArray(),
        ]);
    }

    private function formatAction(Action $action): array
    {
        $base = [
            'action_id' => $action->id,
            'id' => $action->id,
            'round_id' => $action->round_id,
            'round_number' => $action->round->number,
            'player' => [
                'id' => $action->player->id,
                'pseudo' => $action->player->pseudo,
            ],
            'type' => $action->type,
            'is_valid' => $action->is_valid,
            'answer' => $action->answer,
            'created_at' => $action->created_at->toIso8601String(),
        ];

        if ($action->type === ActionType::Question) {
            return array_merge($base, [
                'question' => $action->is_valid ? $action->content : null,
                'refused_reason' => ! $action->is_valid ? 'Mot interdit détecté.' : null,
                'target_player' => $action->targetPlayer ? [
                    'id' => $action->targetPlayer->id,
                    'pseudo' => $action->targetPlayer->pseudo,
                ] : null,
            ]);
        }

        // Le nom proposé est public : ActionPlayed le diffuse déjà à tout le
        // channel. accusation_confirmed permet au front de distinguer une
        // accusation en attente (null) d'une accusation niée (false) après un
        // rechargement, et de rouvrir la modale de confirmation chez la cible.
        return array_merge($base, [
            'target_player' => $action->targetPlayer ? [
                'id' => $action->targetPlayer->id,
                'pseudo' => $action->targetPlayer->pseudo,
            ] : null,
            'accusation_correct' => $action->accusation_correct,
            'accusation_confirmed' => $action->accusation_confirmed,
            'character_name' => $action->character_name ?? $action->content,
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

        $player = Player::findOrFail($request->input('player_id'));
        $character = $player->getCharacterInGame($game);

        if (! $character) {
            return response()->json([
                'message' => 'Aucun personnage trouvé pour ce joueur dans cette partie.',
            ], 404);
        }

        return response()->json([
            'character' => [
                'id' => $character->id,
                'name' => $character->name,
                'image' => $character->image,
                'universe' => $character->universe->name,
                'cosmos' => $character->universe->cosmos?->name,
                'forbidden_words' => $character->forbidden_words,
            ],
        ]);
    }

    /**
     * GET /games/{game}/recap
     * Récap de fin de partie (scores, binomes révélés, trophées). Tout y est
     * révélé : uniquement pour une partie terminée, comme l'historique public.
     */
    public function recap(Game $game): JsonResponse
    {
        if ($game->status !== GameStatus::Finished) {
            return response()->json([
                'message' => "La partie n'est pas terminée.",
            ], 409);
        }

        return response()->json($this->recapService->build($game));
    }

    /**
     * POST /games/{game}/reactions
     * Réaction rapide (emoji) diffusée à tout le channel, spectateurs compris.
     */
    public function react(Request $request, Game $game): JsonResponse
    {
        $validated = $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'emoji' => ['required', 'string', 'in:'.implode(',', self::REACTIONS)],
        ]);

        if (! $game->canBeWatchedBy((int) $validated['player_id'])) {
            return response()->json([
                'message' => 'Tu ne participes pas à cette partie.',
            ], 403);
        }

        broadcast(new ReactionSent($game, Player::find($validated['player_id']), $validated['emoji']));

        return response()->json(['message' => 'Réaction envoyée.']);
    }

    /**
     * POST /games/{game}/skip-turn
     * L'hôte passe le tour du joueur déconnecté qui met la partie en pause
     * (voir ActionService::skipBlockedTurn).
     */
    public function skipTurn(Request $request, Game $game): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
        ]);

        try {
            $result = $this->actionService->skipBlockedTurn(
                $game,
                Player::findOrFail($request->input('player_id')),
            );
        } catch (\RuntimeException $e) {
            return response()->json(['message' => $e->getMessage()], 409);
        }

        return response()->json([
            'message' => "Tour de {$result['player']->pseudo} passé.",
            'reason' => $result['reason'],
        ]);
    }

    /**
     * POST /games/{game}/players/{player}/exclude
     * L'hôte exclut un joueur déconnecté : lui et son binôme sont éliminés
     * (voir ActionService::excludePlayer).
     */
    public function excludePlayer(Request $request, Game $game, Player $player): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
        ]);

        try {
            $partner = $this->actionService->excludePlayer(
                $game,
                Player::findOrFail($request->input('player_id')),
                $player,
            );
        } catch (\RuntimeException $e) {
            return response()->json(['message' => $e->getMessage()], 409);
        }

        return response()->json([
            'message' => $partner
                ? "{$player->pseudo} est exclu, son binôme {$partner->pseudo} est éliminé."
                : "{$player->pseudo} est exclu.",
        ]);
    }
}
