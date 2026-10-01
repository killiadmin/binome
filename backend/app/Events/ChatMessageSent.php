<?php

namespace App\Events;

use App\Models\ChatMessage;
use Illuminate\Broadcasting\Channel;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class ChatMessageSent implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly ChatMessage $message,
    ) {}

    /**
     * Channel public : le chat du hall est ouvert aux visiteurs qui n'ont ni
     * créé ni rejoint de salon (donc sans Player, donc sans auth possible).
     */
    public function broadcastOn(): array
    {
        return [
            new Channel('lobby'),
        ];
    }

    public function broadcastAs(): string
    {
        return 'chat.message';
    }

    public function broadcastWith(): array
    {
        return ['message' => $this->message->toBroadcastArray()];
    }
}
