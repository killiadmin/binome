<?php

namespace App\Events;

use App\Models\Player;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;

/**
 * Un joueur a changé (ou retiré) sa photo : diffusé dans ses salons et dans
 * la partie en cours de ces salons. Seule l'URL voyage, jamais l'image.
 */
class PlayerAvatarUpdated implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets;

    public function __construct(
        public readonly Player $player,
        public readonly ?string $avatarUrl,
    ) {}

    public function broadcastOn(): array
    {
        $this->player->loadMissing('rooms.currentGame');

        return $this->player->rooms
            ->flatMap(fn ($room) => array_filter([
                new PresenceChannel("room.{$room->id}"),
                $room->currentGame ? new PresenceChannel("game.{$room->currentGame->id}") : null,
            ]))
            ->values()
            ->all();
    }

    public function broadcastAs(): string
    {
        return 'player.avatar.updated';
    }

    public function broadcastWith(): array
    {
        return [
            'player_id' => $this->player->id,
            'avatar_url' => $this->avatarUrl,
        ];
    }
}
