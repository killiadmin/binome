<?php

namespace App\Http\Controllers;

use App\Http\Requests\StoreRoomRequest;
use App\Http\Requests\JoinRoomRequest;
use App\Models\Room;
use App\Models\Player;
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
            'code'        => $this->generateUniqueCode(),
            'is_private'  => $request->validated('is_private', false),
            'max_players' => $request->validated('max_players', 6),
            'created_by'  => $player->id,
        ]);

        // Le créateur rejoint automatiquement son salon et est ready
        $room->players()->attach($player->id, ['is_ready' => false]);

        return response()->json([
            'message'  => 'Salon créé avec succès.',
            'room'     => [
                'id'         => $room->id,
                'code'       => $room->code,
                'is_private' => $room->is_private,
                'max_players'=> $room->max_players,
            ],
            'player'   => [
                'id'     => $player->id,
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

        // Vérifie que le joueur n'est pas déjà dans le salon
        if ($room->players()->where('player_id', $player->id)->exists()) {
            return response()->json([
                'message' => 'Tu es déjà dans ce salon.',
            ], 409);
        }

        $room->players()->attach($player->id, ['is_ready' => false]);

        return response()->json([
            'message' => 'Tu as rejoint le salon.',
            'room'    => [
                'id'          => $room->id,
                'code'        => $room->code,
                'players'     => $room->players()->get()->map(fn($p) => [
                    'id'       => $p->id,
                    'pseudo'   => $p->pseudo,
                    'is_ready' => $p->pivot->is_ready,
                ]),
            ],
            'player'  => [
                'id'     => $player->id,
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

        if (!$room->players()->where('player_id', $playerId)->exists()) {
            return response()->json([
                'message' => 'Ce joueur n\'est pas dans ce salon.',
            ], 403);
        }

        $room->players()->updateExistingPivot($playerId, ['is_ready' => true]);

        $allReady   = $room->players()->wherePivot('is_ready', false)->doesntExist();
        $enoughPlayers = $room->players()->count() >= 4;

        return response()->json([
            'message'        => 'Statut mis à jour.',
            'all_ready'      => $allReady,
            'can_start'      => $allReady && $enoughPlayers,
            'players'        => $room->players()->get()->map(fn($p) => [
                'id'       => $p->id,
                'pseudo'   => $p->pseudo,
                'is_ready' => $p->pivot->is_ready,
            ]),
        ]);
    }

    /**
     * GET /rooms/{room}
     * État du salon (liste des joueurs, statut ready)
     */
    public function show(Room $room): JsonResponse
    {
        return response()->json([
            'room' => [
                'id'          => $room->id,
                'code'        => $room->code,
                'is_locked'   => $room->is_locked,
                'max_players' => $room->max_players,
                'created_by'  => $room->created_by,
                'players'     => $room->players()->get()->map(fn($p) => [
                    'id'       => $p->id,
                    'pseudo'   => $p->pseudo,
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
}
