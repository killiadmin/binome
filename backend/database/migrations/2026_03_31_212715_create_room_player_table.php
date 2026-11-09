<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('room_player', function (Blueprint $table) {
            $table->id();
            $table->foreignId('room_id')->constrained()->onDelete('cascade');
            $table->foreignId('player_id')->constrained()->onDelete('cascade');
            $table->boolean('is_ready')->default(false);
            $table->timestamps();

            $table->unique(['room_id', 'player_id']);

            $table->index(['room_id', 'is_ready']);
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('room_player');
    }
};
