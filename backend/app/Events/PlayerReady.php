<?php

namespace App\Events;

use App\Models\Player;
use App\Models\Room;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class PlayerReady implements ShouldBroadcastNow
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
        return 'player.ready';
    }

    public function broadcastWith(): array
    {
        $players = $this->room->players()->get()->map(fn($p) => [
            'id'       => $p->id,
            'pseudo'   => $p->pseudo,
            'is_ready' => $p->pivot->is_ready,
        ]);

        return [
            'player_id' => $this->player->id,
            'players'   => $players,
            'can_start' => $players->every(fn($p) => $p['is_ready']),
        ];
    }
}
