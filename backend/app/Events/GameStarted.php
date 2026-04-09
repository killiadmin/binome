<?php

namespace App\Events;

use App\Models\Game;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcast;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class GameStarted implements ShouldBroadcast
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
     * - La partie avec ses binomes et le premier round
     * - Chaque joueur voit UNIQUEMENT son propre personnage (pas celui des autres)
     */
    public function broadcastWith(): array
    {
        return [
            'game_id' => $this->game->id,
            'status'  => $this->game->status,
            'binomes' => $this->game->binomes->map(fn($binome) => [
                'id'          => $binome->id,
                'universe_id' => $binome->universe_id,
                'players'     => $binome->players->map(fn($player) => [
                    'id'     => $player->id,
                    'pseudo' => $player->pseudo,
                ]),
            ]),
            'first_round' => [
                'id'                => $this->game->rounds->first()->id,
                'number'            => 1,
                'current_player_id' => $this->game->rounds->first()->current_player_id,
            ],
        ];
    }
}
