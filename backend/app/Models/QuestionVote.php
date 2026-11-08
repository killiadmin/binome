<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

/**
 * Vote d'un joueur éliminé pour la meilleure question d'un round
 * (voir QuestionVoteService). Un vote par éliminé et par round.
 */
class QuestionVote extends Model
{
    protected $fillable = ['round_id', 'action_id', 'voter_id'];

    public function round()
    {
        return $this->belongsTo(Round::class);
    }

    public function action()
    {
        return $this->belongsTo(Action::class);
    }

    public function voter()
    {
        return $this->belongsTo(Player::class, 'voter_id');
    }
}
