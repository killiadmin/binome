<?php

namespace App\Http\Controllers;

use App\Enums\GameStatus;
use App\Models\Game;
use App\Models\GameStat;
use App\Models\Player;
use App\Models\PlayerAvatar;
use App\Models\Room;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

/**
 * Liste des joueurs inscrits (lignes `players`) — même mot de passe que la
 * gestion des personnages (middleware characters.access).
 *
 * Un joueur qui quitte un salon sans avoir jamais joué est supprimé
 * (RoomController::leave) : la liste contient donc les joueurs ayant joué au
 * moins une partie, plus ceux actuellement présents dans un salon.
 */
class AdminPlayerController extends Controller
{
    /**
     * GET /admin/players
     */
    public function index(): JsonResponse
    {
        $players = Player::query()->orderBy('pseudo')->get(['id', 'pseudo', 'created_at', 'updated_at']);

        $gamesCount = DB::table('binome_player')
            ->select('player_id', DB::raw('count(*) as total'))
            ->groupBy('player_id')
            ->pluck('total', 'player_id');

        // Salon actuel : un joueur n'est que dans un salon à la fois.
        $rooms = DB::table('room_player')
            ->join('rooms', 'rooms.id', '=', 'room_player.room_id')
            ->pluck('rooms.code', 'room_player.player_id');

        // Victoires et points cumulés : game_stats ne garde que le pseudo.
        // Même rapprochement insensible casse / accents que la collation SQL.
        $key = fn (string $pseudo) => Str::of($pseudo)->ascii()->lower()->value();
        $stats = GameStat::query()
            ->select('player_pseudo', DB::raw('sum(is_winner) as wins'), DB::raw('sum(score) as score'))
            ->groupBy('player_pseudo')
            ->get()
            ->keyBy(fn ($row) => $key($row->player_pseudo));

        // Joueurs d'une partie en cours : ni retirables ni supprimables.
        $inGame = $this->playersInProgressGames();

        $avatars = PlayerAvatar::urlsFor($players);

        return response()->json([
            'players' => $players->map(fn (Player $player) => [
                'id' => $player->id,
                'pseudo' => $player->pseudo,
                'avatar_url' => $avatars[$player->id] ?? null,
                'games_count' => (int) ($gamesCount[$player->id] ?? 0),
                'wins' => (int) ($stats->get($key($player->pseudo))?->wins ?? 0),
                'score' => (int) ($stats->get($key($player->pseudo))?->score ?? 0),
                'room_code' => $rooms[$player->id] ?? null,
                'in_game' => isset($inGame[$player->id]),
                'created_at' => $player->created_at?->toIso8601String(),
                'updated_at' => $player->updated_at?->toIso8601String(),
            ])->values(),
        ]);
    }

    /**
     * DELETE /admin/players/{player}/room
     * Retire le joueur de son salon (hôte transféré, salon supprimé s'il est
     * vide). Son client reçoit PlayerLeft sans lui et quitte le lobby.
     */
    public function removeFromRoom(Player $player): JsonResponse
    {
        if ($blocked = $this->blockedByGame($player)) {
            return $blocked;
        }

        $rooms = $player->rooms()->get();

        if ($rooms->isEmpty()) {
            return response()->json(['message' => "{$player->pseudo} n'est dans aucun salon."], 409);
        }

        $rooms->each(fn (Room $room) => $room->removePlayer($player->id));

        return response()->json(['message' => "{$player->pseudo} a été retiré du salon."]);
    }

    /**
     * DELETE /admin/players/{player}?with_games=1
     *
     * Retire le joueur de ses salons puis supprime sa ligne et sa photo.
     * Un joueur qui a déjà joué est référencé par les actions et les rounds
     * de ses parties (FK sans cascade) : sans `with_games`, la suppression est
     * refusée (409 + nombre de parties) ; avec, ses parties sont supprimées
     * d'abord — pour tous leurs joueurs, stats et classement compris.
     */
    public function destroy(Request $request, Player $player): JsonResponse
    {
        if ($blocked = $this->blockedByGame($player)) {
            return $blocked;
        }

        $gameIds = $this->gameIdsOf($player);

        if ($gameIds->isNotEmpty() && ! $request->boolean('with_games')) {
            return response()->json([
                'message' => "{$player->pseudo} a joué {$gameIds->count()} partie(s) : supprime-les aussi pour pouvoir supprimer le joueur.",
                'games_count' => $gameIds->count(),
            ], 409);
        }

        DB::transaction(function () use ($player, $gameIds) {
            // Salons d'abord : rooms.created_by est en cascade.
            $player->rooms()->get()->each(fn (Room $room) => $room->removePlayer($player->id));

            Game::whereIn('id', $gameIds)->get()->each->delete();

            PlayerAvatar::where('pseudo', $player->pseudo)->delete();
            $player->delete();
        });

        return response()->json([
            'message' => $gameIds->isEmpty()
                ? "{$player->pseudo} a été supprimé."
                : "{$player->pseudo} et ses {$gameIds->count()} partie(s) ont été supprimés.",
        ]);
    }

    /** 409 si le joueur est dans une partie en cours — passer par l'onglet Parties. */
    private function blockedByGame(Player $player): ?JsonResponse
    {
        if (! isset($this->playersInProgressGames()[$player->id])) {
            return null;
        }

        return response()->json([
            'message' => "{$player->pseudo} est dans une partie en cours : supprime ou termine d'abord la partie.",
        ], 409);
    }

    /** @return array<int, true> player_id → true */
    private function playersInProgressGames(): array
    {
        return DB::table('binome_player')
            ->join('binomes', 'binomes.id', '=', 'binome_player.binome_id')
            ->join('games', 'games.id', '=', 'binomes.game_id')
            ->where('games.status', GameStatus::InProgress->value)
            ->pluck('binome_player.player_id')
            ->mapWithKeys(fn ($id) => [$id => true])
            ->all();
    }

    /** Parties qui référencent le joueur (personnage, action jouée ou subie, tour). */
    private function gameIdsOf(Player $player): \Illuminate\Support\Collection
    {
        return DB::table('binome_player')
            ->join('binomes', 'binomes.id', '=', 'binome_player.binome_id')
            ->where('binome_player.player_id', $player->id)
            ->pluck('binomes.game_id')
            ->merge(
                DB::table('actions')
                    ->join('rounds', 'rounds.id', '=', 'actions.round_id')
                    ->where(fn ($q) => $q->where('actions.player_id', $player->id)->orWhere('actions.target_player_id', $player->id))
                    ->pluck('rounds.game_id')
            )
            ->merge(DB::table('rounds')->where('current_player_id', $player->id)->pluck('game_id'))
            ->unique()
            ->values();
    }
}
