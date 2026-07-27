<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Character extends Model
{
    protected $fillable = ['universe_id', 'name', 'slug', 'image', 'forbidden_words', 'level_affectation'];

    protected $casts = [
        'forbidden_words' => 'array',
        'level_affectation' => 'integer',
    ];

    public function universe(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Universe::class);
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
