<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Trace des corrections de réponse par l'hôte (ActionService::correctAnswer).
 * Colonnes ajoutées vides : les actions existantes ne sont pas touchées.
 *
 * - answer_corrected_from : réponse donnée à l'origine par le joueur (yes | no)
 * - answer_corrected_at   : date de la dernière correction
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('actions', function (Blueprint $table) {
            $table->string('answer_corrected_from', 16)->nullable()->after('answer');
            $table->timestamp('answer_corrected_at')->nullable()->after('answer_corrected_from');
        });
    }

    public function down(): void
    {
        Schema::table('actions', function (Blueprint $table) {
            $table->dropColumn(['answer_corrected_from', 'answer_corrected_at']);
        });
    }
};
