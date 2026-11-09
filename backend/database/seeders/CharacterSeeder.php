<?php

namespace Database\Seeders;

use App\Support\CharacterImporter;
use Illuminate\Database\Seeder;

class CharacterSeeder extends Seeder
{
    public function run(): void
    {
        $file = database_path('data/characters.json');

        if (! is_file($file)) {
            $this->command?->warn("Aucun fichier {$file} — CharacterSeeder ignoré.");

            return;
        }

        $importer = new CharacterImporter;
        $importer->importFile($file);

        $this->command?->info(sprintf(
            'Personnages : %d créé(s), %d mis à jour, %d ignoré(s), %d en erreur.',
            count($importer->created),
            count($importer->updated),
            count($importer->skipped),
            count($importer->errors),
        ));

        foreach ($importer->errors as $error) {
            $this->command?->warn("  #{$error['index']} {$error['name']} : {$error['message']}");
        }
    }
}
