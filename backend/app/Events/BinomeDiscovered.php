<?php

// app/Events/BinomeDiscovered.php

namespace App\Events;

use App\Models\Binome;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcast;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class BinomeDiscovered implements ShouldBroadcast
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Binome $binome
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->binome->game_id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'binome.discovered';
    }

    public function broadcastWith(): array
    {
        return [
            'binome_id' => $this->binome->id,
            'universe'  => [
                'id'   => $this->binome->universe->id,
                'name' => $this->binome->universe->name,
            ],
            // Ici on peut révéler les personnages puisque le binome est découvert
            'players'   => $this->binome->players->map(fn($player) => [
                'id'            => $player->id,
                'pseudo'        => $player->pseudo,
                'character'     => [
                    'id'   => $player->pivot->character_id,
                    'name' => \App\Models\Character::find($player->pivot->character_id)?->name,
                ],
            ]),
            'discovered_by' => [
                'id'     => $this->binome->discoveredBy->id,
                'pseudo' => $this->binome->discoveredBy->pseudo,
            ],
        ];
    }
}
