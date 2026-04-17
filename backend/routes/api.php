<?php

use App\Http\Controllers\ActionController;
use App\Http\Controllers\GameController;
use App\Http\Controllers\RoomController;
use App\Models\Game;
use Illuminate\Support\Facades\Route;

Route::get('/welcome', function () {
    return 'Welcome to laravel !';
});

Route::prefix('rooms')->group(function () {
    Route::post('/',         [RoomController::class, 'store']);
    Route::post('join',      [RoomController::class, 'join']);
    Route::get('{room}',     [RoomController::class, 'show']);
    Route::patch('{room}/ready', [RoomController::class, 'ready']);
    Route::post('{room}/start',  [GameController::class, 'start']);
    Route::delete('{room}/leave', [RoomController::class, 'leave']);
});

Route::prefix('games/{game}')->group(function () {
    Route::get('/',   [GameController::class, 'show']);
    Route::get('me',  [GameController::class, 'myCharacter']);

    Route::prefix('rounds/{round}')->group(function () {
        Route::post('question',   [ActionController::class, 'question']);
        Route::post('accusation', [ActionController::class, 'accusation']);
        Route::post('actions/{action}/answer', [ActionController::class, 'answer']);
    });
});
