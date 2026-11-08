<?php

namespace App\Events;

use App\Models\Game;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

/**
 * Tous ont voté et au moins un joueur a demandé à changer : binomes et
 * personnages ont été tirés à nouveau pour tout le monde. Chaque joueur relit
 * le sien via GET /games/{game}/me.
 */
class CharactersRedrawn implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Game $game,
        public readonly int $changeCount,
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->game->id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'characters.redrawn';
    }

    public function broadcastWith(): array
    {
        return [
            'game_id' => $this->game->id,
            'draw_number' => $this->game->draw_number,
            'change_count' => $this->changeCount,
        ];
    }
}
