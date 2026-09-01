<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Universe extends Model
{
    protected $fillable = ['name', 'slug', 'cosmos_id'];

    public function cosmos(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Cosmos::class);
    }

    public function characters(): \Illuminate\Database\Eloquent\Relations\HasMany
    {
        return $this->hasMany(Character::class);
    }

    public function binomes()
    {
        return $this->hasMany(Binome::class);
    }
}
