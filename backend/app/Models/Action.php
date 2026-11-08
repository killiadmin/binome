<?php

namespace App\Models;

use App\Enums\ActionType;
use Illuminate\Database\Eloquent\Model;

class Action extends Model
{
    protected $fillable = [
        'round_id',
        'player_id',
        'target_player_id',
        'type',
        'content',
        'character_name',
        'is_valid',
        'accusation_correct',
        'accusation_confirmed',
        'answer',
        'answer_corrected_from',
        'answer_corrected_at',
        'card_draw_status',
        'card_count',
        'card_draw_deadline',
        'drawn_word',
    ];

    // Le mot tiré n'est connu que du questionneur (réponse HTTP du tirage) et
    // des joueurs à qui il le montre (channel privé) : jamais dans un toArray().
    protected $hidden = ['drawn_word'];

    protected $casts = [
        'type'                 => ActionType::class,
        'is_valid'             => 'boolean',
        'accusation_correct'   => 'boolean',
        'accusation_confirmed' => 'boolean',
        'card_draw_deadline'   => 'datetime',
        'answer_corrected_at'  => 'datetime',
    ];

    /**
     * État du tirage tel que le voient TOUS les joueurs : partagé, gardé ou
     * expiré se confondent en « done », la cible ne doit pas savoir si son mot
     * a circulé. null si aucun tirage.
     */
    public function publicCardDraw(): ?array
    {
        if (! $this->card_draw_status) {
            return null;
        }

        return [
            'status'     => in_array($this->card_draw_status, ['pending', 'drawn'], true) ? $this->card_draw_status : 'done',
            'card_count' => $this->card_count,
            'deadline'   => $this->card_draw_deadline?->toIso8601String(),
        ];
    }

    public function round(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Round::class);
    }

    public function player(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Player::class);
    }

    public function targetPlayer(): \Illuminate\Database\Eloquent\Relations\BelongsTo
    {
        return $this->belongsTo(Player::class, 'target_player_id');
    }
}
