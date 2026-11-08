<?php

namespace App\Events;

use App\Models\Action;
use App\Models\Player;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

/**
 * L'hôte a corrigé une réponse oui / non (le joueur s'était trompé de bouton
 * ou s'est ravisé à voix haute).
 */
class AnswerCorrected implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Action $action,
        public readonly string $previousAnswer,
        public readonly Player $host,
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->action->round->game_id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'answer.corrected';
    }

    public function broadcastWith(): array
    {
        return [
            'action_id' => $this->action->id,
            'round_id' => $this->action->round_id,
            'answer' => $this->action->answer,
            'previous_answer' => $this->previousAnswer,
            // Réponse d'origine du joueur, null si la correction y est revenue
            'answer_corrected_from' => $this->action->answer_corrected_from,
            'question' => $this->action->content,
            'answerer' => [
                'id' => $this->action->targetPlayer->id,
                'pseudo' => $this->action->targetPlayer->pseudo,
            ],
            'host' => [
                'id' => $this->host->id,
                'pseudo' => $this->host->pseudo,
            ],
        ];
    }
}
