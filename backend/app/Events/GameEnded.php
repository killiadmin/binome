<?php

namespace App\Events;

use App\Models\Game;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcastNow;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;
use Illuminate\Support\Collection;

class GameEnded implements ShouldBroadcastNow
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    public function __construct(
        public readonly Game       $game,
        public readonly Collection $winners,
        public readonly Collection $stats,
    ) {}

    public function broadcastOn(): array
    {
        return [new PresenceChannel("game.{$this->game->id}")];
    }

    public function broadcastAs(): string
    {
        return 'game.ended';
    }

    public function broadcastWith(): array
    {
        return [
            'game_id' => $this->game->id,
            'winners' => $this->winners->map(fn($p) => [
                'id'     => $p->id,
                'pseudo' => $p->pseudo,
            ]),
            'stats' => $this->stats->map(fn($s) => [
                'player_pseudo'      => $s->player_pseudo,
                'character_name'     => $s->character_name,
                'eliminations'       => $s->eliminations,
                'rounds_survived'    => $s->rounds_survived,
                'survived_full_game' => $s->survived_full_game,
                'score'              => $s->score,
                'is_winner'          => $s->is_winner,
                'is_eliminated'      => $s->is_eliminated,
            ])->sortByDesc('score')->values(),
            'all_binomes' => $this->game->binomes->load('players', 'universe')
                ->map(fn($binome) => [
                    'id'       => $binome->id,
                    'universe' => $binome->universe->name,
                    'players'  => $binome->players->map(fn($p) => [
                        'pseudo'    => $p->pseudo,
                        'character' => \App\Models\Character::find($p->pivot->character_id)?->name,
                    ]),
                ]),
        ];
    }
}
