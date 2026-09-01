<?php

namespace App\Models;

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

    public function checkForbiddenWords(string $question): bool
    {
        foreach ($this->forbidden_words as $word) {
            if (stripos($question, $word) !== false) {
                return true;
            }
        }

        return false;
    }
}
