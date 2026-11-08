<?php

namespace App\Events;

use App\Models\Action;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;

/**
 * Tirage clos (mot montré, gardé, ou délai dépassé) : le tour reprend.
 * Volontairement muet sur l'issue — la cible ne doit pas savoir si son mot
 * a été montré aux autres.
 */
class CardDrawFinished implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets;

    public function __construct(
        public readonly Action $action,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->action->round->game_id}")];
    }

    public function broadcastAs(): string
    {
        return 'card.draw.finished';
    }

    public function broadcastWith(): array
    {
        return [
            'action_id' => $this->action->id,
            'asker' => [
                'id' => $this->action->player->id,
                'pseudo' => $this->action->player->pseudo,
            ],
        ];
    }
}
