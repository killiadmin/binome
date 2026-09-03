<?php

namespace App\Events;

use App\Models\Room;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class RoomSettingsUpdated implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Room $room,
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("room.{$this->room->id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'room.settings.updated';
    }

    public function broadcastWith(): array
    {
        return [
            'game_mode' => $this->room->game_mode,
            'cosmos_id' => $this->room->cosmos_id,
            'cosmos_name' => $this->room->cosmos?->name,
        ];
    }
}
