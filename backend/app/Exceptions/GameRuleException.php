<?php

namespace App\Exceptions;

use RuntimeException;

/**
 * Coup refusé par les règles du jeu (pas ton tour, cible éliminée, partie
 * déjà lancée…). Rendue en 409 avec son message (bootstrap/app.php) : c'est
 * une erreur du joueur, pas du serveur — en 500, le front n'afficherait que
 * « Server Error » dès que APP_DEBUG est désactivé.
 */
class GameRuleException extends RuntimeException
{
}
