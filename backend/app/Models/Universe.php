<?php

namespace App\Models;
use App\Models\Binome;
use App\Models\Character;
use Illuminate\Database\Eloquent\Model;

class Universe extends Model
{
    public function characters(): \Illuminate\Database\Eloquent\Relations\HasMany
    {
        return $this->hasMany(Character::class);
    }

    public function binomes()
    {
        return $this->hasMany(Binome::class);
    }
}
