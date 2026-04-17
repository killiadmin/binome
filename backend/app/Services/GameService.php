<?php

namespace App\Services;

use App\Enums\GameStatus;
use App\Events\GameStarted;
use App\Models\Binome;
use App\Models\Game;
use App\Models\Room;
use App\Models\Character;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\DB;
use Exception;

class GameService
{
    public function __construct(
        private readonly RoundService $roundService
    ) {}

    /**
     * Point d'entrée : démarre une partie depuis un salon
     */
    public function start(Room $room): Game
    {
        $players = $room->players;

        $this->validateRoom($players);

        return DB::transaction(function () use ($room, $players) {
            // 1. Créer la Game
            $game = Game::create([
                'room_id' => $room->id,
                'status'  => GameStatus::InProgress,
            ]);

            // 2. Former les binomes aléatoirement
            $this->assignBinomes($game, $players);

            // 3. Créer le premier round
            $this->roundService->createRound($game, roundNumber: 1);

            // 4. Broadcast l'événement
            broadcast(new GameStarted($game->load('binomes.players', 'rounds')));

            return $game;
        });
    }

    /**
     * Valide que la room est prête à démarrer
     */
    private function validateRoom(Collection $players): void
    {
        if ($players->count() < 4) {
            throw new Exception('Il faut au minimum 4 joueurs pour démarrer.');
        }

        if ($players->count() % 2 !== 0) {
            throw new Exception('Le nombre de joueurs doit être pair pour former des binomes.');
        }

        $allReady = $players->every(
            fn($player) => $player->pivot->is_ready
        );

        if (!$allReady) {
            throw new Exception('Tous les joueurs doivent être prêts.');
        }
    }

    /**
     * Forme les binomes aléatoirement et assigne un personnage à chaque joueur
     */
    private function assignBinomes(Game $game, Collection $players): void
    {
        // Mélange aléatoire des joueurs
        $shuffledPlayers = $players->shuffle();

        // Découpe en paires : [P1, P2], [P3, P4], [P5, P6]...
        $pairs = $shuffledPlayers->chunk(2);

        // Récupère les univers disponibles (autant que de paires)
        $universes = \App\Models\Universe::inRandomOrder()
            ->take($pairs->count())
            ->get();

        if ($universes->count() < $pairs->count()) {
            throw new Exception('Pas assez d\'univers disponibles pour cette partie.');
        }

        foreach ($pairs as $index => $pair) {
            $universe = $universes[$index];

            // Crée le binome
            $binome = Binome::create([
                'game_id'     => $game->id,
                'universe_id' => $universe->id,
            ]);

            $characters = Character::where('universe_id', $universe->id)
                ->inRandomOrder()
                ->take(2)
                ->get();

            if ($characters->count() < 2) {
                throw new Exception(
                    "L'univers {$universe->name} n'a pas assez de personnages."
                );
            }

            // Attache chaque joueur au binome avec son personnage
            $pair->values()->each(function ($player, $i) use ($binome, $characters) {
                $binome->players()->attach($player->id, [
                    'character_id' => $characters[$i]->id,
                    'score'        => 0,
                ]);
            });
        }
    }
}
