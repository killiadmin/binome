<?php
// database/migrations/xxxx_xx_xx_000001_create_rooms_table.php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('rooms', function (Blueprint $table) {
            $table->id();
            $table->string('code', 6)->unique();
            $table->boolean('is_private')->default(false);
            $table->boolean('is_locked')->default(false);
            $table->integer('max_players')->default(8);
            $table->foreignId('created_by')->constrained('players')->onDelete('cascade');
            $table->timestamps();

            $table->index('code');
        });
    }

    public function down(): void
    {
        Schema::dropIfExists('rooms');
    }
};
