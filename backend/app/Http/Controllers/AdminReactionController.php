<?php

namespace App\Http\Controllers;

use App\Models\CustomReaction;
use App\Support\ReactionImage;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

/**
 * Gestion de toutes les réactions (emojis Unicode et images) — même mot de
 * passe que la gestion des personnages (middleware characters.access).
 */
class AdminReactionController extends Controller
{
    /**
     * GET /admin/reactions
     * Toutes les réactions, masquées comprises, dans l'ordre de la barre.
     */
    public function index(): JsonResponse
    {
        return response()->json([
            'reactions' => CustomReaction::barOrder()
                ->get(['id', 'name', 'emoji', 'created_by_pseudo', 'is_enabled', 'created_at', 'updated_at'])
                ->map(fn (CustomReaction $reaction) => $this->format($reaction)),
        ]);
    }

    /**
     * POST /admin/reactions
     * Body : { name, emoji } (emoji Unicode) ou { name, image } (data URI JPEG carrée).
     */
    public function store(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'name' => ['required', 'string', 'max:20'],
            'emoji' => ['required_without:image', 'nullable', 'string', 'max:32'],
            'image' => ['required_without:emoji', 'nullable', ...ReactionImage::RULES],
        ]);

        if (! empty($validated['emoji'])) {
            $emoji = trim($validated['emoji']);

            // Un seul graphème, sans lettre ni chiffre : « 🎉 » ou « 👍🏽 », pas « ok ».
            if (! preg_match('/^\X$/u', $emoji) || preg_match('/[\p{L}\p{N}]/u', $emoji)) {
                return $this->invalid('Saisis un seul emoji.', 'emoji');
            }

            if (CustomReaction::where('emoji', $emoji)->exists()) {
                return $this->invalid('Cet emoji existe déjà.', 'emoji');
            }

            $data = ['emoji' => $emoji];
        } else {
            if ($error = ReactionImage::error($validated['image'])) {
                return $this->invalid($error, 'image');
            }

            $data = ['image' => $validated['image'], 'created_by_pseudo' => 'Admin'];
        }

        $reaction = CustomReaction::create(['name' => trim($validated['name']), ...$data]);

        return response()->json([
            'message' => 'Emoji ajouté.',
            'reaction' => $this->format($reaction->fresh()),
        ], 201);
    }

    /**
     * PATCH /admin/reactions/{reaction}
     * Body : { name?, is_enabled? } — masquer retire l'emoji de la barre des joueurs.
     */
    public function update(Request $request, CustomReaction $reaction): JsonResponse
    {
        $validated = $request->validate([
            'name' => ['sometimes', 'string', 'max:20'],
            'is_enabled' => ['sometimes', 'boolean'],
        ]);

        $reaction->update($validated);

        return response()->json([
            'message' => 'Emoji mis à jour.',
            'reaction' => $this->format($reaction),
        ]);
    }

    /**
     * DELETE /admin/reactions/{reaction}
     */
    public function destroy(CustomReaction $reaction): JsonResponse
    {
        $reaction->delete();

        return response()->json(['message' => 'Emoji supprimé.']);
    }

    private function format(CustomReaction $reaction): array
    {
        return [
            ...$reaction->toPublicArray(),
            'is_enabled' => $reaction->is_enabled,
            'created_at' => $reaction->created_at,
        ];
    }

    private function invalid(string $message, string $field): JsonResponse
    {
        return response()->json([
            'message' => $message,
            'errors' => [$field => [$message]],
        ], 422);
    }
}
