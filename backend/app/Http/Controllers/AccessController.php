<?php

namespace App\Http\Controllers;

use App\Http\Middleware\EnsureCharactersAccess;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;

class AccessController extends Controller
{
    /**
     * POST /access/characters
     * Vérifie le mot de passe d'accès à la gestion des personnages et renvoie
     * le token à stocker côté client (envoyé ensuite via l'en-tête
     * X-Characters-Access sur les routes protégées).
     */
    public function characters(Request $request): JsonResponse
    {
        $validated = $request->validate([
            'password' => ['required', 'string'],
        ]);

        $password = config('access.characters_password');

        if (blank($password) || ! hash_equals((string) $password, $validated['password'])) {
            return response()->json(['message' => 'Mot de passe incorrect.'], 422);
        }

        return response()->json([
            'token' => EnsureCharactersAccess::token($password),
        ]);
    }
}
