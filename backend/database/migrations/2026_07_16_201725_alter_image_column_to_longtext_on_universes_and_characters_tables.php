<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Support\Facades\DB;

return new class extends Migration
{
    public function up(): void
    {
        DB::statement('ALTER TABLE universes MODIFY image LONGTEXT NULL');
        DB::statement('ALTER TABLE characters MODIFY image LONGTEXT NULL');
    }

    public function down(): void
    {
        DB::statement('ALTER TABLE universes MODIFY image VARCHAR(255) NULL');
        DB::statement('ALTER TABLE characters MODIFY image VARCHAR(255) NULL');
    }
};
