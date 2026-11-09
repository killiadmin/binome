<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('actions', function (Blueprint $table) {
            $table->id();
            $table->foreignId('round_id')->constrained('rounds')->cascadeOnDelete();
            $table->foreignId('player_id')->constrained('players');
            $table->foreignId('target_player_id')->nullable()->constrained('players');
            $table->string('type');
            $table->text('question')->nullable();
            $table->foreignId('character_id')->nullable()->constrained('characters');
            $table->boolean('is_valid');
            $table->boolean('accusation_correct')->nullable();
            $table->timestamps();
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('actions');
    }
};
