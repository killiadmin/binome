<?php

namespace App\Http\Controllers;

use App\Enums\GameMode;
use App\Http\Requests\JoinRoomRequest;
use App\Http\Requests\StoreRoomRequest;
use App\Models\Player;
use App\Models\Room;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Str;

class RoomController extends Controller
{
    /**
     * POST /rooms
     * Crée un nouveau salon
     */
    public function store(StoreRoomRequest $request): JsonResponse
    {
        $player = Player::firstOrCreate(
            ['pseudo' => $request->validated('pseudo')]
        );

        $room = Room::create([
            'code' => $this->generateUniqueCode(),
            'is_private' => $request->validated('is_private', false),
            'max_players' => $request->validated('max_players', 6),
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
            ],
        ], 201);
    }

    /**
     * POST /rooms/join
     * Rejoindre un salon avec un code
     */
    public function join(JoinRoomRequest $request): JsonResponse
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
                'players' => $room->players()->get()->map(fn ($p) => [
                    'id' => $p->id,
                    'pseudo' => $p->pseudo,
                    'is_ready' => $p->pivot->is_ready,
                ]),
            ],
            'player' => [
                'id' => $player->id,
                'pseudo' => $player->pseudo,
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
            'players' => $room->players()->get()->map(fn ($p) => [
                'id' => $p->id,
                'pseudo' => $p->pseudo,
                'is_ready' => $p->pivot->is_ready,
            ]),
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
        ]);

        $room->load('cosmos');

        broadcast(new \App\Events\RoomSettingsUpdated($room));

        return response()->json([
            'message' => 'Configuration mise à jour.',
            'game_mode' => $room->game_mode,
            'cosmos_id' => $room->cosmos_id,
            'cosmos_name' => $room->cosmos?->name,
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
                'created_by' => $room->created_by,
                'players' => $room->players()->get()->map(fn ($p) => [
                    'id' => $p->id,
                    'pseudo' => $p->pseudo,
                    'is_ready' => $p->pivot->is_ready,
                ]),
            ],
        ]);
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

        $room->players()->detach($playerId);

        Player::find($playerId)?->delete();

        if ($room->players()->count() === 0) {
            $room->delete();

            return response()->json([
                'message' => 'Salon supprimé car vide.',
            ]);
        }

        if ($room->created_by === $playerId) {
            $newHost = $room->players()->first();
            $room->update(['created_by' => $newHost->id]);

            broadcast(new \App\Events\PlayerLeft($room->load('players'), $newHost));
        } else {
            broadcast(new \App\Events\PlayerLeft($room->load('players'), null));
        }

        return response()->json([
            'message' => 'Tu as quitté le salon.',
        ]);
    }
}
