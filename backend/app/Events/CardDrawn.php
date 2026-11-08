<?php

namespace App\Events;

use App\Models\Action;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;

/**
 * Le questionneur a retourné une carte (tirage après un « Je ne sais pas »).
 * Public : dit QUELLE carte a été choisie, jamais le mot — celui-ci ne part
 * que dans la réponse HTTP du tirage.
 */
class CardDrawn implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets;

    public function __construct(
        public readonly Action $action,
        public readonly int $card,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->action->round->game_id}")];
    }

    public function broadcastAs(): string
    {
        return 'card.drawn';
    }

    public function broadcastWith(): array
    {
        return [
            'action_id' => $this->action->id,
            'card' => $this->card,
            'asker' => [
                'id' => $this->action->player->id,
                'pseudo' => $this->action->player->pseudo,
            ],
        ];
    }
}
