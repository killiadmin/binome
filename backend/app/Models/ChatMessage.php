<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class ChatMessage extends Model
{
    protected $fillable = [
        'player_id',
        'author_name',
        'author_key',
        'body',
    ];

    public function player(): BelongsTo
    {
        return $this->belongsTo(Player::class);
    }

    /**
     * Codes de salon (format Room::generateUniqueCode : 6 caractères A-Z0-9)
     * réellement existants cités dans le message — le front en fait des puces
     * copiables. On confronte les candidats à la table `rooms` pour éviter que
     * n'importe quel mot de 6 lettres (« COUCOU ») passe pour un code.
     *
     * @return array<int, string>
     */
    public function mentionedRoomCodes(): array
    {
        if (! preg_match_all('/\b[A-Za-z0-9]{6}\b/', $this->body, $matches)) {
            return [];
        }

        $candidates = array_unique(array_map('strtoupper', $matches[0]));

        return Room::whereIn('code', $candidates)
            ->pluck('code')
            ->all();
    }

    /**
     * Charge utile partagée par l'API et l'événement de diffusion.
     */
    public function toBroadcastArray(): array
    {
        return [
            'id' => $this->id,
            'author_name' => $this->author_name,
            'author_key' => $this->author_key,
            // Dérivé de author_key (immuable) et non de player_id : le joueur
            // est supprimé quand il quitte son salon, le message doit garder
            // son identité d'origine.
            'is_anonymous' => str_starts_with($this->author_key, 'a:'),
            'body' => $this->body,
            'codes' => $this->mentionedRoomCodes(),
            'created_at' => $this->created_at?->toIso8601String(),
        ];
    }
}
