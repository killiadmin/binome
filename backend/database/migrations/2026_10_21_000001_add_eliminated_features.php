<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

/**
 * Occuper les joueurs éliminés (options de l'hôte, actives par défaut) :
 *
 * - rooms.reveal_to_eliminated : un éliminé voit tous les personnages et binomes
 *   (GET /games/{game}/reveal)
 * - rooms.best_question_vote   : les éliminés votent pour la meilleure question
 *   de chaque round → question_votes, +1 point par round remporté
 *   (game_stats.best_questions)
 */
return new class extends Migration
{
    public function up(): void
    {
        Schema::table('rooms', function (Blueprint $table) {
            $table->boolean('reveal_to_eliminated')->default(true)->after('cosmos_id');
            $table->boolean('best_question_vote')->default(true)->after('reveal_to_eliminated');
        });

        Schema::create('question_votes', function (Blueprint $table) {
            $table->id();
            $table->foreignId('round_id')->constrained('rounds')->cascadeOnDelete();
            $table->foreignId('action_id')->constrained('actions')->cascadeOnDelete();
            $table->foreignId('voter_id')->constrained('players')->cascadeOnDelete();
            $table->timestamps();

            // Un vote par éliminé et par round (modifiable)
            $table->unique(['round_id', 'voter_id']);
        });

        Schema::table('game_stats', function (Blueprint $table) {
            $table->unsignedInteger('best_questions')->default(0)->after('eliminations');
        });
    }

    public function down(): void
    {
        Schema::table('game_stats', function (Blueprint $table) {
            $table->dropColumn('best_questions');
        });

        Schema::dropIfExists('question_votes');

        Schema::table('rooms', function (Blueprint $table) {
            $table->dropColumn(['reveal_to_eliminated', 'best_question_vote']);
        });
    }
};
