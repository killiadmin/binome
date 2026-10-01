<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Support\Str;

/**
 * Photo de profil, indexée par pseudo (voir la migration create_player_avatars_table).
 *
 * L'image n'est jamais incluse dans les réponses JSON ni dans les events
 * WebSocket (Reverb limite un message à 10 ko) : on n'expose qu'une URL
 * versionnée vers GET /api/avatars/{id}, que le navigateur met en cache.
 */
class PlayerAvatar extends Model
{
    protected $fillable = ['pseudo', 'image'];

    protected $hidden = ['image'];

    public function url(): string
    {
        return "/api/avatars/{$this->id}?v={$this->updated_at?->timestamp}";
    }

    /**
     * URL de la photo de chaque joueur, en une requête.
     *
     * @param  iterable<Player>  $players
     * @return array<int, string|null> player_id → url
     */
    public static function urlsFor(iterable $players): array
    {
        $players = collect($players);
        $urls = static::urlsForPseudos($players->pluck('pseudo'));

        return $players
            ->mapWithKeys(fn (Player $player) => [$player->id => $urls[$player->pseudo] ?? null])
            ->all();
    }

    /**
     * Même chose à partir de pseudos seuls — l'historique ne garde que des
     * pseudos (game_stats), les lignes `players` pouvant avoir disparu.
     *
     * @param  iterable<string>  $pseudos
     * @return array<string, string|null> pseudo (tel que fourni) → url
     */
    public static function urlsForPseudos(iterable $pseudos): array
    {
        $pseudos = collect($pseudos)->filter()->unique()->values();

        if ($pseudos->isEmpty()) {
            return [];
        }

        // La comparaison SQL suit la collation (insensible casse / accents) ;
        // on refait le rapprochement en PHP avec la même tolérance.
        $key = fn (string $pseudo) => Str::of($pseudo)->ascii()->lower()->value();

        $avatars = static::query()
            ->whereIn('pseudo', $pseudos)
            ->get(['id', 'pseudo', 'updated_at'])
            ->keyBy(fn (self $avatar) => $key($avatar->pseudo));

        return $pseudos
            ->mapWithKeys(fn (string $pseudo) => [$pseudo => $avatars->get($key($pseudo))?->url()])
            ->all();
    }
}
