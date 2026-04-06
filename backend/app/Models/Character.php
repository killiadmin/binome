<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use App\Models\Universe;

class Character extends Model
{
    protected $casts = ['forbidden_words' => 'array'];

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
