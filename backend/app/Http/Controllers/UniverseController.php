<?php

namespace App\Http\Controllers;

use App\Http\Requests\StoreUniverseRequest;
use App\Models\Universe;
use Illuminate\Http\JsonResponse;
use Illuminate\Routing\Controller;
use Illuminate\Support\Str;

class UniverseController extends Controller
{
    /**
     * GET /universes
     * Liste des univers existants (pour alimenter le formulaire de création de personnage)
     */
    public function index(): JsonResponse
    {
        return response()->json([
            'universes' => Universe::withCount('characters')->orderBy('name')->get()->map(fn($u) => [
                'id'              => $u->id,
                'name'            => $u->name,
                'characters_count'=> $u->characters_count,
            ]),
        ]);
    }

    /**
     * POST /universes
     * Crée un nouvel univers
     */
    public function store(StoreUniverseRequest $request): JsonResponse
    {
        $universe = Universe::create([
            'name' => $request->validated('name'),
            'slug' => $this->generateUniqueSlug($request->validated('name')),
        ]);

        return response()->json([
            'message'  => 'Univers créé avec succès.',
            'universe' => [
                'id'   => $universe->id,
                'name' => $universe->name,
            ],
        ], 201);
    }

    private function generateUniqueSlug(string $name): string
    {
        $slug = Str::slug($name);
        $original = $slug;
        $i = 2;

        while (Universe::where('slug', $slug)->exists()) {
            $slug = "{$original}-{$i}";
            $i++;
        }

        return $slug;
    }
}
