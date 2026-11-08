<?php

namespace App\Http\Controllers;

use App\Models\Character;
use Illuminate\Http\Response;
use Illuminate\Routing\Controller;

/**
 * Image d'un personnage, servie en binaire avec un cache long (URL versionnée,
 * voir Character::imageUrl()). Route ouverte comme /api/avatars : l'URL n'est
 * donnée qu'au joueur concerné par GET /games/{game}/me, et l'id seul ne dit
 * pas qui joue ce personnage.
 */
class CharacterImageController extends Controller
{
    /** Types acceptés à l'upload / l'import (CharacterController, CharacterImporter). */
    private const MIME_TYPES = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];

    public function show(Character $character): Response
    {
        // data:image/jpeg;base64,....
        abort_unless(preg_match('#^data:(image/[a-z]+);base64,#', (string) $character->image, $m), 404);
        abort_unless(in_array($m[1], self::MIME_TYPES, true), 404);

        $bytes = base64_decode(substr($character->image, strlen($m[0])), true);

        abort_if($bytes === false, 404);

        return response($bytes, 200, [
            'Content-Type' => $m[1],
            'Cache-Control' => 'public, max-age=31536000, immutable',
            'X-Content-Type-Options' => 'nosniff',
        ]);
    }
}
