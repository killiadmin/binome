<?php

namespace App\Http\Controllers;

use App\Enums\GameMode;
use App\Http\Requests\JoinRoomRequest;
use App\Http\Requests\StoreRoomRequest;
use App\Models\Player;
use App\Models\PlayerAvatar;
use App\Models\Room;
use App\Services\PresenceService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

class RoomController extends Controller
{
    /**
     * POST /rooms
     * Crée un nouveau salon
     */
    public function store(StoreRoomRequest $request, PresenceService $presence): JsonResponse
    {
        if ($conflict = $this->pseudoInUse($request->validated('pseudo'), $presence)) {
            return $conflict;
        }

        $player = Player::firstOrCreate(
            ['pseudo' => $request->validated('pseudo')]
        );

        $room = Room::create([
            'code' => $this->generateUniqueCode(),
            'is_private' => $request->validated('is_private', false),
            'max_players' => $request->validated('max_players', 12),
            'created_by' => $player->id,
        ]);

        // Le créateur rejoint automatiquement son salon et est ready
        $room->players()->attach($player->id, ['is_ready' => false]);

        return response()->json([
            'message' => 'Salon créé avec succès.',
            'room' => [
                'id' => $room->id,
                'code' => $room->code,
                'is_private' => $room->is_private,
                'max_players' => $room->max_players,
            ],
            'player' => [
                'id' => $player->id,
                'pseudo' => $player->pseudo,
                // Photo déjà enregistrée sous ce pseudo lors d'une partie précédente
                'avatar_url' => PlayerAvatar::urlsFor([$player])[$player->id],
            ],
        ], 201);
    }

    /**
     * POST /rooms/join
     * Rejoindre un salon avec un code
     */
    public function join(JoinRoomRequest $request, PresenceService $presence): JsonResponse
    {
        $room = Room::where('code', $request->validated('code'))->firstOrFail();

        if ($room->is_locked) {
            return response()->json([
                'message' => 'Ce salon est verrouillé.',
            ], 403);
        }

        if ($room->players()->count() >= $room->max_players) {
            return response()->json([
                'message' => 'Ce salon est complet.',
            ], 409);
        }

        if ($conflict = $this->pseudoInUse($request->validated('pseudo'), $presence)) {
            return $conflict;
        }

        $player = Player::firstOrCreate(
            ['pseudo' => $request->validated('pseudo')]
        );

        if ($room->players()->where('player_id', $player->id)->exists()) {
            return response()->json([
                'message' => 'Tu es déjà dans ce salon.',
            ], 409);
        }

        $room->players()->attach($player->id, ['is_ready' => false]);

        broadcast(new \App\Events\PlayerJoined(
            $room->load('players'),
            $player
        ));

        $room->load('cosmos');

        return response()->json([
            'message' => 'Tu as rejoint le salon.',
            'room' => [
                'id' => $room->id,
                'code' => $room->code,
                'game_mode' => $room->game_mode,
                'cosmos_id' => $room->cosmos_id,
                'cosmos_name' => $room->cosmos?->name,
                'reveal_to_eliminated' => (bool) $room->reveal_to_eliminated,
                'best_question_vote' => (bool) $room->best_question_vote,
                // Partie déjà lancée : le nouveau venu n'a pas de personnage,
                // il peut la suivre en spectateur (le channel game.{id} accepte
                // les membres du salon).
                'current_game_id' => $room->currentGame()->value('id'),
                'players' => $room->playersPayload(),
            ],
            'player' => [
                'id' => $player->id,
                'pseudo' => $player->pseudo,
                // Photo déjà enregistrée sous ce pseudo lors d'une partie précédente
                'avatar_url' => PlayerAvatar::urlsFor([$player])[$player->id],
            ],
        ]);
    }

    /**
     * PATCH /rooms/{room}/ready
     * Le joueur indique qu'il est prêt
     */
    public function ready(Request $request, Room $room): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
        ]);

        $playerId = $request->input('player_id');

        if (! $room->players()->where('player_id', $playerId)->exists()) {
            return response()->json([
                'message' => 'Ce joueur n\'est pas dans ce salon.',
            ], 403);
        }

        $currentStatus = $room->players()
            ->where('player_id', $playerId)
            ->first()
            ->pivot
            ->is_ready;

        $newStatus = ! $currentStatus;

        $room->players()->updateExistingPivot($playerId, ['is_ready' => $newStatus]);

        $room->load('players');
        broadcast(new \App\Events\PlayerReady($room, Player::find($playerId)));

        $allReady = $room->players()->wherePivot('is_ready', false)->doesntExist();
        $enoughPlayers = $room->players()->count() >= 4;

        return response()->json([
            'message' => $newStatus ? 'Tu es prêt !' : 'Tu n\'es plus prêt.',
            'is_ready' => $newStatus,
            'all_ready' => $allReady,
            'can_start' => $allReady && $enoughPlayers,
            'players' => $room->playersPayload(),
        ]);
    }

    /**
     * PATCH /rooms/{room}/settings
     * L'hôte configure le mode de jeu (aléatoire / cosmos imposé)
     */
    public function updateSettings(Request $request, Room $room): JsonResponse
    {
        $validated = $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'game_mode' => ['required', 'string', 'in:'.implode(',', array_column(GameMode::cases(), 'value'))],
            'cosmos_id' => ['nullable', 'required_if:game_mode,cosmos', 'integer', 'exists:cosmos,id'],
            'reveal_to_eliminated' => ['sometimes', 'boolean'],
            'best_question_vote' => ['sometimes', 'boolean'],
        ]);

        if ($room->created_by !== (int) $validated['player_id']) {
            return response()->json([
                'message' => 'Seul le créateur du salon peut configurer la partie.',
            ], 403);
        }

        $room->update([
            'game_mode' => $validated['game_mode'],
            'cosmos_id' => $validated['game_mode'] === GameMode::Cosmos->value
                ? $validated['cosmos_id']
                : null,
            'reveal_to_eliminated' => $validated['reveal_to_eliminated'] ?? $room->reveal_to_eliminated,
            'best_question_vote' => $validated['best_question_vote'] ?? $room->best_question_vote,
        ]);

        $room->load('cosmos');

        broadcast(new \App\Events\RoomSettingsUpdated($room));

        return response()->json([
            'message' => 'Configuration mise à jour.',
            'game_mode' => $room->game_mode,
            'cosmos_id' => $room->cosmos_id,
            'cosmos_name' => $room->cosmos?->name,
            'reveal_to_eliminated' => $room->reveal_to_eliminated,
            'best_question_vote' => $room->best_question_vote,
        ]);
    }

    /**
     * GET /rooms/{room}
     * État du salon (liste des joueurs, statut ready)
     */
    public function show(Room $room): JsonResponse
    {
        $room->load('cosmos');

        return response()->json([
            'room' => [
                'id' => $room->id,
                'code' => $room->code,
                'is_locked' => $room->is_locked,
                'max_players' => $room->max_players,
                'game_mode' => $room->game_mode,
                'cosmos_id' => $room->cosmos_id,
                'cosmos_name' => $room->cosmos?->name,
                'reveal_to_eliminated' => (bool) $room->reveal_to_eliminated,
                'best_question_vote' => (bool) $room->best_question_vote,
                'created_by' => $room->created_by,
                'current_game_id' => $room->currentGame()->value('id'),
                'players' => $room->playersPayload(),
            ],
        ]);
    }

    /**
     * Un pseudo = un joueur (firstOrCreate) : le reprendre sur un second appareil
     * pendant qu'il est connecté ailleurs mélangerait les deux sessions. On
     * refuse avec le salon concerné pour que l'utilisateur comprenne pourquoi.
     */
    private function pseudoInUse(string $pseudo, PresenceService $presence): ?JsonResponse
    {
        $player = Player::where('pseudo', $pseudo)->first();
        $room = $player ? $presence->connectedRoomFor($player) : null;

        if (! $room) {
            return null;
        }

        return response()->json([
            'message' => "Le pseudo « {$player->pseudo} » est déjà connecté sur un autre appareil "
                ."(salon {$room->code}). Choisis un autre pseudo, ou quitte ce salon sur l'autre appareil.",
        ], 409);
    }

    private function generateUniqueCode(): string
    {
        do {
            $code = strtoupper(Str::random(6));
        } while (Room::where('code', $code)->exists());

        return $code;
    }

    public function leave(Request $request, Room $room): JsonResponse
    {
        $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
        ]);

        $playerId = $request->input('player_id');

        if (! $room->players()->where('player_id', $playerId)->exists()) {
            return response()->json([
                'message' => 'Ce joueur n\'est pas dans ce salon.',
            ], 403);
        }

        // Un joueur de la partie en cours ne peut pas la quitter : il emporterait
        // son personnage (binome vidé, vote bloqué, fin de partie faussée).
        // S'il est parti pour de bon, l'hôte l'exclut depuis la partie.
        if ($room->currentGame()->first()?->hasPlayer((int) $playerId)) {
            return response()->json([
                'message' => 'Une partie est en cours : tu ne peux pas quitter le salon avant la fin. Si tu dois partir, l\'hôte peut t\'exclure.',
            ], 409);
        }

        $roomDeleted = $room->players()->count() === 1;
        $room->removePlayer((int) $playerId);

        // Le joueur n'est supprimé que s'il n'a jamais joué : ses parties
        // passées (récap, journal d'actions) pointent encore sur lui. Le pseudo
        // reste réservé à cette ligne, que firstOrCreate() réutilisera.
        // Après removePlayer() : l'hôte a déjà été transféré (cascade created_by).
        $hasPlayed = DB::table('binome_player')->where('player_id', $playerId)->exists()
            || DB::table('actions')->where('player_id', $playerId)->exists();

        if (! $hasPlayed) {
            Player::find($playerId)?->delete();
        }

        if ($roomDeleted) {
            return response()->json([
                'message' => 'Salon supprimé car vide.',
            ]);
        }

        return response()->json([
            'message' => 'Tu as quitté le salon.',
        ]);
    }
}
