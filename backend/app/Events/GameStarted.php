<?php

namespace App\Events;

use App\Models\Game;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcast;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class GameStarted implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Game $game
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("room.{$this->game->room_id}"),
            new PresenceChannel("game.{$this->game->id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'game.started';
    }

    /**
     * Ce que reçoit le front :
     * - La liste à plat des joueurs et le premier round
     * - La composition des binomes n'est JAMAIS diffusée : c'est tout l'enjeu
     *   de la partie. On annonce seulement qu'un orphelin existe (nombre de
     *   joueurs impair), sans dire de qui il s'agit.
     * - Chaque joueur voit UNIQUEMENT son propre personnage (pas celui des autres)
     */
    public function broadcastWith(): array
    {
        return [
            'game_id' => $this->game->id,
            'status'  => $this->game->status,
            'players' => $this->game->binomes
                ->flatMap(fn($binome) => $binome->players)
                ->map(fn($player) => [
                    'id'     => $player->id,
                    'pseudo' => $player->pseudo,
                ])
                ->sortBy('id')
                ->values(),
            'has_orphan' => $this->game->hasOrphan(),
            'first_round' => [
                'id'                => $this->game->rounds->first()->id,
                'number'            => 1,
                'current_player_id' => $this->game->rounds->first()->current_player_id,
            ],
        ];
    }
}
