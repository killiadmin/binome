<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        // Le classement agrège game_stats GROUP BY player_pseudo.
        Schema::table('game_stats', function (Blueprint $table) {
            $table->index('player_pseudo');
        });

        // L'historique public ne liste que les parties terminées.
        Schema::table('games', function (Blueprint $table) {
            $table->index('status');
        });
    }

    public function down(): void
    {
        Schema::table('game_stats', function (Blueprint $table) {
            $table->dropIndex(['player_pseudo']);
        });

        Schema::table('games', function (Blueprint $table) {
            $table->dropIndex(['status']);
        });
    }
};
