<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('game_stats', function (Blueprint $table) {
            $table->id();
            $table->foreignId('game_id')->constrained('games')->cascadeOnDelete();
            $table->string('player_pseudo');
            $table->string('character_name');
            $table->integer('eliminations')->default(0);
            $table->integer('rounds_survived')->default(0);
            $table->boolean('survived_full_game')->default(false);
            $table->integer('score')->default(0);
            $table->boolean('is_winner')->default(false);
            $table->boolean('is_eliminated')->default(false);
            $table->timestamps();
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('game_stats');
    }
};
