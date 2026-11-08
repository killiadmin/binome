<?php

namespace App\Http\Controllers;

use App\Models\CustomReaction;
use App\Models\Player;
use App\Support\ReactionImage;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;
use Illuminate\Routing\Controller;

/**
 * Réactions utilisables en partie (POST /games/{game}/reactions avec
 * reaction_id) : emojis Unicode et images créées par les joueurs. Une image
 * créée est aussitôt disponible ; l'admin peut la masquer ou la supprimer.
 */
class CustomReactionController extends Controller
{
    /** Emojis actifs par pseudo : au-delà, la barre de réactions devient illisible. */
    private const MAX_PER_PSEUDO = 10;

    /**
     * GET /reactions
     * Réactions actives, dans l'ordre de la barre.
     */
    public function index(): JsonResponse
    {
        return response()->json([
            'reactions' => CustomReaction::enabled()
                ->barOrder()
                ->get(['id', 'name', 'emoji', 'created_by_pseudo', 'updated_at'])
                ->map->toPublicArray(),
        ]);
    }

    /**
     * GET /reactions/{reaction}/image
     * Servie même si l'emoji est masqué : une réaction déjà envoyée garde son image.
     */
    public function image(CustomReaction $reaction): Response
    {
        abort_if($reaction->image === null, 404);

        $bytes = base64_decode(substr($reaction->image, strpos($reaction->image, ',') + 1), true);

        abort_if($bytes === false, 404);

        return response($bytes, 200, [
            'Content-Type' => 'image/jpeg',
            'Cache-Control' => 'public, max-age=31536000, immutable',
            'X-Content-Type-Options' => 'nosniff',
        ]);
    }

    /**
     * POST /reactions
     * Body : { player_id, name, image } — image = data URI JPEG carrée.
     */
    public function store(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'player_id' => ['required', 'integer', 'exists:players,id'],
            'name' => ['required', 'string', 'max:20'],
            'image' => ['required', ...ReactionImage::RULES],
        ]);

        $player = Player::findOrFail($validated['player_id']);

        if (CustomReaction::enabled()->where('created_by_pseudo', $player->pseudo)->count() >= self::MAX_PER_PSEUDO) {
            return $this->invalid('Tu as déjà créé '.self::MAX_PER_PSEUDO.' emojis.', 'name');
        }

        if ($error = ReactionImage::error($validated['image'])) {
            return $this->invalid($error);
        }

        $reaction = CustomReaction::create([
            'name' => trim($validated['name']),
            'image' => $validated['image'],
            'created_by_pseudo' => $player->pseudo,
        ]);

        return response()->json([
            'message' => 'Emoji créé.',
            'reaction' => $reaction->toPublicArray(),
        ], 201);
    }

    private function invalid(string $message, string $field = 'image'): JsonResponse
    {
        return response()->json([
            'message' => $message,
            'errors' => [$field => [$message]],
        ], 422);
    }
}
