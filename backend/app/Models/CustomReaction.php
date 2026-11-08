<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

/**
 * Réaction utilisable en partie : soit un emoji Unicode (`emoji`), soit une
 * image créée par un joueur ou un admin (`image`). Toutes sont gérables dans
 * l'onglet Emojis de l'admin.
 *
 * Même principe que PlayerAvatar : l'image n'est jamais incluse dans le JSON
 * ni dans les events (Reverb limite un message à 10 ko), seulement une URL
 * versionnée vers GET /api/reactions/{id}/image.
 */
class CustomReaction extends Model
{
    protected $fillable = ['name', 'emoji', 'image', 'created_by_pseudo', 'is_enabled'];

    protected $hidden = ['image'];

    protected $casts = [
        'is_enabled' => 'boolean',
    ];

    public function scopeEnabled($query)
    {
        return $query->where('is_enabled', true);
    }

    /** Ordre de la barre : emojis Unicode d'abord, puis images, par ancienneté. */
    public function scopeBarOrder($query)
    {
        return $query->orderByRaw('emoji is null')->orderBy('id');
    }

    /** null pour un emoji Unicode. Ne demande pas de charger `image`. */
    public function url(): ?string
    {
        return $this->emoji === null
            ? config('app.public_prefix')."/api/reactions/{$this->id}/image?v={$this->updated_at?->timestamp}"
            : null;
    }

    /** Forme publique, partagée par la liste, la création et ReactionSent. */
    public function toPublicArray(): array
    {
        return [
            'id' => $this->id,
            'name' => $this->name,
            'emoji' => $this->emoji,
            'url' => $this->url(),
            'created_by' => $this->created_by_pseudo,
        ];
    }
}
