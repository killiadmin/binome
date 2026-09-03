<?php

return [

    /*
    |--------------------------------------------------------------------------
    | Accès à la gestion des personnages
    |--------------------------------------------------------------------------
    |
    | Mot de passe protégeant les pages « Liste des personnages » et « Création
    | de personnages » (front) ainsi que les routes API correspondantes
    | (universes, characters, POST cosmos). Laissé vide, l'accès est refusé.
    |
    */

    'characters_password' => env('CHARACTERS_ACCESS_PASSWORD'),

];
