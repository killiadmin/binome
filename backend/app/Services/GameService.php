<?php

namespace App\Services;

use App\Enums\GameMode;
use App\Enums\GameStatus;
use App\Events\GameStarted;
use App\Models\Binome;
use App\Models\Character;
use App\Models\Game;
use App\Models\Room;
use App\Models\Universe;
use Exception;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\DB;

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
                'status' => GameStatus::InProgress,
            ]);

            // 2. Former les binomes (mode aléatoire ou cosmos imposé)
            $this->assignBinomes($game, $players, $room);

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
            fn ($player) => $player->pivot->is_ready
        );

        if (! $allReady) {
            throw new Exception('Tous les joueurs doivent être prêts.');
        }
    }

    /**
     * Forme les binomes et assigne un personnage à chaque joueur.
     * Un univers distinct par binome ; en mode « cosmos imposé » tous les
     * univers proviennent du cosmos choisi par l'hôte.
     */
    private function assignBinomes(Game $game, Collection $players, Room $room): void
    {
        // Mélange aléatoire des joueurs
        $shuffledPlayers = $players->shuffle();

        // Découpe en paires : [P1, P2], [P3, P4], [P5, P6]...
        $pairs = $shuffledPlayers->chunk(2);

        $universes = $this->resolveUniverses($pairs->count(), $room);

        foreach ($pairs as $index => $pair) {
            $universe = $universes[$index];

            // Crée le binome
            $binome = Binome::create([
                'game_id' => $game->id,
                'universe_id' => $universe->id,
            ]);

            // Un binome ne peut être formé qu'entre 2 personnages partageant le même
            // niveau d'affectation au sein de l'univers (ex : Luke + Leia = niveau 1)
            $levelGroups = Character::playable()
                ->where('universe_id', $universe->id)
                ->get()
                ->groupBy('level_affectation')
                ->filter(fn ($group) => $group->count() >= 2);

            if ($levelGroups->isEmpty()) {
                throw new Exception(
                    "L'univers {$universe->name} n'a pas de binome de personnages valide (niveau d'affectation)."
                );
            }

            $characters = $levelGroups->random()->shuffle()->take(2)->values();

            // Attache chaque joueur au binome avec son personnage
            $pair->values()->each(function ($player, $i) use ($binome, $characters) {
                $binome->players()->attach($player->id, [
                    'character_id' => $characters[$i]->id,
                    'score' => 0,
                ]);
            });
        }
    }

    /**
     * Tire `$pairCount` univers distincts jouables (≥ 2 personnages jouables au même
     * niveau d'affectation). En mode « cosmos imposé », restreint au cosmos de la room.
     */
    private function resolveUniverses(int $pairCount, Room $room): Collection
    {
        // Univers éligibles : ceux qui ont au moins un binôme de personnages
        // JOUABLES (verif_manual = true, hidden = false) partageant un même niveau.
        $eligibleUniverseIds = Character::playable()
            ->select('universe_id')
            ->groupBy('universe_id', 'level_affectation')
            ->havingRaw('COUNT(*) >= 2')
            ->pluck('universe_id')
            ->unique();

        $query = Universe::whereIn('id', $eligibleUniverseIds)->inRandomOrder();

        if ($room->game_mode === GameMode::Cosmos) {
            $query->where('cosmos_id', $room->cosmos_id);
        }

        $universes = $query->take($pairCount)->get();

        if ($universes->count() < $pairCount) {
            if ($room->game_mode === GameMode::Cosmos) {
                throw new Exception(
                    "Le cosmos « {$room->cosmos?->name} » n'a que {$universes->count()} univers jouable(s) "
                    ."pour {$pairCount} binôme(s). Choisis un autre cosmos ou ajoute des personnages."
                );
            }

            throw new Exception('Pas assez d\'univers avec des personnages validés pour cette partie.');
        }

        return $universes;
    }
}
