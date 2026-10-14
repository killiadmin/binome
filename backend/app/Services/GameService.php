<?php

namespace App\Services;

use App\Enums\GameMode;
use App\Enums\GameStatus;
use App\Events\CharactersRedrawn;
use App\Events\CharacterVoteCast;
use App\Events\GameStarted;
use App\Models\Binome;
use App\Models\Character;
use App\Models\Game;
use App\Models\Player;
use App\Models\Room;
use App\Models\Universe;
use Exception;
use RuntimeException;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\DB;

class GameService
{
    public function __construct(
        private readonly RoundService $roundService
    ) {}

    /**
     * Point d'entrée : démarre une partie depuis un salon.
     * Les personnages sont distribués mais le round 1 n'est créé qu'une fois
     * que tous les joueurs les ont acceptés (voir voteCharacter()).
     */
    public function start(Room $room): Game
    {
        $players = $room->players;

        $this->validateRoom($players);

        return DB::transaction(function () use ($room, $players) {
            // 1. Créer la Game — characters_confirmed_at reste null : vote en cours
            $game = Game::create([
                'room_id' => $room->id,
                'status' => GameStatus::InProgress,
            ]);

            // 2. Former les binomes (mode aléatoire ou cosmos imposé)
            $this->assignBinomes($game, $players, $room);

            // 3. Broadcast l'événement
            broadcast(new GameStarted($game->load('binomes.players')));

            return $game;
        });
    }

    /**
     * Un joueur accepte son personnage, ou demande à en changer.
     *
     * - Accepté : quand tous les joueurs ont accepté, le round 1 démarre.
     * - Refusé : un joueur qui ne connaît pas son personnage donnerait de
     *   fausses réponses. Binomes et personnages sont retirés pour TOUT le
     *   monde (le personnage refusé est écarté), et chacun revote.
     *
     * `$characterId` est le personnage que le joueur a sous les yeux : si un
     * retirage est passé entre-temps, son vote portait sur un personnage qui
     * n'est plus le sien et il est rejeté.
     */
    public function voteCharacter(Game $game, Player $player, int $characterId, bool $accepted): void
    {
        $event = DB::transaction(function () use ($game, $player, $characterId, $accepted) {
            // Verrou : un refus remplace tous les personnages, les votes
            // simultanés doivent passer l'un après l'autre.
            $game = Game::whereKey($game->id)->lockForUpdate()->firstOrFail();

            if (! $game->isVotingCharacters()) {
                throw new RuntimeException('Le choix des personnages est terminé.');
            }

            $binome = $game->binomes()
                ->whereHas('players', fn ($q) => $q->where('player_id', $player->id))
                ->with('players')
                ->first();

            if (! $binome) {
                throw new RuntimeException('Tu ne joues pas dans cette partie.');
            }

            $pivot = $binome->players->firstWhere('id', $player->id)->pivot;

            if ((int) $pivot->character_id !== $characterId) {
                throw new RuntimeException('Les personnages ont été tirés à nouveau : regarde ton nouveau personnage.');
            }

            if ($pivot->character_accepted) {
                throw new RuntimeException('Tu as déjà accepté ton personnage.');
            }

            if (! $accepted) {
                $this->redraw($game, $characterId);

                return new CharactersRedrawn($game);
            }

            $binome->players()->updateExistingPivot($player->id, ['character_accepted' => true]);

            $votes = $this->characterVotes($game);

            if ($votes['accepted']->count() === $votes['total']) {
                $game->update(['characters_confirmed_at' => now()]);
                $this->roundService->createRound($game, roundNumber: 1);
            }

            return new CharacterVoteCast($game, $votes['accepted']->all());
        });

        // Après le commit : les joueurs relisent leur personnage dès la
        // réception, ils doivent trouver le nouveau tirage.
        broadcast($event);
    }

    /**
     * Où en est le vote : ids des joueurs qui ont accepté, et nombre de joueurs.
     *
     * @return array{accepted: Collection<int, int>, total: int}
     */
    public function characterVotes(Game $game): array
    {
        $rows = DB::table('binome_player')
            ->join('binomes', 'binomes.id', '=', 'binome_player.binome_id')
            ->where('binomes.game_id', $game->id)
            ->get(['binome_player.player_id', 'binome_player.character_accepted']);

        return [
            'accepted' => $rows->filter(fn ($row) => $row->character_accepted)
                ->map(fn ($row) => (int) $row->player_id)
                ->sort()
                ->values(),
            'total' => $rows->count(),
        ];
    }

    /**
     * Nouveau tirage complet (binomes, univers, personnages) avec les mêmes
     * joueurs. Les spectateurs arrivés dans le salon depuis le lancement n'y
     * entrent pas : on repart des joueurs de la partie, pas de ceux du salon.
     */
    private function redraw(Game $game, int $rejectedCharacterId): void
    {
        $players = $game->binomes()->with('players')->get()
            ->flatMap(fn ($binome) => $binome->players)
            ->values();

        $rejected = collect($game->rejected_character_ids ?? [])
            ->push($rejectedCharacterId)
            ->unique()
            ->values()
            ->all();

        // binome_player part avec (cascadeOnDelete)
        $game->binomes()->delete();

        $game->update([
            'draw_number' => $game->draw_number + 1,
            'rejected_character_ids' => $rejected,
        ]);

        $this->assignBinomes($game, $players, $game->room, $rejected);
    }

    /**
     * Valide que la room est prête à démarrer
     */
    private function validateRoom(Collection $players): void
    {
        if ($players->count() < 4) {
            throw new Exception('Il faut au minimum 4 joueurs pour démarrer.');
        }

        // Un nombre impair est accepté : le joueur surnuméraire devient orphelin
        // (binome d'un seul joueur) — voir assignBinomes().

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
     *
     * Si le nombre de joueurs est impair, le dernier joueur du mélange se
     * retrouve seul : son binome est marqué `is_orphan` et ne reçoit qu'un
     * personnage. Il joue comme une équipe à part entière (et touche le bonus
     * de victoire) mais n'a aucun partenaire à protéger — ce qu'il ignore.
     *
     * Les personnages refusés pendant le vote sont écartés du tirage tant que
     * le catalogue le permet ; sinon on retombe sur tous les personnages.
     *
     * @param  int[]  $excludedCharacterIds
     */
    private function assignBinomes(Game $game, Collection $players, Room $room, array $excludedCharacterIds = []): void
    {
        // Mélange aléatoire puis découpe en paires : [P1, P2], [P3, P4]...
        // En nombre impair, la dernière « paire » ne contient qu'un joueur.
        $teams = $players->shuffle()->chunk(2)->map(fn ($team) => $team->values())->values();

        $draw = null;

        if ($excludedCharacterIds) {
            try {
                $draw = $this->drawCharacters($teams, $room, $excludedCharacterIds);
            } catch (Exception) {
                // Pas assez de personnages sans les refusés : tirage sur tout le catalogue
            }
        }

        $draw ??= $this->drawCharacters($teams, $room);

        foreach ($teams as $index => $members) {
            $binome = Binome::create([
                'game_id' => $game->id,
                'universe_id' => $draw[$index]['universe']->id,
                'is_orphan' => $members->count() === 1,
            ]);

            $characters = $draw[$index]['characters'];

            // Attache chaque joueur au binome avec son personnage
            $members->each(function ($player, $i) use ($binome, $characters) {
                $binome->players()->attach($player->id, [
                    'character_id' => $characters[$i]->id,
                    'score' => 0,
                ]);
            });
        }
    }

    /**
     * Tire un univers et les personnages de chaque équipe, sans rien écrire.
     *
     * @param  int[]  $excludedCharacterIds
     * @return array<int, array{universe: Universe, characters: Collection}>
     */
    private function drawCharacters(Collection $teams, Room $room, array $excludedCharacterIds = []): array
    {
        $universes = $this->resolveUniverses($teams->count(), $room, $excludedCharacterIds);

        return $teams->map(function ($members, $index) use ($universes, $excludedCharacterIds) {
            $universe = $universes[$index];

            // Un binome ne peut être formé qu'entre 2 personnages partageant le même
            // niveau d'affectation au sein de l'univers (ex : Luke + Leia = niveau 1)
            $levelGroups = Character::playable()
                ->where('universe_id', $universe->id)
                ->whereNotIn('id', $excludedCharacterIds)
                ->get()
                ->groupBy('level_affectation')
                ->filter(fn ($group) => $group->count() >= 2);

            if ($levelGroups->isEmpty()) {
                throw new Exception(
                    "L'univers {$universe->name} n'a pas de binome de personnages valide (niveau d'affectation)."
                );
            }

            // L'orphelin ne tire qu'un personnage, mais toujours dans un univers
            // qui aurait pu porter un vrai binome : rien ne le trahit.
            return [
                'universe' => $universe,
                'characters' => $levelGroups->random()->shuffle()->take($members->count())->values(),
            ];
        })->all();
    }

    /**
     * Tire `$teamCount` univers distincts jouables (≥ 2 personnages jouables au même
     * niveau d'affectation). En mode « cosmos imposé », restreint au cosmos de la room.
     * L'orphelin compte comme une équipe : il consomme lui aussi un univers.
     *
     * @param  int[]  $excludedCharacterIds
     */
    private function resolveUniverses(int $teamCount, Room $room, array $excludedCharacterIds = []): Collection
    {
        // Univers éligibles : ceux qui ont au moins un binôme de personnages
        // JOUABLES (verif_manual = true, hidden = false) partageant un même niveau.
        $eligibleUniverseIds = Character::playable()
            ->whereNotIn('id', $excludedCharacterIds)
            ->select('universe_id')
            ->groupBy('universe_id', 'level_affectation')
            ->havingRaw('COUNT(*) >= 2')
            ->pluck('universe_id')
            ->unique();

        $query = Universe::whereIn('id', $eligibleUniverseIds)->inRandomOrder();

        if ($room->game_mode === GameMode::Cosmos) {
            $query->where('cosmos_id', $room->cosmos_id);
        }

        $universes = $query->take($teamCount)->get();

        if ($universes->count() < $teamCount) {
            if ($room->game_mode === GameMode::Cosmos) {
                throw new Exception(
                    "Le cosmos « {$room->cosmos?->name} » n'a que {$universes->count()} univers jouable(s) "
                    ."pour {$teamCount} binôme(s). Choisis un autre cosmos ou ajoute des personnages."
                );
            }

            throw new Exception('Pas assez d\'univers avec des personnages validés pour cette partie.');
        }

        return $universes;
    }
}
