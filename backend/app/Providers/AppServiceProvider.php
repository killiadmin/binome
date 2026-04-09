<?php

namespace App\Providers;

use Illuminate\Support\ServiceProvider;
use Illuminate\Support\Facades\Route;

class AppServiceProvider extends ServiceProvider
{
    public function register(): void {}

    public function boot(): void
    {
        Route::middleware('web')
            ->post('/broadcasting/auth', [
                \App\Http\Controllers\BroadcastAuthController::class,
                'authenticate',
            ]);

        require base_path('routes/channels.php');
    }
}
