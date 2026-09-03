<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class EnsureCharactersAccess
{
    /**
     * Refuse l'accès aux routes de gestion des personnages tant que le client
     * ne présente pas un token dérivé du mot de passe défini dans la config.
     */
    public function handle(Request $request, Closure $next): Response
    {
        $password = config('access.characters_password');

        if (blank($password)) {
            return response()->json([
                'message' => "L'accès à la gestion des personnages n'est pas configuré.",
            ], 403);
        }

        $provided = $request->header('X-Characters-Access');

        if (! is_string($provided) || ! hash_equals(self::token($password), $provided)) {
            return response()->json([
                'message' => 'Mot de passe requis pour accéder à la gestion des personnages.',
            ], 403);
        }

        return $next($request);
    }

    /**
     * Token stable (non réversible) dérivé du mot de passe, échangé avec le front.
     */
    public static function token(string $password): string
    {
        return hash_hmac('sha256', 'binome-characters-access', $password);
    }
}
