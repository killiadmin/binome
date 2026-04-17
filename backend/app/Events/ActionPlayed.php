<?php

namespace App\Events;

use App\Models\Action;
use App\Enums\ActionType;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcast;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class ActionPlayed implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Action $action
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->action->round->game_id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'action.played';
    }

    public function broadcastWith(): array
    {
        $base = [
            'action_id'  => $this->action->id,
            'round_id'   => $this->action->round_id,
            'round_number' => $this->action->round->number,
            'player'     => [
                'id'     => $this->action->player->id,
                'pseudo' => $this->action->player->pseudo,
            ],
            'type'       => $this->action->type,
            'is_valid'   => $this->action->is_valid,
        ];

        // Pour une question : on expose le contenu à tout le monde
        // (les autres joueurs doivent entendre la question)
        if ($this->action->type === ActionType::Question) {
            return array_merge($base, [
                'question' => $this->action->is_valid ? $this->action->content : null,
                'refused_reason' => !$this->action->is_valid ? 'Mot interdit détecté.' : null,
                'target_player'  => $this->action->targetPlayer ? [
                    'id'     => $this->action->targetPlayer->id,
                    'pseudo' => $this->action->targetPlayer->pseudo,
                ] : null,
            ]);
        }

        // Pour une accusation : on expose la cible et le résultat
        return array_merge($base, [
            'target_player' => [
                'id'     => $this->action->targetPlayer->id,
                'pseudo' => $this->action->targetPlayer->pseudo,
            ],
            'accusation_correct' => $this->action->accusation_correct,
            // On n'expose le nom du personnage accusé que si c'est correct
            // pour ne pas donner trop d'indices aux autres joueurs
            'character_name' => $this->action->accusation_correct
                ? $this->action->content
                : null,
        ]);
    }
}
