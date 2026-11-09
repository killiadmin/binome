<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('chat_messages', function (Blueprint $table) {
            $table->id();
            // Le joueur est supprimé quand il quitte un salon (RoomController::leave)
            // : on conserve le message, d'où le nullOnDelete + author_name dénormalisé.
            $table->foreignId('player_id')->nullable()->constrained('players')->nullOnDelete();
            $table->string('author_name');
            // Identité stable côté client ("p:{playerId}" ou "a:{anonId}") : sert au
            // front à reconnaître ses propres messages même après suppression du joueur.
            $table->string('author_key', 32)->index();
            $table->text('body');
            $table->timestamps();

            $table->index('created_at');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('chat_messages');
    }
};
