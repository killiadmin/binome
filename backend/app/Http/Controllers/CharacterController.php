<?php

namespace App\Http\Controllers;

use App\Http\Controllers\Concerns\EncodesUploadedImage;
use App\Http\Requests\StoreCharacterRequest;
use App\Models\Character;
use App\Models\Universe;
use Illuminate\Database\QueryException;
use Illuminate\Http\JsonResponse;
use Illuminate\Routing\Controller;
use Illuminate\Support\Str;

class CharacterController extends Controller
{
    use EncodesUploadedImage;

    /**
     * GET /characters
     * Liste tous les personnages (toutes univers confondus), pour la page de gestion
     */
    public function index(): JsonResponse
    {
        return response()->json([
            'characters' => Character::with('universe:id,name')->orderBy('name')->get()->map(fn($c) => [
                'id'              => $c->id,
                'universe_id'     => $c->universe_id,
                'universe_name'   => $c->universe->name,
                'name'            => $c->name,
                'image'           => $c->image,
                'forbidden_words' => $c->forbidden_words,
            ]),
        ]);
    }

    /**
     * POST /universes/{universe}/characters
     * Crée un personnage rattaché à un univers
     */
    public function store(StoreCharacterRequest $request, Universe $universe): JsonResponse
    {
        $character = Character::create([
            'universe_id'     => $universe->id,
            'name'            => $request->validated('name'),
            'slug'            => $this->generateUniqueSlug($universe, $request->validated('name')),
            'image'           => $this->encodeImage($request->file('image')),
            'forbidden_words' => $request->validated('forbidden_words'),
        ]);

        return response()->json([
            'message'   => 'Personnage créé avec succès.',
            'character' => [
                'id'              => $character->id,
                'universe_id'     => $character->universe_id,
                'name'            => $character->name,
                'image'           => $character->image,
                'forbidden_words' => $character->forbidden_words,
            ],
        ], 201);
    }

    /**
     * PUT /characters/{character}
     * Modifie un personnage existant (l'image n'est remplacée que si un nouveau fichier est envoyé)
     */
    public function update(StoreCharacterRequest $request, Character $character): JsonResponse
    {
        $name = $request->validated('name');

        if ($name !== $character->name) {
            $character->slug = $this->generateUniqueSlug($character->universe, $name, ignoreId: $character->id);
        }

        $character->name = $name;
        $character->forbidden_words = $request->validated('forbidden_words');

        if ($request->hasFile('image')) {
            $character->image = $this->encodeImage($request->file('image'));
        }

        $character->save();

        return response()->json([
            'message'   => 'Personnage modifié avec succès.',
            'character' => [
                'id'              => $character->id,
                'universe_id'     => $character->universe_id,
                'name'            => $character->name,
                'image'           => $character->image,
                'forbidden_words' => $character->forbidden_words,
            ],
        ]);
    }

    /**
     * DELETE /characters/{character}
     * Supprime un personnage (impossible s'il a déjà été utilisé dans une partie)
     */
    public function destroy(Character $character): JsonResponse
    {
        try {
            $character->delete();
        } catch (QueryException) {
            return response()->json([
                'message' => 'Ce personnage a déjà été utilisé dans une partie et ne peut pas être supprimé.',
            ], 409);
        }

        return response()->json([
            'message' => 'Personnage supprimé avec succès.',
        ]);
    }

    private function generateUniqueSlug(Universe $universe, string $name, ?int $ignoreId = null): string
    {
        $slug = Str::slug($name);
        $original = $slug;
        $i = 2;

        while (
            Character::where('universe_id', $universe->id)
                ->where('slug', $slug)
                ->when($ignoreId, fn($query) => $query->where('id', '!=', $ignoreId))
                ->exists()
        ) {
            $slug = "{$original}-{$i}";
            $i++;
        }

        return $slug;
    }
}
