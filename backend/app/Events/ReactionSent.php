<?php

namespace App\Events;

use App\Models\CustomReaction;
use App\Models\Game;
use App\Models\Player;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;

/**
 * Réaction rapide envoyée pendant une partie. Rien n'est stocké : l'event
 * n'existe que pour l'animation chez les autres joueurs. `reaction` porte
 * soit `emoji` (Unicode), soit `url` (image — jamais l'image elle-même).
 */
class ReactionSent implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets;

    public function __construct(
        public readonly Game $game,
        public readonly Player $player,
        public readonly CustomReaction $reaction,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->game->id}")];
    }

    public function broadcastAs(): string
    {
        return 'reaction.sent';
    }

    public function broadcastWith(): array
    {
        return [
            'player' => [
                'id' => $this->player->id,
                'pseudo' => $this->player->pseudo,
            ],
            'reaction' => $this->reaction->toPublicArray(),
        ];
    }
}
