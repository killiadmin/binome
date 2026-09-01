<?php

namespace App\Console\Commands;

use App\Support\CharacterImporter;
use Illuminate\Console\Command;

class ImportCharacters extends Command
{
    protected $signature = 'characters:import
        {file : Chemin du fichier JSON à importer}
        {--images-dir= : Dossier des images référencées par nom de fichier (défaut : database/data/images)}
        {--skip-existing : Ne pas modifier les personnages déjà présents}';

    protected $description = 'Importe des personnages en masse depuis un fichier JSON (crée les univers et cosmos manquants)';

    public function handle(): int
    {
        $importer = new CharacterImporter(
            imagesDir: $this->option('images-dir'),
            skipExisting: (bool) $this->option('skip-existing'),
            onProgress: fn (string $name) => $this->line("  <fg=gray>·</> {$name}"),
        );

        try {
            $importer->importFile($this->argument('file'));
        } catch (\Throwable $e) {
            $this->components->error($e->getMessage());

            return self::FAILURE;
        }

        $this->newLine();
        $this->components->info(sprintf(
            '%d créé(s), %d mis à jour, %d ignoré(s), %d en erreur.',
            count($importer->created),
            count($importer->updated),
            count($importer->skipped),
            count($importer->errors),
        ));

        foreach ($importer->errors as $error) {
            $label = $error['name'] ? "#{$error['index']} {$error['name']}" : "ligne #{$error['index']}";
            $this->components->twoColumnDetail("<fg=red>{$label}</>", $error['message']);
        }

        return $importer->errors === [] ? self::SUCCESS : self::FAILURE;
    }
}
