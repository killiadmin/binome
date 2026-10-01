<?php

namespace App\Http\Controllers;

use App\Events\ChatMessageSent;
use App\Http\Requests\StoreChatMessageRequest;
use App\Models\ChatMessage;
use App\Models\Player;
use Illuminate\Http\JsonResponse;
use Illuminate\Routing\Controller;

class ChatController extends Controller
{
    /** Nombre de messages renvoyés à l'ouverture de la page. */
    private const HISTORY_SIZE = 50;

    /**
     * Durée de vie d'un message. Passé ce délai il n'est plus servi par
     * l'API et il est supprimé en base à la prochaine lecture ou écriture.
     * Le front applique la même limite en local (voir LobbyChat.vue) pour que
     * les messages disparaissent de l'écran sans attendre un rechargement.
     */
    private const RETENTION_MINUTES = 60;

    /**
     * GET /chat/messages
     * Historique récent du chat commun (ordre chronologique).
     */
    public function index(): JsonResponse
    {
        $this->prune();

        $messages = ChatMessage::query()
            ->where('created_at', '>=', $this->cutoff())
            ->latest('id')
            ->limit(self::HISTORY_SIZE)
            ->get()
            ->reverse()
            ->values()
            ->map(fn (ChatMessage $m) => $m->toBroadcastArray());

        return response()->json(['messages' => $messages]);
    }

    /**
     * POST /chat/messages
     * Poste un message dans le chat commun et le diffuse sur le channel `lobby`.
     */
    public function store(StoreChatMessageRequest $request): JsonResponse
    {
        $player = $request->validated('player_id')
            ? Player::find($request->validated('player_id'))
            : null;

        // Le nom d'auteur est toujours dérivé côté serveur : un client ne peut
        // pas se faire passer pour le pseudo d'un joueur du salon.
        $anonId = $request->validated('anon_id');

        $message = ChatMessage::create([
            'player_id' => $player?->id,
            'author_name' => $player?->pseudo ?? 'Anonyme#'.$anonId,
            'author_key' => $player ? 'p:'.$player->id : 'a:'.$anonId,
            'body' => trim($request->validated('body')),
        ]);

        $this->prune();

        broadcast(new ChatMessageSent($message));

        return response()->json(['message' => $message->toBroadcastArray()], 201);
    }

    private function prune(): void
    {
        ChatMessage::where('created_at', '<', $this->cutoff())->delete();
    }

    private function cutoff(): \Illuminate\Support\Carbon
    {
        return now()->subMinutes(self::RETENTION_MINUTES);
    }
}
