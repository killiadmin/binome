<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;

/**
 * Toutes les réactions passent en base, emojis Unicode compris : l'admin
 * peut supprimer ou ajouter n'importe laquelle (onglet Emojis). Une réaction
 * est soit `emoji` (caractère Unicode), soit `image` (photo d'un joueur).
 * Les 8 emojis jusque-là codés en dur (GameController::REACTIONS) sont repris.
 */
return new class extends Migration
{
    private const DEFAULTS = [
        '😂' => 'Mort de rire',
        '🤔' => 'Réflexion',
        '😱' => 'Choc',
        '👏' => 'Bravo',
        '🔥' => 'Feu',
        '🤫' => 'Chut',
        '😈' => 'Diabolique',
        '💀' => 'Mort',
    ];

    public function up(): void
    {
        Schema::table('custom_reactions', function (Blueprint $table) {
            // Collation binaire : en utf8mb4_0900_ai_ci, tous les emojis
            // sont « égaux » (😂 = 🔥) et le contrôle de doublon échouerait.
            $table->string('emoji', 32)->collation('utf8mb4_bin')->nullable()->after('name');
            $table->longText('image')->nullable()->change();
            $table->string('created_by_pseudo')->nullable()->change();
        });

        $now = now();
        foreach (self::DEFAULTS as $emoji => $name) {
            if (! DB::table('custom_reactions')->where('emoji', $emoji)->exists()) {
                DB::table('custom_reactions')->insert([
                    'name' => $name,
                    'emoji' => $emoji,
                    'is_enabled' => true,
                    'created_at' => $now,
                    'updated_at' => $now,
                ]);
            }
        }
    }

    public function down(): void
    {
        DB::table('custom_reactions')->whereNull('image')->delete();

        Schema::table('custom_reactions', function (Blueprint $table) {
            $table->dropColumn('emoji');
            $table->longText('image')->nullable(false)->change();
            $table->string('created_by_pseudo')->nullable(false)->change();
        });
    }
};
