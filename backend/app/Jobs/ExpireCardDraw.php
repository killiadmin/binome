<?php

namespace App\Jobs;

use App\Services\ActionService;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;

/**
 * Délai du tirage de carte dépassé : la carte est perdue et le tour avance.
 * Dispatché avec un délai à l'ouverture du tirage ; sans effet si le
 * questionneur a déjà décidé (mise à jour conditionnelle).
 */
class ExpireCardDraw implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable;

    public function __construct(public readonly int $actionId) {}

    public function handle(ActionService $actionService): void
    {
        $actionService->expireCardDraw($this->actionId);
    }
}
