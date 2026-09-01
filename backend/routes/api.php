<?php

use App\Http\Controllers\ActionController;
use App\Http\Controllers\CharacterController;
use App\Http\Controllers\CosmosController;
use App\Http\Controllers\GameController;
use App\Http\Controllers\RoomController;
use App\Http\Controllers\UniverseController;
use Illuminate\Support\Facades\Route;

Route::get('/welcome', function () {
    return 'Welcome to laravel !';
});

Route::prefix('cosmos')->group(function () {
    Route::get('/', [CosmosController::class, 'index']);
    Route::post('/', [CosmosController::class, 'store']);
});

Route::prefix('universes')->group(function () {
    Route::get('/', [UniverseController::class, 'index']);
    Route::post('/', [UniverseController::class, 'store']);
    Route::patch('{universe}', [UniverseController::class, 'update']);
    Route::post('{universe}/characters', [CharacterController::class, 'store']);
});

Route::prefix('characters')->group(function () {
    Route::get('/', [CharacterController::class, 'index']);
    Route::get('pending', [CharacterController::class, 'pending']);
    Route::post('validation/accept', [CharacterController::class, 'accept']);
    Route::post('validation/reject', [CharacterController::class, 'reject']);
    Route::put('{character}', [CharacterController::class, 'update']);
    Route::patch('{character}/forbidden-words', [CharacterController::class, 'updateForbiddenWords']);
    Route::post('{character}/image', [CharacterController::class, 'uploadImage']);
    Route::delete('{character}', [CharacterController::class, 'destroy']);
});

Route::prefix('rooms')->group(function () {
    Route::post('/', [RoomController::class, 'store']);
    Route::post('join', [RoomController::class, 'join']);
    Route::get('{room}', [RoomController::class, 'show']);
    Route::patch('{room}/ready', [RoomController::class, 'ready']);
    Route::post('{room}/start', [GameController::class, 'start']);
    Route::delete('{room}/leave', [RoomController::class, 'leave']);
});

Route::prefix('games/{game}')->group(function () {
    Route::get('/', [GameController::class, 'show']);
    Route::get('me', [GameController::class, 'myCharacter']);

    Route::prefix('rounds/{round}')->group(function () {
        Route::post('question', [ActionController::class, 'question']);
        Route::post('accusation', [ActionController::class, 'accusation']);
        Route::post('actions/{action}/answer', [ActionController::class, 'answer']);
        Route::post('actions/{action}/confirm', [ActionController::class, 'confirmAccusation']);
    });
});
