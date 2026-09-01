<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Cosmos extends Model
{
    // "cosmos" est invariable : on fige le nom de table pour éviter toute
    // pluralisation hasardeuse ("cosmoses") par Eloquent.
    protected $table = 'cosmos';

    protected $fillable = ['name', 'slug'];

    public function universes(): \Illuminate\Database\Eloquent\Relations\HasMany
    {
        return $this->hasMany(Universe::class);
    }
}
