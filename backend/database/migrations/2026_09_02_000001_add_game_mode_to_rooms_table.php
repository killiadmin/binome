<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('rooms', function (Blueprint $table) {
            $table->string('game_mode')->default('random')->after('max_players'); // random | cosmos
            $table->foreignId('cosmos_id')
                ->nullable()
                ->after('game_mode')
                ->constrained('cosmos')
                ->nullOnDelete();
        });
    }

    public function down(): void
    {
        Schema::table('rooms', function (Blueprint $table) {
            $table->dropConstrainedForeignId('cosmos_id');
            $table->dropColumn('game_mode');
        });
    }
};
