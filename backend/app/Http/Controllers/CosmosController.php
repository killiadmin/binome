<?php

namespace App\Http\Controllers;

use App\Http\Requests\StoreCosmosRequest;
use App\Models\Cosmos;
use Illuminate\Http\JsonResponse;
use Illuminate\Routing\Controller;
use Illuminate\Support\Str;

class CosmosController extends Controller
{
    /**
     * GET /cosmos
     * Liste des cosmos existants (pour alimenter le formulaire de création d'univers)
     */
    public function index(): JsonResponse
    {
        return response()->json([
            'cosmos' => Cosmos::withCount('universes')->orderBy('name')->get()->map(fn ($c) => [
                'id' => $c->id,
                'name' => $c->name,
                'universes_count' => $c->universes_count,
            ]),
        ]);
    }

    /**
     * POST /cosmos
     * Crée un nouveau cosmos (sur-catégorie regroupant plusieurs univers)
     */
    public function store(StoreCosmosRequest $request): JsonResponse
    {
        $cosmos = Cosmos::create([
            'name' => $request->validated('name'),
            'slug' => $this->generateUniqueSlug($request->validated('name')),
        ]);

        return response()->json([
            'message' => 'Cosmos créé avec succès.',
            'cosmos' => [
                'id' => $cosmos->id,
                'name' => $cosmos->name,
            ],
        ], 201);
    }

    private function generateUniqueSlug(string $name): string
    {
        $slug = Str::slug($name);
        $original = $slug;
        $i = 2;

        while (Cosmos::where('slug', $slug)->exists()) {
            $slug = "{$original}-{$i}";
            $i++;
        }

        return $slug;
    }
}
