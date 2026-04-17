<?php

namespace App\Events;

use App\Models\Round;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class RoundStarted implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Round $round,
        public readonly bool  $isNewRound = false,
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->round->game_id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'round.started';
    }

    public function broadcastWith(): array
    {
        return [
            'round_id'          => $this->round->id,
            'number'            => $this->round->number,
            'is_new_round'   => $this->isNewRound,
            'current_player'    => [
                'id'     => $this->round->currentPlayer->id,
                'pseudo' => $this->round->currentPlayer->pseudo,
            ],
        ];
    }
}
