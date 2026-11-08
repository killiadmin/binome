<?php

namespace App\Events;

use App\Models\Action;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PrivateChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;

/**
 * Le questionneur montre le mot tiré à tout le monde SAUF la cible.
 * Diffusé sur le channel privé player.{id} de chaque destinataire : le
 * channel game.{id} atteint aussi la cible, qui lirait le mot dans ses
 * trames WebSocket.
 */
class CardWordShared implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets;

    public const REVEAL_SECONDS = 5;

    /**
     * @param  int[]  $recipientIds
     */
    public function __construct(
        public readonly Action $action,
        public readonly array $recipientIds,
    ) {}

    public function broadcastOn(): array
    {
        return array_map(fn (int $id) => new PrivateChannel("player.{$id}"), $this->recipientIds);
    }

    public function broadcastAs(): string
    {
        return 'card.word.shared';
    }

    public function broadcastWith(): array
    {
        return [
            'action_id' => $this->action->id,
            'word' => $this->action->drawn_word,
            'reveal_seconds' => self::REVEAL_SECONDS,
            'asker' => [
                'id' => $this->action->player->id,
                'pseudo' => $this->action->player->pseudo,
            ],
            'target' => [
                'id' => $this->action->targetPlayer->id,
                'pseudo' => $this->action->targetPlayer->pseudo,
            ],
        ];
    }
}
