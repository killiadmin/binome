<?php

namespace App\Services;

use Illuminate\Support\Facades\Broadcast;
use Illuminate\Support\Facades\Log;
use Pusher\ApiErrorException;

/**
 * Qui est réellement connecté à un PresenceChannel, vu par Reverb lui-même
 * (API HTTP compatible Pusher : GET /channels/{channel}/users).
 *
 * Sert à valider côté serveur qu'un joueur est hors ligne avant de laisser
 * les autres passer son tour : le front seul ne suffit pas, n'importe quel
 * client pourrait appeler l'endpoint.
 */
class PresenceService
{
    /**
     * Ids des joueurs connectés au channel, ou null si Reverb ne répond pas.
     *
     * @return int[]|null
     */
    public function onlinePlayerIds(string $presenceChannel): ?array
    {
        try {
            $response = Broadcast::connection()
                ->getPusher()
                ->get("/channels/presence-{$presenceChannel}/users");

            return collect($response->users ?? [])
                ->map(fn ($user) => (int) $user->id)
                ->all();
        } catch (ApiErrorException $e) {
            // Reverb répond 404 pour un channel sans aucun abonné : personne en ligne.
            if ($e->getCode() === 404) {
                return [];
            }

            return $this->unavailable($presenceChannel, $e);
        } catch (\Throwable $e) {
            return $this->unavailable($presenceChannel, $e);
        }
    }

    private function unavailable(string $presenceChannel, \Throwable $e): ?array
    {
        Log::warning('Présence Reverb indisponible', [
            'channel' => $presenceChannel,
            'error' => $e->getMessage(),
        ]);

        return null;
    }
}
