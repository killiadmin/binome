<?php

namespace App\Events;

use App\Models\Player;
use App\Models\Room;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class PlayerJoined implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Room   $room,
        public readonly Player $player,
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("room.{$this->room->id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'player.joined';
    }

    public function broadcastWith(): array
    {
        return [
            'player' => [
                'id'       => $this->player->id,
                'pseudo'   => $this->player->pseudo,
                'is_ready' => false,
            ],
            'players' => $this->room->players->map(fn($p) => [
                'id'       => $p->id,
                'pseudo'   => $p->pseudo,
                'is_ready' => $p->pivot->is_ready,
            ]),
        ];
    }
}
