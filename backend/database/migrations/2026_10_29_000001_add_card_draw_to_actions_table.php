<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Tirage de carte après un « Je ne sais pas » : le questionneur retourne une
 * carte face cachée parmi les mots interdits de la cible (ActionService).
 *
 * - card_draw_status   : pending → drawn → shared | kept, ou expired (délai dépassé)
 * - card_count         : nombre de cartes proposées (= mots interdits de la cible)
 * - card_draw_deadline : au-delà, le tirage est perdu et le tour avance
 * - drawn_word         : SECRET — jamais sérialisé (Action::$hidden)
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('actions', function (Blueprint $table) {
            $table->string('card_draw_status', 16)->nullable()->after('answer');
            $table->unsignedTinyInteger('card_count')->nullable()->after('card_draw_status');
            $table->timestamp('card_draw_deadline')->nullable()->after('card_count');
            $table->string('drawn_word')->nullable()->after('card_draw_deadline');
        });
    }

    public function down(): void
    {
        Schema::table('actions', function (Blueprint $table) {
            $table->dropColumn(['card_draw_status', 'card_count', 'card_draw_deadline', 'drawn_word']);
        });
    }
};
