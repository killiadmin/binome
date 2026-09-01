<?php

namespace App\Support;

use App\Models\Character;
use App\Models\Cosmos;
use App\Models\Universe;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Validator;
use Illuminate\Support\Str;

/**
 * Import de personnages en masse depuis un tableau (issu d'un JSON en général).
 *
 * Idempotent : un personnage est identifié par (univers, nom). Relancer l'import
 * met à jour les personnages déjà présents plutôt que de les dupliquer.
 * Les univers et cosmos référencés sont créés à la volée s'ils n'existent pas.
 *
 * Partagé entre la commande `characters:import` et le `CharacterSeeder`.
 */
class CharacterImporter
{
    private const MAX_IMAGE_BYTES = 4 * 1024 * 1024;

    private const IMAGE_MIMES = ['image/jpeg', 'image/png', 'image/gif', 'image/webp'];

    private const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif'];

    /** @var list<array{name:string}> */
    public array $created = [];

    /** @var list<array{name:string}> */
    public array $updated = [];

    /** @var list<array{name:string,reason:string}> */
    public array $skipped = [];

    /** @var list<array{index:int,name:?string,message:string}> */
    public array $errors = [];

    private string $imagesDir;

    /**
     * @param  string|null  $imagesDir  Dossier où chercher les images référencées par nom de fichier
     *                                  (défaut : database/data/images)
     * @param  bool  $skipExisting  Laisser intacts les personnages déjà en base
     * @param  (callable(string):void)|null  $onProgress  Appelé avec le nom de chaque personnage traité
     */
    public function __construct(
        ?string $imagesDir = null,
        private bool $skipExisting = false,
        private $onProgress = null,
    ) {
        $this->imagesDir = rtrim($imagesDir ?: database_path('data/images'), '/');
    }

    public function importFile(string $path): void
    {
        if (! is_file($path)) {
            throw new \InvalidArgumentException("Fichier introuvable : {$path}");
        }

        $rows = json_decode(file_get_contents($path), true, flags: JSON_THROW_ON_ERROR);

        if (! is_array($rows)) {
            throw new \InvalidArgumentException('Le fichier JSON doit contenir un tableau de personnages.');
        }

        $this->import($rows);
    }

    /**
     * @param  array<int,mixed>  $rows
     */
    public function import(array $rows): void
    {
        foreach (array_values($rows) as $index => $row) {
            try {
                $this->importRow($row);
            } catch (\Throwable $e) {
                $this->errors[] = [
                    'index' => $index,
                    'name' => is_array($row) && isset($row['name']) && is_string($row['name']) ? $row['name'] : null,
                    'message' => $e->getMessage(),
                ];
            }
        }
    }

    private function importRow(mixed $row): void
    {
        if (! is_array($row)) {
            throw new \RuntimeException('Entrée invalide (objet attendu).');
        }

        $data = Validator::make($row, [
            'name' => ['required', 'string', 'min:2', 'max:50'],
            'universe' => ['required', 'string', 'min:2', 'max:50'],
            'cosmos' => ['nullable', 'string', 'min:2', 'max:50'],
            'forbidden_words' => ['required', 'array', 'size:3'],
            'forbidden_words.*' => ['required', 'string', 'min:1', 'max:50'],
            'level_affectation' => ['required', 'integer', 'min:1'],
            'image' => ['nullable', 'string'],
        ])->validate();

        $cosmos = $this->resolveCosmos($data['cosmos'] ?? null);
        $universe = $this->resolveUniverse($data['universe'], $cosmos);

        $character = Character::where('universe_id', $universe->id)
            ->where('name', $data['name'])
            ->first();

        // Une proposition refusée (hidden) n'est jamais recréée ni modifiée.
        if ($character && $character->hidden) {
            $this->skipped[] = ['name' => $data['name'], 'reason' => 'refusé précédemment'];

            return;
        }

        if ($character && $this->skipExisting) {
            $this->skipped[] = ['name' => $data['name'], 'reason' => 'déjà présent'];

            return;
        }

        $this->assertLevelAvailable($universe, (int) $data['level_affectation'], $character?->id);

        $image = $this->resolveImage($data['image'] ?? null, Str::slug($data['name'])) ?? $character?->image;

        if ($character) {
            $character->fill([
                'forbidden_words' => array_values($data['forbidden_words']),
                'level_affectation' => (int) $data['level_affectation'],
                'image' => $image,
            ])->save();

            $this->updated[] = ['name' => $data['name']];
        } else {
            Character::create([
                'universe_id' => $universe->id,
                'name' => $data['name'],
                'slug' => $this->uniqueSlug($universe, $data['name']),
                'forbidden_words' => array_values($data['forbidden_words']),
                'level_affectation' => (int) $data['level_affectation'],
                'image' => $image,
                // Proposition : à valider manuellement avant d'être jouable.
                'verif_manual' => false,
                'hidden' => false,
            ]);

            $this->created[] = ['name' => $data['name']];
        }

        if ($this->onProgress) {
            ($this->onProgress)($data['name']);
        }
    }

    private function resolveCosmos(?string $name): ?Cosmos
    {
        $name = trim((string) $name);

        if ($name === '') {
            return null;
        }

        return Cosmos::firstOrCreate(['slug' => Str::slug($name)], ['name' => $name]);
    }

    private function resolveUniverse(string $name, ?Cosmos $cosmos): Universe
    {
        $universe = Universe::firstOrCreate(
            ['slug' => Str::slug($name)],
            ['name' => $name, 'cosmos_id' => $cosmos?->id],
        );

        // Rattache le cosmos si l'univers existait déjà sans (backfill) ou en a un différent.
        if ($cosmos && $universe->cosmos_id !== $cosmos->id) {
            $universe->update(['cosmos_id' => $cosmos->id]);
        }

        return $universe;
    }

    private function assertLevelAvailable(Universe $universe, int $level, ?int $ignoreId): void
    {
        $count = Character::where('universe_id', $universe->id)
            ->where('level_affectation', $level)
            ->when($ignoreId, fn ($q) => $q->where('id', '!=', $ignoreId))
            ->count();

        if ($count >= 2) {
            throw new \RuntimeException(
                "L'univers « {$universe->name} » a déjà 2 personnages au niveau d'affectation {$level}."
            );
        }
    }

    private function uniqueSlug(Universe $universe, string $name): string
    {
        $base = Str::slug($name);
        $slug = $base;
        $i = 2;

        while (Character::where('universe_id', $universe->id)->where('slug', $slug)->exists()) {
            $slug = "{$base}-{$i}";
            $i++;
        }

        return $slug;
    }

    /**
     * Résout le champ `image` en data URI base64 (le même format que l'upload manuel).
     * Accepte : une data URI déjà encodée, une URL http(s), un chemin de fichier
     * (absolu ou relatif au dossier d'images). À défaut, cherche par convention
     * un fichier `<slug-du-nom>.(png|jpg|jpeg|webp|gif)` dans le dossier d'images.
     */
    private function resolveImage(?string $ref, string $slug): ?string
    {
        $ref = trim((string) $ref);

        if ($ref !== '') {
            if (str_starts_with($ref, 'data:')) {
                return $ref;
            }

            if (Str::startsWith($ref, ['http://', 'https://'])) {
                return $this->downloadImage($ref);
            }

            return $this->encodeLocalImage($this->resolveLocalPath($ref));
        }

        foreach (self::IMAGE_EXTENSIONS as $ext) {
            $candidate = "{$this->imagesDir}/{$slug}.{$ext}";

            if (is_file($candidate)) {
                return $this->encodeLocalImage($candidate);
            }
        }

        return null;
    }

    private function resolveLocalPath(string $ref): string
    {
        if (is_file($ref)) {
            return $ref;
        }

        return "{$this->imagesDir}/".ltrim($ref, '/');
    }

    private function encodeLocalImage(string $path): string
    {
        if (! is_file($path)) {
            throw new \RuntimeException("Image introuvable : {$path}");
        }

        if (filesize($path) > self::MAX_IMAGE_BYTES) {
            throw new \RuntimeException("Image trop lourde (> 4 Mo) : {$path}");
        }

        $mime = mime_content_type($path) ?: 'image/png';
        $this->assertImageMime($mime, $path);

        return "data:{$mime};base64,".base64_encode(file_get_contents($path));
    }

    private function downloadImage(string $url): string
    {
        $response = Http::timeout(20)->get($url);

        if ($response->failed()) {
            throw new \RuntimeException("Téléchargement de l'image échoué (HTTP {$response->status()}) : {$url}");
        }

        if (strlen($response->body()) > self::MAX_IMAGE_BYTES) {
            throw new \RuntimeException("Image trop lourde (> 4 Mo) : {$url}");
        }

        $mime = Str::before($response->header('Content-Type') ?: 'image/png', ';');
        $this->assertImageMime($mime, $url);

        return "data:{$mime};base64,".base64_encode($response->body());
    }

    private function assertImageMime(string $mime, string $source): void
    {
        if (! in_array($mime, self::IMAGE_MIMES, true)) {
            throw new \RuntimeException("Type d'image non supporté ({$mime}) : {$source}");
        }
    }
}
