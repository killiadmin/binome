<?php

namespace App\Events;

use App\Models\Action;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class AnswerGiven implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Action $action
    )
    {
    }

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->action->round->game_id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'answer.given';
    }

    public function broadcastWith(): array
    {
        return [
            'action_id' => $this->action->id,
            'round_id' => $this->action->round_id,
            'answer' => $this->action->answer,
            'answerer' => [
                'id' => $this->action->targetPlayer->id,
                'pseudo' => $this->action->targetPlayer->pseudo,
            ],
            'asker' => [
                'id' => $this->action->player->id,
                'pseudo' => $this->action->player->pseudo,
            ],
        ];
    }
}
