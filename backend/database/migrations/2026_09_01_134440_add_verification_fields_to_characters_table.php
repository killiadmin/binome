<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('characters', function (Blueprint $table) {
            // Personnage relu et confirmé par un humain. Défaut false : les personnages
            // proposés en masse (import / génération) doivent être validés à la main.
            $table->boolean('verif_manual')->default(false)->after('level_affectation');

            // Proposition refusée : on la garde en base pour ne jamais la reproposer,
            // mais elle n'apparaît nulle part et n'est jamais tirée en partie.
            $table->boolean('hidden')->default(false)->after('verif_manual');
        });

        // Les personnages déjà en base ont tous été créés à la main via la fiche : on les considère validés.
        DB::table('characters')->update(['verif_manual' => true]);
    }

    public function down(): void
    {
        Schema::table('characters', function (Blueprint $table) {
            $table->dropColumn(['verif_manual', 'hidden']);
        });
    }
};
