<?php

namespace App\Support;

use Illuminate\Support\Str;

/**
 * Détection des mots interdits dans une question, en deux niveaux :
 *
 *  - exact (100 %) : un mot de la question EST le mot interdit, à la casse,
 *    aux accents et au pluriel (s/x final) près → la cible DOIT mentir ;
 *  - proche (≥ 80 %) : faute de frappe, conjugaison ou dérivé
 *    (« vol » / « voler ») → le mensonge est légal.
 *
 * Port du détecteur front (frontend/src/services/forbiddenWords.js), seul
 * à servir pendant la partie : garder les deux synchronisés.
 */
class ForbiddenWordMatcher
{
    public const NEAR_THRESHOLD = 0.8;

    private const SUFFIXES = [
        'issements', 'issement', 'eraient', 'assions', 'erions', 'eriez', 'erons', 'eront',
        'erais', 'erait', 'aient', 'ement', 'ments', 'ment', 'euse', 'eurs', 'ions', 'eur',
        'ees', 'iez', 'ais', 'ait', 'ant', 'ent', 'es', 'ee', 'er', 'ir', 'ez', 'ai', 'e', 's', 'x',
    ];

    /**
     * @param  array<int, string>  $forbiddenWords
     * @return array{exact: array<int, string>, near: array<int, string>}
     */
    public static function match(?string $question, array $forbiddenWords): array
    {
        $tokens = self::tokenize((string) $question);
        $result = ['exact' => [], 'near' => []];

        if (! $tokens) {
            return $result;
        }

        foreach ($forbiddenWords as $word) {
            $score = self::bestScore($tokens, (string) $word);
            if ($score >= 1) {
                $result['exact'][] = $word;
            } elseif ($score >= self::NEAR_THRESHOLD) {
                $result['near'][] = $word;
            }
        }

        return $result;
    }

    /** Vrai si la question contient un mot interdit, exact ou proche. */
    public static function matchesAny(?string $question, array $forbiddenWords): bool
    {
        $match = self::match($question, $forbiddenWords);

        return $match['exact'] || $match['near'];
    }

    /** @return array<int, string> */
    private static function tokenize(string $text): array
    {
        $ascii = strtolower(Str::ascii($text, 'fr'));

        return array_values(array_filter(preg_split('/[^a-z0-9]+/', $ascii) ?: [], 'strlen'));
    }

    private static function singular(string $token): string
    {
        return strlen($token) > 3 && preg_match('/[sx]$/', $token) ? substr($token, 0, -1) : $token;
    }

    private static function stem(string $token): string
    {
        foreach (self::SUFFIXES as $suffix) {
            if (str_ends_with($token, $suffix) && strlen($token) - strlen($suffix) >= 3) {
                return substr($token, 0, -strlen($suffix));
            }
        }

        return $token;
    }

    private static function similarity(string $q, string $f): float
    {
        if (self::singular($q) === self::singular($f)) {
            return 1.0;
        }
        if (strlen($q) < 3 || strlen($f) < 3) {
            return 0.0;
        }

        $score = 1 - levenshtein($q, $f) / max(strlen($q), strlen($f));
        if (self::stem($q) === self::stem($f)) {
            $score = max($score, 0.9);
        }

        return $score;
    }

    /** @param  array<int, string>  $tokens */
    private static function bestScore(array $tokens, string $forbidden): float
    {
        $fTokens = self::tokenize($forbidden);
        $n = count($fTokens);
        if ($n === 0) {
            return 0.0;
        }

        $fJoined = implode('', $fTokens);
        $best = 0.0;

        for ($i = 0; $i + $n <= count($tokens); $i++) {
            $window = array_slice($tokens, $i, $n);
            $perToken = min(array_map(fn ($t, $k) => self::similarity($t, $fTokens[$k]), $window, array_keys($window)));
            $joined = $n > 1 ? self::similarity(implode('', $window), $fJoined) : 0.0;
            $best = max($best, $perToken, $joined);
        }

        if ($n > 1) {
            foreach ($tokens as $token) {
                $best = max($best, self::similarity($token, $fJoined));
            }
        }

        return $best;
    }
}
