<?php

namespace App\Http\Controllers;

use App\Events\PlayerAvatarUpdated;
use App\Models\Player;
use App\Models\PlayerAvatar;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;
use Illuminate\Routing\Controller;

/**
 * Photo de profil des joueurs (remplace le badge à initiales).
 *
 * Le navigateur recadre la photo en carré et la réencode en JPEG avant
 * l'envoi : l'extension GD n'est pas disponible dans le conteneur, le serveur
 * se contente donc de vérifier ce qu'il reçoit (format, taille, dimensions).
 * Le réencodage canvas supprime au passage les métadonnées EXIF (position GPS).
 */
class AvatarController extends Controller
{
    /** Taille maximale de l'image décodée. Une photo 256 px JPEG pèse ~20 ko. */
    private const MAX_BYTES = 200 * 1024;

    private const MIN_SIDE = 64;

    private const MAX_SIDE = 512;

    /**
     * GET /avatars/{avatar}
     * L'image elle-même. L'URL est versionnée (?v=updated_at) : cache long.
     */
    public function show(PlayerAvatar $avatar): Response
    {
        $bytes = base64_decode(substr($avatar->image, strpos($avatar->image, ',') + 1), true);

        abort_if($bytes === false, 404);

        return response($bytes, 200, [
            'Content-Type' => 'image/jpeg',
            'Cache-Control' => 'public, max-age=31536000, immutable',
            'X-Content-Type-Options' => 'nosniff',
        ]);
    }

    /**
     * POST /players/{player}/avatar
     * Body : { player_id, image } — image = data URI JPEG carrée.
     */
    public function update(Request $request, Player $player): JsonResponse
    {
        $validated = $request->validate([
            'player_id' => ['required', 'integer'],
            'image' => ['required', 'string', 'max:300000', 'starts_with:data:image/jpeg;base64,'],
        ]);

        if ((int) $validated['player_id'] !== $player->id) {
            return response()->json(['message' => 'Tu ne peux modifier que ta propre photo.'], 403);
        }

        $bytes = base64_decode(substr($validated['image'], strlen('data:image/jpeg;base64,')), true);
        $info = $bytes !== false ? @getimagesizefromstring($bytes) : false;

        if ($bytes === false || strlen($bytes) > self::MAX_BYTES || ! $info || $info[2] !== IMAGETYPE_JPEG) {
            return $this->invalid('La photo doit être une image JPEG de moins de 200 ko.');
        }

        [$width, $height] = $info;

        if ($width !== $height || $width < self::MIN_SIDE || $width > self::MAX_SIDE) {
            return $this->invalid('La photo doit être carrée, entre 64 et 512 pixels de côté.');
        }

        $avatar = PlayerAvatar::updateOrCreate(
            ['pseudo' => $player->pseudo],
            ['image' => 'data:image/jpeg;base64,'.base64_encode($bytes)],
        );
        // Force une nouvelle version d'URL même si l'image est identique.
        $avatar->touch();

        broadcast(new PlayerAvatarUpdated($player, $avatar->url()));

        return response()->json([
            'message' => 'Photo enregistrée.',
            'avatar_url' => $avatar->url(),
        ]);
    }

    /**
     * DELETE /players/{player}/avatar
     * Retour au badge à initiales.
     */
    public function destroy(Request $request, Player $player): JsonResponse
    {
        $request->validate(['player_id' => ['required', 'integer']]);

        if ((int) $request->input('player_id') !== $player->id) {
            return response()->json(['message' => 'Tu ne peux modifier que ta propre photo.'], 403);
        }

        PlayerAvatar::where('pseudo', $player->pseudo)->delete();

        broadcast(new PlayerAvatarUpdated($player, null));

        return response()->json(['message' => 'Photo supprimée.', 'avatar_url' => null]);
    }

    private function invalid(string $message): JsonResponse
    {
        return response()->json([
            'message' => $message,
            'errors' => ['image' => [$message]],
        ], 422);
    }
}
