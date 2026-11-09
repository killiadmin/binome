<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Une demande de changement n'est plus immédiate : c'est un vote visible,
     * comme « prêt ». Le retirage n'a lieu qu'une fois que tous ont voté.
     * null = pas encore voté | 'accepted' | 'change'
     */
    public function up(): void
    {
        Schema::table('binome_player', function (Blueprint $table) {
            $table->string('character_vote', 10)->nullable()->after('character_id');
        });

        DB::table('binome_player')->where('character_accepted', true)->update(['character_vote' => 'accepted']);

        Schema::table('binome_player', function (Blueprint $table) {
            $table->dropColumn('character_accepted');
        });
    }

    public function down(): void
    {
        Schema::table('binome_player', function (Blueprint $table) {
            $table->boolean('character_accepted')->default(false)->after('character_id');
        });

        DB::table('binome_player')->where('character_vote', 'accepted')->update(['character_accepted' => true]);

        Schema::table('binome_player', function (Blueprint $table) {
            $table->dropColumn('character_vote');
        });
    }
};
