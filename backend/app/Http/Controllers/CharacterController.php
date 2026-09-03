<?php

namespace App\Http\Controllers;

use App\Http\Controllers\Concerns\EncodesUploadedImage;
use App\Http\Requests\StoreCharacterRequest;
use App\Models\Character;
use App\Models\Universe;
use Illuminate\Database\QueryException;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
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
            'characters' => Character::with(['universe:id,name,cosmos_id', 'universe.cosmos:id,name'])
                ->orderBy('name')
                ->get()
                ->map(fn ($c) => [
                    'id' => $c->id,
                    'universe_id' => $c->universe_id,
                    'universe_name' => $c->universe->name,
                    'cosmos_id' => $c->universe->cosmos_id,
                    'cosmos_name' => $c->universe->cosmos?->name,
                    'name' => $c->name,
                    'image' => $c->image,
                    'forbidden_words' => $c->forbidden_words,
                    'level_affectation' => $c->level_affectation,
                    'verif_manual' => $c->verif_manual,
                    'hidden' => $c->hidden,
                ]),
        ]);
    }

    /**
     * GET /characters/pending
     * Binômes proposés (import / génération) en attente de validation manuelle,
     * regroupés par univers + niveau d'affectation.
     */
    public function pending(): JsonResponse
    {
        $binomes = Character::pendingValidation()
            ->with(['universe:id,name,cosmos_id', 'universe.cosmos:id,name'])
            ->orderBy('universe_id')
            ->orderBy('level_affectation')
            ->orderBy('id')
            ->get()
            ->groupBy(fn ($c) => $c->universe_id.'-'.$c->level_affectation)
            ->values()
            ->map(function ($chars) {
                $first = $chars->first();

                return [
                    'key' => $first->universe_id.'-'.$first->level_affectation,
                    'universe_id' => $first->universe_id,
                    'universe_name' => $first->universe->name,
                    'cosmos_name' => $first->universe->cosmos?->name,
                    'level_affectation' => $first->level_affectation,
                    'characters' => $chars->map(fn ($c) => [
                        'id' => $c->id,
                        'name' => $c->name,
                        'forbidden_words' => $c->forbidden_words,
                        'image' => $c->image,
                    ])->values(),
                ];
            });

        return response()->json(['binomes' => $binomes]);
    }

    /**
     * POST /characters/validation/accept
     * Valide un binôme proposé : verif_manual = true (chaque personnage doit déjà avoir une image).
     */
    public function accept(Request $request): JsonResponse
    {
        $data = $request->validate([
            'character_ids' => ['required', 'array', 'min:1'],
            'character_ids.*' => ['integer'],
        ]);

        $characters = Character::pendingValidation()->whereIn('id', $data['character_ids'])->get();

        if ($characters->isEmpty()) {
            return response()->json(['message' => 'Aucun personnage en attente pour ces identifiants.'], 422);
        }

        if ($missing = $characters->first(fn ($c) => empty($c->image))) {
            return response()->json([
                'message' => "Chaque personnage doit avoir une image avant validation (manquante : « {$missing->name} »).",
            ], 422);
        }

        Character::whereIn('id', $characters->pluck('id'))->update(['verif_manual' => true]);

        return response()->json([
            'message' => 'Binôme validé.',
            'validated_ids' => $characters->pluck('id')->values(),
        ]);
    }

    /**
     * POST /characters/validation/reject
     * Refuse un binôme proposé : hidden = true (conservé en base pour ne jamais le reproposer).
     */
    public function reject(Request $request): JsonResponse
    {
        $data = $request->validate([
            'character_ids' => ['required', 'array', 'min:1'],
            'character_ids.*' => ['integer'],
        ]);

        $ids = Character::pendingValidation()->whereIn('id', $data['character_ids'])->pluck('id');

        if ($ids->isEmpty()) {
            return response()->json(['message' => 'Aucun personnage en attente pour ces identifiants.'], 422);
        }

        Character::whereIn('id', $ids)->update(['hidden' => true]);

        return response()->json([
            'message' => 'Binôme refusé.',
            'rejected_ids' => $ids->values(),
        ]);
    }

    /**
     * POST /characters/{character}/image
     * Importe / remplace l'image d'un personnage (utilisé par la popup de validation).
     */
    public function uploadImage(Request $request, Character $character): JsonResponse
    {
        $request->validate([
            'image' => ['required', 'image', 'mimes:jpeg,jpg,png,gif,webp', 'max:4096'],
        ]);

        $character->image = $this->encodeImage($request->file('image'));
        $character->save();

        return response()->json([
            'message' => 'Image importée.',
            'character' => ['id' => $character->id, 'image' => $character->image],
        ]);
    }

    /**
     * PATCH /characters/{character}/forbidden-words
     * Met à jour uniquement les 3 mots interdits (édition rapide depuis la popup de validation).
     */
    public function updateForbiddenWords(Request $request, Character $character): JsonResponse
    {
        $data = $request->validate([
            'forbidden_words' => ['required', 'array', 'size:3'],
            'forbidden_words.*' => ['required', 'string', 'min:1', 'max:50'],
        ]);

        $character->forbidden_words = array_values($data['forbidden_words']);
        $character->save();

        return response()->json([
            'message' => 'Mots interdits mis à jour.',
            'character' => ['id' => $character->id, 'forbidden_words' => $character->forbidden_words],
        ]);
    }

    /**
     * POST /universes/{universe}/characters
     * Crée un personnage rattaché à un univers
     */
    public function store(StoreCharacterRequest $request, Universe $universe): JsonResponse
    {
        $character = Character::create([
            'universe_id' => $universe->id,
            'name' => $request->validated('name'),
            'slug' => $this->generateUniqueSlug($universe, $request->validated('name')),
            'image' => $this->encodeImage($request->file('image')),
            'forbidden_words' => $request->validated('forbidden_words'),
            'level_affectation' => $request->validated('level_affectation'),
            // Créé à la main via la fiche : considéré validé d'emblée.
            'verif_manual' => true,
        ]);

        return response()->json([
            'message' => 'Personnage créé avec succès.',
            'character' => [
                'id' => $character->id,
                'universe_id' => $character->universe_id,
                'name' => $character->name,
                'image' => $character->image,
                'forbidden_words' => $character->forbidden_words,
                'level_affectation' => $character->level_affectation,
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

        $newUniverseId = $request->validated('universe_id') ?? $character->universe_id;
        $universeChanged = (int) $newUniverseId !== (int) $character->universe_id;
        $universe = $universeChanged ? Universe::findOrFail($newUniverseId) : $character->universe;

        // Le slug est unique par univers : on le régénère si le nom OU l'univers change.
        if ($name !== $character->name || $universeChanged) {
            $character->slug = $this->generateUniqueSlug($universe, $name, ignoreId: $character->id);
        }

        $character->universe_id = $universe->id;
        $character->name = $name;
        $character->forbidden_words = $request->validated('forbidden_words');
        $character->level_affectation = $request->validated('level_affectation');

        if ($request->hasFile('image')) {
            $character->image = $this->encodeImage($request->file('image'));
        }

        $character->save();

        return response()->json([
            'message' => 'Personnage modifié avec succès.',
            'character' => [
                'id' => $character->id,
                'universe_id' => $character->universe_id,
                'name' => $character->name,
                'image' => $character->image,
                'forbidden_words' => $character->forbidden_words,
                'level_affectation' => $character->level_affectation,
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
                ->when($ignoreId, fn ($query) => $query->where('id', '!=', $ignoreId))
                ->exists()
        ) {
            $slug = "{$original}-{$i}";
            $i++;
        }

        return $slug;
    }
}
