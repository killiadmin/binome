<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('universes', function (Blueprint $table) {
            $table->foreignId('cosmos_id')
                ->nullable()
                ->after('slug')
                ->constrained('cosmos')
                ->nullOnDelete();
        });
    }

    public function down(): void
    {
        Schema::table('universes', function (Blueprint $table) {
            $table->dropConstrainedForeignId('cosmos_id');
        });
    }
};
