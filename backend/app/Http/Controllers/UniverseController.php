<?php

namespace App\Http\Controllers;

use App\Http\Requests\StoreUniverseRequest;
use App\Http\Requests\UpdateUniverseRequest;
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
            'universes' => Universe::with('cosmos:id,name')->withCount('characters')->orderBy('name')->get()->map(fn ($u) => [
                'id' => $u->id,
                'name' => $u->name,
                'cosmos_id' => $u->cosmos_id,
                'cosmos_name' => $u->cosmos?->name,
                'characters_count' => $u->characters_count,
            ]),
        ]);
    }

    /**
     * POST /universes
     * Crée un nouvel univers, éventuellement rattaché à un cosmos
     */
    public function store(StoreUniverseRequest $request): JsonResponse
    {
        $universe = Universe::create([
            'name' => $request->validated('name'),
            'slug' => $this->generateUniqueSlug($request->validated('name')),
            'cosmos_id' => $request->validated('cosmos_id'),
        ]);

        $universe->load('cosmos:id,name');

        return response()->json([
            'message' => 'Univers créé avec succès.',
            'universe' => [
                'id' => $universe->id,
                'name' => $universe->name,
                'cosmos_id' => $universe->cosmos_id,
                'cosmos_name' => $universe->cosmos?->name,
            ],
        ], 201);
    }

    /**
     * PATCH /universes/{universe}
     * Met à jour le cosmos d'un univers existant (backfill depuis la fiche personnage)
     */
    public function update(UpdateUniverseRequest $request, Universe $universe): JsonResponse
    {
        $universe->cosmos_id = $request->validated('cosmos_id');
        $universe->save();

        $universe->load('cosmos:id,name');

        return response()->json([
            'message' => 'Univers mis à jour avec succès.',
            'universe' => [
                'id' => $universe->id,
                'name' => $universe->name,
                'cosmos_id' => $universe->cosmos_id,
                'cosmos_name' => $universe->cosmos?->name,
            ],
        ]);
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
