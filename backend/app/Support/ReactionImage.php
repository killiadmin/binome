<?php

namespace App\Support;

/**
 * Contrôle d'une image de réaction envoyée en data URI JPEG carrée — partagé
 * par la création côté joueur et côté admin. Le navigateur recadre et
 * réencode (pas de GD dans le conteneur) : on vérifie format, poids, taille.
 */
class ReactionImage
{
    public const PREFIX = 'data:image/jpeg;base64,';

    /** Une image 128 px JPEG pèse ~8 ko. */
    private const MAX_BYTES = 60 * 1024;

    private const MIN_SIDE = 64;

    private const MAX_SIDE = 256;

    /** Règles de validation Laravel du champ `image`. */
    public const RULES = ['string', 'max:100000', 'starts_with:'.self::PREFIX];

    /**
     * Message d'erreur, ou null si l'image est valide.
     */
    public static function error(string $dataUri): ?string
    {
        $bytes = base64_decode(substr($dataUri, strlen(self::PREFIX)), true);
        $info = $bytes !== false ? @getimagesizefromstring($bytes) : false;

        if ($bytes === false || strlen($bytes) > self::MAX_BYTES || ! $info || $info[2] !== IMAGETYPE_JPEG) {
            return 'L\'emoji doit être une image JPEG de moins de 60 ko.';
        }

        [$width, $height] = $info;

        if ($width !== $height || $width < self::MIN_SIDE || $width > self::MAX_SIDE) {
            return 'L\'emoji doit être carré, entre 64 et 256 pixels de côté.';
        }

        return null;
    }
}
