<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Photo de profil d'un joueur, rattachée à son PSEUDO et non à la ligne
 * `players` : RoomController::leave supprime le joueur quand il quitte un
 * salon, alors que le pseudo est l'identité durable du jeu (classement,
 * historique). La photo suit donc le joueur d'une partie à l'autre.
 *
 * L'image est stockée comme les images de personnages : data URI base64
 * (JPEG carré, redimensionné côté navigateur).
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::create('player_avatars', function (Blueprint $table) {
            $table->id();
            // Même collation que players.pseudo : « Killian » et « killian » partagent la photo.
            $table->string('pseudo')->unique();
            $table->longText('image');
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('player_avatars');
    }
};
