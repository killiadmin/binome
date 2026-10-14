<?php

namespace App\Events;

use App\Models\Game;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

/**
 * Un joueur a accepté son personnage. Seul le fait d'avoir accepté est
 * diffusé, jamais le personnage.
 */
class CharacterVoteCast implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    /**
     * @param  int[]  $acceptedPlayerIds
     */
    public function __construct(
        public readonly Game $game,
        public readonly array $acceptedPlayerIds,
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->game->id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'character.vote.cast';
    }

    public function broadcastWith(): array
    {
        return [
            'game_id' => $this->game->id,
            'draw_number' => $this->game->draw_number,
            'accepted_player_ids' => $this->acceptedPlayerIds,
        ];
    }
}
