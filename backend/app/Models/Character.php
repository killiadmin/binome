<?php

namespace App\Models;

use App\Support\ForbiddenWordMatcher;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;

class Character extends Model
{
    protected $fillable = ['universe_id', 'name', 'slug', 'image', 'forbidden_words', 'level_affectation', 'verif_manual', 'hidden'];

    protected $casts = [
        'forbidden_words' => 'array',
        'level_affectation' => 'integer',
        'verif_manual' => 'boolean',
        'hidden' => 'boolean',
    ];

    public function universe(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Universe::class);
    }

    /** Personnages utilisables en partie : relus par un humain et non masqués. */
    public function scopePlayable(Builder $query): Builder
    {
        return $query->where('verif_manual', true)->where('hidden', false);
    }

    /** Propositions en attente de validation manuelle (ni validées, ni refusées). */
    public function scopePendingValidation(Builder $query): Builder
    {
        return $query->where('verif_manual', false)->where('hidden', false);
    }

    /**
     * URL versionnée de l'image (GET /api/character-images/{id}), mise en
     * cache par le navigateur : l'image (data URI de 50 à 160 ko) n'est plus
     * renvoyée par GET /games/{game}/me à chaque nouveau tirage ni à chaque
     * resynchronisation. `v` change avec updated_at, donc à chaque nouvel import.
     */
    public function imageUrl(): ?string
    {
        // Chargé via withoutImage() : seul l'indicateur has_image est connu.
        $hasImage = array_key_exists('has_image', $this->attributes)
            ? (bool) $this->attributes['has_image']
            : ! empty($this->image);

        return $hasImage
            ? config('app.public_prefix')."/api/character-images/{$this->id}?v={$this->updated_at?->timestamp}"
            : null;
    }

    /**
     * Toutes les colonnes sauf l'image (data URI de 50 à 160 ko — ~11 Mo pour
     * tout le catalogue), remplacée par un booléen `has_image`. Pour les
     * listes, qui n'ont besoin que de imageUrl().
     */
    public function scopeWithoutImage(Builder $query): Builder
    {
        return $query
            ->select(array_map(
                fn ($column) => "characters.{$column}",
                ['id', 'universe_id', 'level_affectation', 'verif_manual', 'hidden', 'name', 'slug', 'forbidden_words', 'created_at', 'updated_at'],
            ))
            ->selectRaw("(characters.image IS NOT NULL AND characters.image <> '') AS has_image");
    }

    public function checkForbiddenWords(string $question): bool
    {
        return ForbiddenWordMatcher::matchesAny($question, $this->forbidden_words ?? []);
    }
}
