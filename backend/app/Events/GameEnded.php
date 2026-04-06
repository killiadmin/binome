<?php

namespace App\Events;

use App\Models\Game;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcast;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;
use Illuminate\Support\Collection;

class GameEnded implements ShouldBroadcast
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Game       $game,
        public readonly Collection $winningBinomes,
    ) {}

    public function broadcastOn(): array
    {
        return [
            new PresenceChannel("game.{$this->game->id}"),
        ];
    }

    public function broadcastAs(): string
    {
        return 'game.ended';
    }

    public function broadcastWith(): array
    {
        return [
            'game_id'         => $this->game->id,
            'winning_binomes' => $this->winningBinomes->map(fn($binome) => [
                'id'      => $binome->id,
                'universe' => [
                    'id'   => $binome->universe_id,
                    'name' => $binome->universe->name,
                ],
                'players' => $binome->players->map(fn($player) => [
                    'id'        => $player->id,
                    'pseudo'    => $player->pseudo,
                    'character' => [
                        'id'   => $player->pivot->character_id,
                        'name' => \App\Models\Character::find($player->pivot->character_id)?->name,
                    ],
                    'score'     => $player->pivot->score,
                ]),
            ]),
            // Résumé complet de la partie : tous les binomes révélés
            'all_binomes' => $this->game->binomes->load('players', 'universe')
                ->map(fn($binome) => [
                    'id'          => $binome->id,
                    'is_winner'   => $this->winningBinomes->contains('id', $binome->id),
                    'universe'    => $binome->universe->name,
                    'players'     => $binome->players->map(fn($p) => [
                        'pseudo'    => $p->pseudo,
                        'character' => \App\Models\Character::find($p->pivot->character_id)?->name,
                    ]),
                ]),
        ];
    }
}
