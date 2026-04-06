<?php

namespace App\Models;
use App\Models\Binome;
use Illuminate\Database\Eloquent\Model;
use App\Models\Room;
use App\Models\Round;

class Game extends Model
{
    public function room()
    {
        return $this->belongsTo(Room::class);
    }

    public function binomes()
    {
        return $this->hasMany(Binome::class);
    }

    public function rounds()
    {
        return $this->hasMany(Round::class);
    }

    public function currentRound()
    {
        return $this->hasOne(Round::class)->where('is_finished', false)->latest();
    }
}
