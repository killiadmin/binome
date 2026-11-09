<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Vote des personnages avant le round 1 : chaque joueur accepte le sien,
     * ou demande un nouveau tirage pour tout le monde (GameService::voteCharacter).
     */
    public function up(): void
    {
        Schema::table('games', function (Blueprint $table) {
            // null = vote en cours ; renseigné quand tous ont accepté (round 1 créé)
            $table->timestamp('characters_confirmed_at')->nullable()->after('status');
            $table->unsignedSmallInteger('draw_number')->default(1)->after('characters_confirmed_at');
            // Personnages refusés pendant le vote : écartés des tirages suivants
            $table->json('rejected_character_ids')->nullable()->after('draw_number');
        });

        Schema::table('binome_player', function (Blueprint $table) {
            $table->boolean('character_accepted')->default(false)->after('character_id');
        });

        // Les parties existantes ont démarré sans vote : leurs personnages sont acquis.
        DB::table('games')->update(['characters_confirmed_at' => DB::raw('created_at')]);
    }

    public function down(): void
    {
        Schema::table('binome_player', function (Blueprint $table) {
            $table->dropColumn('character_accepted');
        });

        Schema::table('games', function (Blueprint $table) {
            $table->dropColumn(['characters_confirmed_at', 'draw_number', 'rejected_character_ids']);
        });
    }
};
