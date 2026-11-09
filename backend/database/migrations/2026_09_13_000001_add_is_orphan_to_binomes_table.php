<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Un binome « orphelin » ne contient qu'un seul joueur : c'est le joueur
     * surnuméraire quand la partie démarre avec un nombre impair de joueurs.
     * Le flag distingue ce solo voulu d'un binome brisé par une élimination.
     */
    public function up(): void
    {
        Schema::table('binomes', function (Blueprint $table) {
            $table->boolean('is_orphan')->default(false)->after('universe_id');
        });
    }

    public function down(): void
    {
        Schema::table('binomes', function (Blueprint $table) {
            $table->dropColumn('is_orphan');
        });
    }
};
