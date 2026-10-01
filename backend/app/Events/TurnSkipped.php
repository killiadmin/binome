<?php

namespace App\Events;

use App\Models\Game;
use App\Models\Player;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;

/**
 * Un joueur déconnecté mettait la partie en pause et l'hôte a passé son tour
 * (voir ActionService::skipBlockedTurn).
 *
 * reason : 'turn'       → c'était son tour, il n'a rien joué
 *          'accusation' → il devait confirmer une accusation (tranchée par le serveur)
 */
class TurnSkipped implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets;

    public function __construct(
        public readonly Game $game,
        public readonly Player $skippedPlayer,
        public readonly Player $skippedBy,
        public readonly string $reason,
        public readonly ?int $actionId = null,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->game->id}")];
    }

    public function broadcastAs(): string
    {
        return 'turn.skipped';
    }

    public function broadcastWith(): array
    {
        return [
            'reason' => $this->reason,
            'action_id' => $this->actionId,
            'player' => [
                'id' => $this->skippedPlayer->id,
                'pseudo' => $this->skippedPlayer->pseudo,
            ],
            'skipped_by' => [
                'id' => $this->skippedBy->id,
                'pseudo' => $this->skippedBy->pseudo,
            ],
        ];
    }
}
