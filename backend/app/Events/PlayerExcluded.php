<?php

namespace App\Events;

use App\Models\Game;
use App\Models\Player;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;

/**
 * L'hôte a exclu un joueur déconnecté (ActionService::excludePlayer) : lui et
 * son binôme sont éliminés. Révéler le partenaire est inhérent à la règle.
 *
 * cancelled_action_id : question / accusation en attente devenue sans objet
 * (posée par ou à un joueur éliminé), supprimée — le joueur courant rejoue
 * si c'est lui qui l'avait posée.
 */
class PlayerExcluded implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets;

    public function __construct(
        public readonly Game $game,
        public readonly Player $excluded,
        public readonly ?Player $partner,
        public readonly Player $excludedBy,
        public readonly ?int $cancelledActionId = null,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->game->id}")];
    }

    public function broadcastAs(): string
    {
        return 'player.excluded';
    }

    public function broadcastWith(): array
    {
        return [
            'player' => [
                'id' => $this->excluded->id,
                'pseudo' => $this->excluded->pseudo,
            ],
            'partner' => $this->partner ? [
                'id' => $this->partner->id,
                'pseudo' => $this->partner->pseudo,
            ] : null,
            'excluded_by' => [
                'id' => $this->excludedBy->id,
                'pseudo' => $this->excludedBy->pseudo,
            ],
            'cancelled_action_id' => $this->cancelledActionId,
        ];
    }
}
