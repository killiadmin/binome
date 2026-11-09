<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Emojis de réaction créés par les joueurs (photo prise ou importée, recadrée
 * en carré côté navigateur), en plus des emojis Unicode fixes de
 * GameController::REACTIONS.
 *
 * Rattachés au PSEUDO de l'auteur, comme player_avatars : la ligne `players`
 * peut disparaître (RoomController::leave), pas le pseudo. Actifs dès leur
 * création ; un administrateur peut les masquer (is_enabled) ou les supprimer.
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::create('custom_reactions', function (Blueprint $table) {
            $table->id();
            $table->string('name', 20);
            $table->longText('image');
            $table->string('created_by_pseudo');
            $table->boolean('is_enabled')->default(true);
            $table->timestamps();

            $table->index(['is_enabled', 'created_at']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('custom_reactions');
    }
};
