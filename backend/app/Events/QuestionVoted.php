<?php

namespace App\Events;

use App\Models\Game;
use App\Models\Round;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

/**
 * Un éliminé a voté (ou changé son vote) pour la meilleure question d'un
 * round. On diffuse tous les votes du round : le front remplace sa liste.
 */
class QuestionVoted implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    /**
     * @param  array<int, array{action_id: int, voter_id: int}>  $votes
     */
    public function __construct(
        public Game $game,
        public Round $round,
        public array $votes,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->game->id}")];
    }

    public function broadcastAs(): string
    {
        return 'question.voted';
    }

    public function broadcastWith(): array
    {
        return [
            'round_id' => $this->round->id,
            'votes' => $this->votes,
        ];
    }
}
