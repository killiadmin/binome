<?php

namespace App\Providers;

use Illuminate\Cache\RateLimiting\Limit;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\RateLimiter;
use Illuminate\Support\Facades\Route;
use Illuminate\Support\ServiceProvider;

class AppServiceProvider extends ServiceProvider
{
    public function register(): void {}

    public function boot(): void
    {
        Route::middleware('web')
            ->post('/broadcasting/auth', [
                \App\Http\Controllers\BroadcastAuthController::class,
                'authenticate',
            ]);

        $this->configureRateLimiting();

        require base_path('routes/channels.php');
    }

    /**
     * Anti-spam du chat commun.
     *
     * Le quota est indexé sur l'identité de l'auteur, pas sur l'IP : le proxy du
     * serveur Vite ne transmet pas l'IP du client (pas de `xfwd`, pas de
     * TrustProxies), donc tous les joueurs du réseau local arrivent ici avec
     * l'IP du conteneur frontend et se partageraient un seul compteur.
     */
    private function configureRateLimiting(): void
    {
        RateLimiter::for('chat', function (Request $request) {
            $key = $request->input('player_id')
                ? 'p:'.$request->input('player_id')
                : 'a:'.($request->input('anon_id') ?: $request->ip());

            return Limit::perMinute(20)->by($key);
        });

        // Réactions rapides en partie : assez large pour réagir à chaud, assez
        // serré pour qu'un doigt posé sur un emoji n'inonde pas les écrans.
        // Même raison que le chat pour indexer sur le joueur plutôt que l'IP.
        RateLimiter::for('reactions', function (Request $request) {
            return Limit::perMinute(30)->by('p:'.$request->input('player_id'));
        });

        // Photos de profil : quelques essais par minute suffisent. Le throttle
        // passe avant la liaison de modèle : {player} est encore l'id brut.
        RateLimiter::for('avatars', function (Request $request) {
            $player = $request->route('player');

            return Limit::perMinute(10)->by('p:'.(is_object($player) ? $player->id : $player));
        });
    }
}
