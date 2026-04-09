<?php

namespace App\Http\Controllers;

use App\Models\Player;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Facades\Broadcast;

class BroadcastAuthController extends Controller
{
    public function authenticate(Request $request)
    {
        $playerId = $request->header('X-Player-Id');

        if (!$playerId) {
            return response()->json(['error' => 'X-Player-Id header manquant'], 403);
        }

        $player = Player::find($playerId);

        if (!$player) {
            return response()->json(['error' => 'Joueur introuvable'], 403);
        }

        auth()->setUser($player);

        return Broadcast::auth($request);
    }
}
