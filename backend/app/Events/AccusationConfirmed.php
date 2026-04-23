<?php

namespace App\Events;

use App\Models\Action;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class AccusationConfirmed implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(public readonly Action $action) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->action->round->game_id}")];
    }

    public function broadcastAs(): string
    {
        return 'accusation.confirmed';
    }

    public function broadcastWith(): array
    {
        return [
            'action_id'            => $this->action->id,
            'round_id'             => $this->action->round_id,
            'accusation_confirmed' => $this->action->accusation_confirmed,
            'accusation_correct'   => $this->action->accusation_correct,
            'character_name'       => $this->action->character_name,
            'accuser' => [
                'id'     => $this->action->player->id,
                'pseudo' => $this->action->player->pseudo,
            ],
            'accused' => [
                'id'     => $this->action->targetPlayer->id,
                'pseudo' => $this->action->targetPlayer->pseudo,
            ],
        ];
    }
}
