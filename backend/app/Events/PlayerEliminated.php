<?php

namespace App\Events;

use App\Models\Game;
use App\Models\Player;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class PlayerEliminated implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Game   $game,
        public readonly Player $eliminatedPlayer,
        public readonly Player $eliminatedBy,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->game->id}")];
    }

    public function broadcastAs(): string
    {
        return 'player.eliminated';
    }

    public function broadcastWith(): array
    {
        return [
            'eliminated_player' => [
                'id'     => $this->eliminatedPlayer->id,
                'pseudo' => $this->eliminatedPlayer->pseudo,
            ],
            'eliminated_by' => [
                'id'     => $this->eliminatedBy->id,
                'pseudo' => $this->eliminatedBy->pseudo,
            ],
        ];
    }
}
