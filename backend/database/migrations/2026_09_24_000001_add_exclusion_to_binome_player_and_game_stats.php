<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Exclusion d'un joueur déconnecté par l'hôte (ActionService::excludePlayer) :
 * l'exclu et son binôme sont éliminés, l'exclu marque 0 point.
 *
 * - binome_player.is_excluded      : c'est ce joueur que l'hôte a exclu
 * - binome_player.eliminated_round : round de l'élimination hors accusation
 *   (exclu + son binôme), pour calculer les rounds survécus
 * - game_stats.is_excluded         : affiché au tableau des scores / historique
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('binome_player', function (Blueprint $table) {
            $table->boolean('is_excluded')->default(false)->after('is_eliminated');
            $table->unsignedInteger('eliminated_round')->nullable()->after('is_excluded');
        });

        Schema::table('game_stats', function (Blueprint $table) {
            $table->boolean('is_excluded')->default(false)->after('is_eliminated');
        });
    }

    public function down(): void
    {
        Schema::table('binome_player', function (Blueprint $table) {
            $table->dropColumn(['is_excluded', 'eliminated_round']);
        });

        Schema::table('game_stats', function (Blueprint $table) {
            $table->dropColumn('is_excluded');
        });
    }
};
