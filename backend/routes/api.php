<?php

use App\Http\Controllers\AccessController;
use App\Http\Controllers\ActionController;
use App\Http\Controllers\AdminGameController;
use App\Http\Controllers\AvatarController;
use App\Http\Controllers\CharacterController;
use App\Http\Controllers\ChatController;
use App\Http\Controllers\CosmosController;
use App\Http\Controllers\GameController;
use App\Http\Controllers\HistoryController;
use App\Http\Controllers\RoomController;
use App\Http\Controllers\UniverseController;
use Illuminate\Support\Facades\Route;

Route::get('/welcome', function () {
    return 'Welcome to laravel !';
});

// Authentification à la gestion des personnages (mot de passe -> token de session)
Route::post('access/characters', [AccessController::class, 'characters']);

Route::prefix('cosmos')->group(function () {
    // GET /cosmos reste ouvert : utilisé par le lobby (mode de jeu « cosmos »).
    Route::get('/', [CosmosController::class, 'index']);
    Route::post('/', [CosmosController::class, 'store'])->middleware('characters.access');
});

Route::prefix('universes')->middleware('characters.access')->group(function () {
    Route::get('/', [UniverseController::class, 'index']);
    Route::post('/', [UniverseController::class, 'store']);
    Route::patch('{universe}', [UniverseController::class, 'update']);
    Route::post('{universe}/characters', [CharacterController::class, 'store']);
});

Route::prefix('characters')->middleware('characters.access')->group(function () {
    Route::get('/', [CharacterController::class, 'index']);
    Route::get('pending', [CharacterController::class, 'pending']);
    Route::post('validation/accept', [CharacterController::class, 'accept']);
    Route::post('validation/reject', [CharacterController::class, 'reject']);
    Route::put('{character}', [CharacterController::class, 'update']);
    Route::patch('{character}/forbidden-words', [CharacterController::class, 'updateForbiddenWords']);
    Route::post('{character}/image', [CharacterController::class, 'uploadImage']);
    Route::delete('{character}', [CharacterController::class, 'destroy']);
});

// Vue admin des parties — protégée par le même mot de passe que la gestion des personnages.
Route::prefix('admin/games')->middleware('characters.access')->group(function () {
    Route::get('/', [AdminGameController::class, 'index']);
    Route::get('{game}', [AdminGameController::class, 'show']);
    Route::delete('{game}', [AdminGameController::class, 'destroy']);
});

// Historique public — aucune authentification. Seules les parties TERMINÉES
// sont exposées : publier une partie en cours révélerait les personnages à ses
// propres joueurs (voir HistoryController).
Route::prefix('history')->group(function () {
    Route::get('games', [HistoryController::class, 'games']);
    Route::get('games/{game}', [HistoryController::class, 'game']);
    Route::get('leaderboard', [HistoryController::class, 'leaderboard']);
    Route::get('players', [HistoryController::class, 'player']);
});

// Chat commun du hall (/rooms) — channel public `lobby`, ouvert aux visiteurs
// qui n'ont ni créé ni rejoint de salon.
Route::prefix('chat')->group(function () {
    Route::get('messages', [ChatController::class, 'index']);
    Route::post('messages', [ChatController::class, 'store'])->middleware('throttle:chat');
});

// Photos de profil des joueurs (remplacent le badge à initiales).
// L'image est servie par URL versionnée, jamais incluse dans le JSON ni les events.
Route::get('avatars/{avatar}', [AvatarController::class, 'show']);
Route::prefix('players/{player}/avatar')->middleware('throttle:avatars')->group(function () {
    Route::post('/', [AvatarController::class, 'update']);
    Route::delete('/', [AvatarController::class, 'destroy']);
});

Route::prefix('rooms')->group(function () {
    Route::post('/', [RoomController::class, 'store']);
    Route::post('join', [RoomController::class, 'join']);
    Route::get('{room}', [RoomController::class, 'show']);
    Route::patch('{room}/settings', [RoomController::class, 'updateSettings']);
    Route::patch('{room}/ready', [RoomController::class, 'ready']);
    Route::post('{room}/start', [GameController::class, 'start']);
    Route::delete('{room}/leave', [RoomController::class, 'leave']);
});

Route::prefix('games/{game}')->group(function () {
    Route::get('/', [GameController::class, 'show']);
    Route::get('me', [GameController::class, 'myCharacter']);
    Route::get('recap', [GameController::class, 'recap']);
    Route::post('reactions', [GameController::class, 'react'])->middleware('throttle:reactions');
    Route::post('skip-turn', [GameController::class, 'skipTurn']);
    Route::post('players/{player}/exclude', [GameController::class, 'excludePlayer']);

    Route::prefix('rounds/{round}')->group(function () {
        Route::post('question', [ActionController::class, 'question']);
        Route::post('accusation', [ActionController::class, 'accusation']);
        Route::post('actions/{action}/answer', [ActionController::class, 'answer']);
        Route::post('actions/{action}/confirm', [ActionController::class, 'confirmAccusation']);
    });
});
